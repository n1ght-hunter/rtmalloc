# Potential Improvements for rtmalloc

Ideas drawn from rpmalloc, mimalloc, jemalloc, and snmalloc that could improve rtmalloc's throughput, latency, RSS behavior, or fragmentation resistance. Organized by impact area.

---

## Current rtmalloc Architecture (Baseline)

rtmalloc is a tcmalloc-style allocator with:
- Three-tier architecture: thread cache → transfer cache → central free list → page heap
- Per-thread free lists with adaptive slow-start growth
- Low-water-mark GC for thread cache scavenging
- 3-level radix tree pagemap for O(1) span lookup
- Span coalescing in the page heap
- Optional per-CPU caches via rseq

---

## Speed Improvements

### 1. Free List Sharding (from mimalloc) — HIGH IMPACT

**Problem**: rtmalloc's central free list has one lock per size class. Under contention, all threads requesting the same size class serialize on this lock.

**Idea**: Instead of a single central free list per size class, shard the free list across multiple spans. Each span maintains its own local free list. When a thread needs objects, it grabs an entire span (or partially-full span) rather than objects from a central list.

**What this looks like in rtmalloc**:
- The `CentralFreeList` currently maintains a `nonempty_spans: SpanList` and extracts individual objects
- Instead, when a thread's cache is empty, give it an entire span to allocate from directly
- The thread's `FreeList` would contain a pointer to its "active span" per size class
- Objects freed back to the same span's free list stay on that span (locality)
- When all objects in a span are freed, the whole span returns to the page heap

**Expected benefit**: Better locality (temporally related allocations cluster on the same pages), reduced central lock contention, increased probability of full-span return to OS.

**Estimated complexity**: Medium. Requires reworking the thread cache ↔ central free list interface. The transfer cache may become less necessary.

---

### 2. Faster Metadata Lookup via Address Masking (from rpmalloc/mimalloc) — MEDIUM IMPACT

**Problem**: rtmalloc uses a 3-level radix tree (`PageMap`) for pointer-to-span lookup. This requires three pointer chases with `Acquire` ordering on every `dealloc`.

**Idea**: Align spans (or groups of spans/segments) to power-of-two boundaries. Then the span metadata can be found by masking the pointer: `ptr & ~(segment_size - 1)`. This eliminates the radix tree lookup on the deallocation hot path.

**What this looks like in rtmalloc**:
- Allocate memory from the OS in fixed-size segments (e.g., 4 MiB or 32 MiB)
- Store segment metadata (including per-span info) at the start of each segment
- On dealloc: `segment = ptr & SEGMENT_MASK` → metadata is at `segment + offset`
- The radix tree could be kept as a fallback for large allocations that don't fit the segment model

**Trade-off**: Requires aligned OS allocations (over-allocate + trim on mmap, natural on Windows VirtualAlloc with 64 KiB granularity). Increases virtual address space usage slightly.

**Expected benefit**: ~5-15ns savings per dealloc on the hot path (eliminates three cache-line loads for radix tree traversal).

---

### 3. Batch Message-Passing for Cross-Thread Frees (from snmalloc) — MEDIUM IMPACT

**Problem**: rtmalloc's cross-thread deallocation path goes through the transfer cache or central free list, both of which require locking.

**Idea**: When a thread frees an object belonging to another thread's span, batch these frees and send them as a message to the owning thread's queue (lock-free MPSC). The owning thread processes them on its next slow-path entry.

**What this looks like in rtmalloc**:
- Add a per-thread `RemoteQueue` (lock-free linked list)
- On dealloc: if span owner ≠ current thread, enqueue to owner's RemoteQueue
- On alloc slow path: drain own RemoteQueue, recycle objects into local free lists
- Batch sends: accumulate in a thread-local buffer, flush when threshold reached (single CAS)

**Trade-off**: Deferred processing means freed memory isn't immediately available to other threads. Good for producer-consumer workloads, neutral for symmetric patterns.

**Expected benefit**: Eliminates central free list lock contention for cross-thread patterns. Particularly beneficial when threads specialize (some produce, some consume).

---

### 4. Eliminate Transfer Cache Round-Trip for Full Batches (minor)

**Problem**: The transfer cache stores pre-linked batches. If a thread needs more or fewer objects than `batch_size`, it must go through the central free list anyway.

**Idea**: Allow the transfer cache to hold partial batches. Or better: if we adopt span-level sharding (#1), the transfer cache can transfer entire spans instead of object lists.

**Expected benefit**: Small reduction in central free list lock acquisitions.

---

### 5. Reduce Fast-Path Branches

**Current state**: rtmalloc's fast path for nightly TLS is: load TLS → check state (Uninitialized/Active/Destroyed) → index into `lists[class]` → pop from free list.

**mimalloc achieves**: Load heap → direct page lookup → pop from free → one NULL check.

**Idea**: On the nightly path, ensure the `#[thread_local]` ThreadCache is const-initialized so the state check can be eliminated after the first call. Consider a direct page-pointer array per size class (like mimalloc's `pages_direct`) instead of indexing into the FreeList array.

**Expected benefit**: 1-2 fewer branches on the allocation fast path. Small but compounds across billions of allocations.

---

## RSS / Memory Overhead Improvements

### 6. Eager Page Purging with Configurable Delay (from mimalloc) — HIGH IMPACT

**Problem**: rtmalloc currently doesn't have a page purging/decommit mechanism. Once memory is mapped, it stays resident until the span is unmapped.

**Idea**: When a span becomes free but isn't immediately reused, decommit its physical pages after a configurable delay. This keeps virtual address space reserved (avoiding fragmentation) but reduces RSS.

**What this looks like in rtmalloc**:
- Add `page_decommit(ptr, size)` to `platform.rs` (madvise(MADV_DONTNEED) on Linux, VirtualFree(MEM_DECOMMIT) on Windows — `page_decommit` already exists!)
- Track decommit state per span (add a `committed: bool` field to `Span`)
- When a span enters the page heap free list, start a purge timer
- After the delay (configurable, default ~10ms), decommit the span's pages
- On reuse, recommit (or let demand-faulting handle it)

**Trade-off**: Purge delay too short → CPU overhead from frequent decommit/recommit. Too long → RSS stays high. Mimalloc's 10ms default is a good starting point.

**Expected benefit**: Significant RSS reduction for long-running programs with bursty allocation patterns. This is probably the single highest-impact change for RSS.

---

### 7. Decay-Based Two-Phase Purging (from jemalloc) — MEDIUM IMPACT

**Problem**: A single decommit threshold doesn't account for the difference between "probably will be reused soon" and "definitely idle."

**Idea**: Implement a two-phase purge inspired by jemalloc:
1. **Phase 1 (Dirty → Muzzy)**: After `dirty_decay_ms` (default 10s), call `madvise(MADV_FREE)` (Linux) — the OS *may* reclaim the pages under memory pressure but they remain resident if untouched.
2. **Phase 2 (Muzzy → Clean)**: After `muzzy_decay_ms` (default 10s), call `madvise(MADV_DONTNEED)` — pages are unconditionally reclaimed.

**What this looks like in rtmalloc**:
- Extend `Span` with a `last_free_time: Instant` (or a monotonic tick counter)
- On each allocation slow path, check the page heap free lists for spans past their decay threshold
- Phase 1: mark as "MADV_FREE'd" → Phase 2: mark as decommitted
- Configuration: expose `dirty_decay_ms` and `muzzy_decay_ms` as build-time or runtime settings

**Trade-off**: Complexity. Two phases are more to manage than one. But the gradual approach avoids decommit/recommit churn.

**Expected benefit**: Better RSS tracking of actual usage over time. Particularly valuable for services with periodic load spikes.

---

### 8. Per-Span Object Tracking for Full-Span Return (from mimalloc) — MEDIUM IMPACT

**Problem**: rtmalloc tracks `allocated_count` per span, but the central free list holds spans even when they could be fully returned to the page heap.

**Idea**: When `allocated_count` drops to zero (all objects freed), immediately remove the span from the central free list and return it to the page heap for potential decommit.

**Current state**: `CentralFreeList::insert_range()` (central_free_list.rs:93-138) does check for fully free spans and returns them. Verify this path is actually exercised — ensure there's no "keep one cached span" logic that prevents return of idle spans under memory pressure.

**Expected benefit**: Faster reclamation of idle spans → lower RSS.

---

### 9. Thread Cache Budget Tightening — LOW-MEDIUM IMPACT

**Problem**: `OVERALL_THREAD_CACHE_SIZE` is 32 MiB. With many threads, each claiming `MIN_PER_THREAD_CACHE_SIZE` (512 KiB) plus stealing from the global pool, total cached memory can be substantial.

**Ideas**:
- Reduce `OVERALL_THREAD_CACHE_SIZE` default (e.g., 8-16 MiB)
- Make it configurable at runtime via an init function
- Implement more aggressive scavenging: if a size class hasn't been used in N scavenge cycles, release all cached objects (not just above low-water mark)
- Add a global memory pressure callback that triggers emergency cache flushing across all threads

**Expected benefit**: Lower idle RSS, especially for applications with many threads that have bursty allocation patterns.

---

### 10. Segment-Based Virtual Memory (from mimalloc/rpmalloc) — MEDIUM IMPACT

**Problem**: rtmalloc's page heap grows by requesting ≥128 pages at a time from the OS, but there's no higher-level structure grouping these allocations.

**Idea**: Allocate from the OS in large segments (e.g., 4-32 MiB). Track commit/decommit state at the page level within each segment using a bitmap. This enables:
- Partial decommit (return individual pages without unmapping the segment)
- Reduced mmap/VirtualAlloc syscall frequency
- Better huge page / THP utilization (aligned, contiguous ranges)

**What this looks like in rtmalloc**:
- New `Segment` struct: `start_address`, `total_pages`, `commit_bitmap`, `purge_bitmap`
- `PageHeap::grow_heap()` allocates segments instead of raw OS pages
- Decommit individual pages within a segment via bitmap tracking
- When an entire segment is idle, optionally unmap it

**Expected benefit**: Fewer syscalls, better THP utilization, fine-grained RSS control.

---

## Fragmentation Improvements

### 11. Lowest-Address-First Allocation (from jemalloc) — LOW-MEDIUM IMPACT

**Problem**: rtmalloc's central free list serves from whichever span happens to be at the head of `nonempty_spans`. This can spread allocations across many spans.

**Idea**: When selecting a span to allocate from, prefer the span with the lowest start address. This concentrates live objects into fewer pages, increasing the chance that high-address spans become completely empty and returnable.

**What this looks like in rtmalloc**:
- In `CentralFreeList`, maintain `nonempty_spans` sorted by `start_page` (or use a min-heap)
- When a partially-free span is returned, insert it in sorted order

**Trade-off**: Sorted insertion is O(n) worst case. Could use a priority queue for O(log n).

**Expected benefit**: Reduced fragmentation, more complete spans for return to page heap.

---

### 12. Size Class Optimization

**Current state**: rtmalloc's size classes are configurable via `rtmalloc.toml`, with a histogram feature for workload-specific tuning.

**Ideas from other allocators**:
- **jemalloc's 4-per-doubling**: Guarantees ≤20% internal fragmentation. Check if rtmalloc's default classes match this.
- **mimalloc's ~12.5% bound**: More size classes means less waste per allocation but more metadata.
- **rpmalloc's variable granularity**: For medium/large sizes, use a granularity that caps waste at a fixed percentage rather than a fixed byte count.

**Action**: Analyze rtmalloc's current size class distribution for gaps where internal fragmentation exceeds 20%. The histogram feature already supports this — run it against target workloads and adjust.

---

## Architectural Improvements

### 13. Flat Pagemap Option (from snmalloc) — LOW IMPACT

**Problem**: The 3-level radix tree is flexible but involves three pointer chases.

**Idea**: For 48-bit address spaces, a flat pagemap (array indexed by `addr >> PAGE_SHIFT`) uses ~32 MB of virtual memory but only physical memory for touched pages. This gives true O(1) lookup with a single array index.

**Trade-off**: 32 MB virtual reservation. Acceptable on 64-bit systems, not on 32-bit.

**Expected benefit**: Simpler code, slightly faster lookup. Consider as a platform-specific option.

---

### 14. Span-Local Free Lists for Central Allocation (from mimalloc) — MEDIUM IMPACT

**Problem**: The central free list extracts individual objects from spans and links them into a separate list. This destroys the spatial locality of the span.

**Idea**: Keep freed objects on the span's own free list (`span.freelist`). When a thread needs objects, hand it the entire span (or a reference to allocate from). The thread allocates from the span's local free list until it's exhausted, then gets another span.

This is essentially the span-sharding from improvement #1, applied at the central free list level.

**Expected benefit**: Much better cache locality — objects stay near their siblings.

---

### 15. Background Purge Thread (from jemalloc) — LOW IMPACT

**Problem**: Purging on the allocation slow path adds latency to application threads.

**Idea**: Spawn a background thread that periodically scans the page heap for idle spans past their decay threshold and decommits them. This moves purging off the hot path.

**Trade-off**: Adds a thread. May not be appropriate for all deployment contexts (embedded, WASM, etc.). Make it opt-in via a feature flag.

**Expected benefit**: Lower tail latency on allocation-heavy workloads.

---

## Priority Ranking

| # | Improvement | Speed | RSS | Frag | Complexity | Priority |
|---|------------|-------|-----|------|------------|----------|
| 6 | Eager page purging | — | +++ | — | Low | **P0** |
| 1 | Free list sharding | ++ | + | ++ | Medium | **P1** |
| 3 | Message-passing remote frees | ++ | — | — | Medium | **P1** |
| 10 | Segment-based VM | + | ++ | + | Medium | **P1** |
| 7 | Two-phase decay purging | — | ++ | — | Medium | **P2** |
| 2 | Address masking metadata lookup | ++ | — | — | Medium | **P2** |
| 9 | Thread cache budget tightening | — | + | — | Low | **P2** |
| 11 | Lowest-address-first allocation | — | + | ++ | Low | **P2** |
| 5 | Reduce fast-path branches | + | — | — | Low | **P3** |
| 8 | Full-span return verification | — | + | — | Low | **P3** |
| 14 | Span-local free lists | + | — | + | Medium | **P3** |
| 12 | Size class analysis | — | — | + | Low | **P3** |
| 15 | Background purge thread | — | + | — | Low | **P3** |
| 13 | Flat pagemap option | + | — | — | Low | **P4** |
| 4 | Transfer cache partial batches | + | — | — | Low | **P4** |

**Legend**: `+++` = major improvement, `++` = moderate, `+` = minor, `—` = neutral

---

## Recommended Implementation Order

### Phase 1: RSS (biggest gap vs. competitors)
1. **#6 Eager page purging** — rtmalloc already has `page_decommit` in `platform.rs`. Wire it up to the page heap free lists with a configurable delay. This is the lowest-effort, highest-impact change.
2. **#9 Thread cache budget tightening** — Reduce defaults, add more aggressive scavenging.

### Phase 2: Throughput
3. **#1 Free list sharding** — Rework the central free list to hand out spans rather than individual objects. This improves both speed and locality.
4. **#3 Message-passing remote frees** — Add a lock-free remote queue per thread. Reduces contention on the central free list.

### Phase 3: Architecture
5. **#10 Segment-based VM** — Restructure OS allocation around segments with commit bitmaps.
6. **#2 Address masking** — Once segments are in place, metadata lookup becomes a mask operation.
7. **#7 Two-phase decay** — Build on the segment commit bitmaps for fine-grained purging.

### Phase 4: Polish
8. **#11 Lowest-address-first** — Sort nonempty_spans for better fragmentation.
9. **#5 Reduce fast-path branches** — Profile and optimize the hot path.
10. **#12 Size class analysis** — Run histograms against target workloads and tune.

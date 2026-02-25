# Modern Allocator Comparison: rpmalloc, mimalloc, jemalloc, snmalloc

A deep technical comparison of four high-performance memory allocators, focusing on what makes each unique, their key optimizations, and their trade-offs.

---

## Overview

| Property | rpmalloc | mimalloc | jemalloc | snmalloc |
|----------|----------|----------|----------|----------|
| **Author** | Mattias Jansson | Microsoft Research (Daan Leijen) | Jason Evans / Meta | Microsoft Research |
| **Language** | C (~2,200 LOC) | C (~3,500 LOC core) | C (~25,000 LOC) | C++ (~15,000 LOC) |
| **License** | Public domain | MIT | BSD-2 | MIT |
| **First release** | ~2017 | 2019 | 2005 (FreeBSD 7) | 2019 |
| **Key paper** | None (blog post) | APLAS 2019 | BSDCan 2006 | ISMM 2019 |
| **Design origin** | Game engines | Koka/Lean runtimes | FreeBSD libc | Verona runtime |

---

## 1. rpmalloc

### What Makes It Unique

**Absolute lock-freedom.** rpmalloc is the only allocator in this comparison that uses zero mutexes anywhere. Cross-thread deallocation is handled entirely through atomic operations on per-span deferred free lists. The allocation fast path touches zero shared state.

**O(1) metadata lookup via address masking.** Spans are aligned to known boundaries (64 KiB on main branch, page-type-dependent on develop). To find the metadata for any pointer, it masks the address: `(uintptr_t)p & _memory_span_mask`. No radix tree, no hash table — just a bitwise AND.

**Single-ownership model.** Every span is owned by exactly one thread's heap. Foreign frees are deferred to the owning thread and processed lazily on the next allocation from that size class. This means producer-consumer patterns see rapid object reuse without global cache round-trips.

**Minimal codebase.** Two files (`rpmalloc.c` + `rpmalloc.h`), ~2,200 LOC, public domain. Extremely easy to audit, embed, and modify.

### Key Optimizations

- **Four-level thread cache**: Active span → partially-free spans → thread-local free spans → global free span cache. Each level avoids locks.
- **Cross-thread deallocation via atomic CAS on per-span lists**: The develop branch packs both the block index and count into a single 64-bit atomic, enabling a true lock-free CAS loop.
- **Configurable cache modes**: Unlimited (max speed), Performance (default, balanced), Size-oriented (low RSS), No-cache (minimal overhead, still beats glibc to ~6 threads).
- **Page-level decommit** (develop branch): Individual pages within a 256 MiB span can be decommitted without releasing the virtual address range.
- **Three-tier size classes**: Small (16-byte granularity), Medium (variable), Large (variable) — with variable granularity designed to cap waste at a fixed percentage rather than fixed byte count.

### Trade-offs

- Cross-thread frees are deferred (memory not immediately reusable by other threads).
- Thread cache memory is proportional to thread count.
- Virtual address space overhead from alignment padding (especially 256 MiB spans on develop).
- Less sophisticated heuristics than jemalloc/tcmalloc due to simplicity focus.
- Known RSS issues under sustained heavy allocation on develop branch (issue #334).

---

## 2. mimalloc

### What Makes It Unique

**Free list sharding.** The defining innovation. Instead of one free list per size class across the heap, mimalloc maintains a free list *per page* (~64 KiB). Since there are thousands of pages, temporally related allocations cluster on the same physical page. This dramatically improves locality and increases the probability of entire pages becoming empty (enabling return to the OS). The Lean runtime saw >25% speedup from this single change alone.

**Three free lists per page.** Each page has:
1. **`free`**: Primary allocation list. Thread-local, zero atomics.
2. **`local_free`**: Same-thread frees. Thread-local, zero atomics.
3. **`thread_free`**: Cross-thread frees. Single atomic CAS per free, sharded across thousands of pages so contention is near-zero.

When `free` is exhausted, `local_free` is swapped in (pointer swap, no atomics). When that's exhausted, `thread_free` is atomically collected. This batching amortizes cross-thread overhead.

**Temporal cadence.** The `free` list is guaranteed to empty after a bounded number of allocations. This triggers the "generic path" which performs deferred freeing, cross-thread list collection, and runtime heartbeat callbacks — giving bounded worst-case allocation times.

**No bump pointer on fast path.** The team tested bump-pointer allocation and found it consistently ~2% slower because it requires two conditionals in the fast path (bump or free-list?) versus one (free-list empty?).

### Key Optimizations

- **Single conditional on the malloc fast path**: Pop from `page->free`, check NULL. That's it.
- **Segment-based virtual memory**: ~32 MiB segments divided into 64 KiB slices. Arena-level bitmap management for commit/purge state.
- **Eager page purging** (`MIMALLOC_PURGE_DELAY`, default 10ms): Returns memory to the OS aggressively, keeping RSS low for long-running programs with changing patterns.
- **Abandoned segment reclamation**: When a thread dies, its segments are reclaimed by other threads on next allocation or free to that segment.
- **First-class heaps** (v3): Multiple independent heaps usable from any thread, destroyable atomically. Enables arena-style allocation.

### Trade-offs

- Up to 25% more memory than the best allocator on specific benchmarks (worst case).
- ~12.5% maximum internal fragmentation from size classes.
- Throughput for allocations >64 KiB drops off more than some competitors.
- Segment overhead (32 MiB granularity) can be wasteful for small programs.

### Security Features

- **Secure mode** (~10% overhead): Guard pages, encoded free-list pointers, double-free detection, randomized allocation order.
- **Guarded mode**: OS guard pages behind sampled allocations (configurable sampling rate).

---

## 3. jemalloc

### What Makes It Unique

**Arena-based isolation.** jemalloc creates `4 * ncpus` independent arenas, each with its own locks, bins, extent caches, and decay timers. Threads are assigned to arenas via round-robin (or CPU affinity with `percpu_arena`). This reduces contention by probabilistic distribution rather than lock-freedom.

**Decay-based purging with sigmoidal curve.** Instead of purging all dirty pages immediately or never, jemalloc uses a time-based decay with a sigmoidal (S-shaped) curve. Pages transition: Active → Dirty → Muzzy → Clean. The two-phase purge (MADV_FREE then MADV_DONTNEED) smooths CPU overhead while maintaining accurate RSS. Default decay: 10 seconds per phase.

**Production-grade introspection.** The `mallctl()` API exposes a hierarchical namespace with per-arena, per-bin, per-extent, and per-mutex statistics. Built-in heap profiling with Bernoulli sampling, bias correction, and `jeprof` compatibility. Background threads for async purging. No other allocator matches this level of operational observability.

**Bitmap-based slab allocation.** Small allocations use slabs with hierarchical bitmaps (tree-based `bitmap_sfu` — "scan forward unset"). This avoids intrusive free lists within objects, enabling the allocator to always serve from the lowest-address non-full slab (concentrating allocations for locality).

### Key Optimizations

- **Thread cache (tcache)**: Per-thread contiguous stack per size class. Fast path: stack pop, zero locks. Bulk fill/flush amortizes arena lock acquisition.
- **Four size classes per doubling**: Limits worst-case internal fragmentation to ~20%.
- **Lowest-address-first slab allocation**: Concentrates live objects into minimal pages, reducing working set.
- **Extent merging**: Adjacent free extents are coalesced to form larger contiguous regions.
- **Oversize arena isolation**: Large allocations (>8 MiB) go to dedicated arenas to prevent fragmenting normal arenas.
- **Background threads**: Shift purging off the application hot path, improving tail latency.
- **Transparent huge page support**: `metadata_thp`, HPA (Huge Page Allocator) PAI implementation.

### Trade-offs

- Higher baseline RSS (~9 MB) compared to mimalloc/glibc (~4 MB) due to arena metadata.
- More complex codebase (~25K LOC) — harder to audit and modify.
- Single-threaded performance slightly below tcmalloc/mimalloc (arena overhead).
- tcache GC can create periodic latency spikes on allocation-heavy threads.
- Per-arena locks still serialize within an arena under high contention.

### Unique Capabilities

- **Extended API**: `mallocx`, `rallocx`, `xallocx` (in-place resize), `sdallocx` (sized dealloc), `mallctl` (programmatic control).
- **Explicit tcache management**: Create, destroy, pass between threads.
- **Extent hooks**: Full customization of memory mapping, commit, decommit, purge, split, merge.
- **Heap profiling**: Bernoulli sampling with bias correction, peak-RSS-triggered dumps, `jeprof` output.

---

## 4. snmalloc

### What Makes It Unique

**Message-passing for cross-thread frees.** The signature innovation. When Thread B frees memory owned by Thread A, the freed pointer goes into a per-allocator batching queue. When the batch reaches a threshold, the entire batch is sent via a single atomic CAS to the destination allocator's lock-free MPSC queue. The owning thread processes incoming messages lazily. This means thousands of remote frees can be batched with only one atomic operation.

**Bump-pointer-free-list hybrid.** Each slab maintains both a bump pointer (high-water mark of virgin space) and a free list (previously freed slots). Allocation tries the free list first (cache-warm), then bumps. This requires only **64 bits of metadata per 64 KiB slab** — the most compact metadata representation of any allocator here.

**Two-branch malloc fast path.** On Linux with Clang, the small allocation fast path compiles to just two conditional branches: size-class lookup + free-list pop.

**CHERI-compatible design.** The `CapPtr<T, B>` type system encodes pointer provenance bounds, making snmalloc one of the few allocators that works on capability-based hardware (Arm Morello). All metadata is reached via pointer chains from TLS/globals rather than pointer arithmetic from allocation addresses.

### Key Optimizations

- **Per-allocator `LocalCache` array**: Cached free list per small size class, inspired by mimalloc's sharding.
- **Lock-free MPSC queues**: Multiple-producer, single-consumer queues for remote deallocation. Producers append via CAS on tail; consumer drains without contention.
- **Thread-local buddy allocators**: Each thread has its own buddy allocator, refilling from a global one at 2 MiB granularity. Minimizes global lock frequency.
- **Variable-sized slabs (v2)**: Out-of-band metadata, replacing fixed superslabs. Reduces minimum memory requirements.
- **Range pipeline**: Memory flows through a chain of range allocators (thread-local buddy → commit → global buddy → pagemap → OS), each layer handling one concern.

### Trade-offs

- ~36% slower than mimalloc on general single-thread workloads (per mimalloc benchmarks).
- Flat pagemap size is proportional to virtual address range used, not allocation count.
- Batched remote frees add latency to cross-thread deallocation (deferred processing).
- Complex C++ template-heavy implementation — harder to port or embed than rpmalloc.
- RSS characteristics not as well-documented as jemalloc.

### Security Features (< 5% overhead)

- 13 configurable hardening mitigations including free-list obfuscation, initial randomization, guard pages, out-of-band metadata, and bounds-checking memcpy.
- Full CHERI/strict-provenance support via the `CapPtr` type system.

---

## Cross-Cutting Comparison

### Size Class Systems

| Allocator | Small granularity | Classes per doubling | Max internal frag | Approach |
|-----------|------------------|---------------------|-------------------|----------|
| rpmalloc | 16 bytes | Variable | Fixed % per tier | Three tiers with variable granularity |
| mimalloc | 8 bytes | ~8 | ~12.5% | Per-page sharding eliminates cross-class interference |
| jemalloc | 8 bytes | 4 | ~20% | Formula: `(1<<lg_grp) + (ndelta<<lg_delta)` |
| snmalloc | 16 bytes | 4 | ~20% | Similar to jemalloc |

### Metadata Lookup (pointer → metadata)

| Allocator | Method | Cost |
|-----------|--------|------|
| rpmalloc | Address mask (`ptr & span_mask`) | O(1), zero shared state |
| mimalloc | Segment alignment (`ptr & ~(segment_size-1)`) | O(1), zero shared state |
| jemalloc | Radix tree (`emap`) | O(1), shared (read-only on fast path) |
| snmalloc | Flat pagemap array | O(1), shared (read-only on fast path) |

### Cross-Thread Deallocation

| Allocator | Mechanism | Atomics per free | Contention |
|-----------|-----------|-----------------|------------|
| rpmalloc | Per-span atomic deferred list | 1 CAS | Low (per-span) |
| mimalloc | Per-page atomic `thread_free` list | 1 CAS | Very low (sharded across thousands of pages) |
| jemalloc | tcache absorption → arena bin flush | 0 (tcache) or 1 lock (flush) | Medium (per-bin lock) |
| snmalloc | Batched message-passing to MPSC queue | 1 CAS per batch (amortized) | Very low (batched) |

### Memory Return to OS

| Allocator | Mechanism | Default behavior |
|-----------|-----------|-----------------|
| rpmalloc | Page decommit (develop), configurable cache modes | Performance mode: bounded caches |
| mimalloc | Eager purge, `MIMALLOC_PURGE_DELAY` (default 10ms) | Aggressive — returns quickly |
| jemalloc | Two-phase decay (MADV_FREE → MADV_DONTNEED), sigmoidal curve | 10s dirty + 10s muzzy |
| snmalloc | PAL `notify_not_using()` | Returns on slab freeing |

### Locking Strategy

| Allocator | Fast-path locks | Slow-path locks | Cross-thread locks |
|-----------|----------------|-----------------|-------------------|
| rpmalloc | None | None | None (atomic-only) |
| mimalloc | None | None | None (atomic CAS) |
| jemalloc | None (tcache) | Per-bin mutex | Per-bin mutex |
| snmalloc | None | Global buddy (rare) | None (message queue) |

### Thread Cache Design

| Allocator | Structure | GC strategy |
|-----------|-----------|-------------|
| rpmalloc | Active span + partial spans + free spans + global cache | Configurable cache modes |
| mimalloc | Per-page free list (inherent in page-based design) | Temporal cadence (bounded allocation count triggers generic path) |
| jemalloc | Contiguous stack per size class (`cache_bin_t`) | Exponential decay, incremental GC |
| snmalloc | `LocalCache` array per size class | Processed on slow-path entry |

---

## Benchmark Landscape

Performance varies heavily by workload. General trends:

| Workload | Best performer(s) | Notes |
|----------|------------------|-------|
| Single-thread small allocs | mimalloc, rpmalloc | Fewest branches on fast path |
| Multi-thread small allocs | mimalloc, snmalloc | Free-list sharding / message passing |
| Producer-consumer (cross-thread) | snmalloc, rpmalloc | Message batching / deferred processing |
| Server workloads (many threads) | jemalloc | Arena isolation, mature tuning |
| Long-running RSS management | jemalloc, mimalloc | Decay purging / eager purge |
| Fragmentation resistance | jemalloc | Lowest-address-first, extent merging |
| Cache-thrash resistance | mimalloc | >18x faster than jemalloc/tcmalloc |
| Database workloads | jemalloc | Proven in Redis, MySQL, Cassandra |
| Minimal overhead / embedded | rpmalloc | 2 files, public domain, simple |

---

## Summary: Each Allocator's Core Thesis

- **rpmalloc**: "Eliminate all locks. Let each thread own its memory. Keep it simple." — Optimizes for zero-contention throughput with minimal complexity.

- **mimalloc**: "Shard the free lists per page to maximize locality." — Three-free-list-per-page design clusters temporally related allocations, enabling aggressive page return.

- **jemalloc**: "Probabilistic arena isolation + production observability." — Targets fragmentation avoidance and operational diagnostics for long-running production services.

- **snmalloc**: "Treat cross-thread deallocation as message-passing." — Batches remote frees for amortized O(1) cross-thread overhead, with strong security guarantees.

//! Allocation statistics counters.
//!
//! All counters use `Relaxed` ordering — they are observational only and not
//! used as synchronization primitives. The allocator's own spinlocks provide
//! the ordering guarantees for correctness; these counters are purely for
//! monitoring.
//!
//! # Usage
//!
//! ```ignore
//! let snap = rtmalloc::stats::snapshot();
//! println!("allocs: {}", snap.alloc_count);
//! ```
//!
//! Obtain a [`Snapshot`] with [`snapshot()`]. Individual counter loads are
//! individually atomic but not globally consistent with each other.

use core::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct Stats {
    // ---- Global allocation stats ----
    /// Total calls to alloc with size > 0.
    pub alloc_count: AtomicU64,
    /// Total calls to dealloc with size > 0.
    pub dealloc_count: AtomicU64,
    /// Total calls to realloc (after null/zero-size guards).
    pub realloc_count: AtomicU64,
    /// Sum of all requested byte sizes passed to alloc.
    pub alloc_bytes: AtomicU64,

    // ---- Cache tier breakdown ----
    /// Allocations served from thread/CPU cache (fast path, no lock).
    pub thread_cache_hits: AtomicU64,
    /// Allocations that fell through to central/page heap (slow path).
    pub thread_cache_misses: AtomicU64,
    /// Allocations served by the central free list.
    pub central_cache_hits: AtomicU64,
    /// Large allocations going directly to the page heap.
    pub page_heap_allocs: AtomicU64,

    // ---- Page heap / OS ----
    /// Calls to `platform::page_alloc`.
    pub os_alloc_count: AtomicU64,
    /// Bytes requested from the OS via `platform::page_alloc`.
    pub os_alloc_bytes: AtomicU64,
    /// Times `carve_span` produced a remainder (i.e. a span was split).
    pub span_splits: AtomicU64,
    /// Times `coalesce_left` or `coalesce_right` merged two adjacent spans.
    pub span_coalesces: AtomicU64,
}

impl Stats {
    const fn new() -> Self {
        Self {
            alloc_count: AtomicU64::new(0),
            dealloc_count: AtomicU64::new(0),
            realloc_count: AtomicU64::new(0),
            alloc_bytes: AtomicU64::new(0),
            thread_cache_hits: AtomicU64::new(0),
            thread_cache_misses: AtomicU64::new(0),
            central_cache_hits: AtomicU64::new(0),
            page_heap_allocs: AtomicU64::new(0),
            os_alloc_count: AtomicU64::new(0),
            os_alloc_bytes: AtomicU64::new(0),
            span_splits: AtomicU64::new(0),
            span_coalesces: AtomicU64::new(0),
        }
    }
}

pub(crate) static STATS: Stats = Stats::new();

/// A point-in-time snapshot of all allocation statistics.
///
/// Fields are plain `u64` values loaded from the global atomic counters.
/// Individual fields are each atomically read, but the snapshot as a whole
/// is not globally consistent — concurrent allocations may race between loads.
/// For monitoring purposes this is always sufficient.
///
/// Obtain a snapshot with [`snapshot()`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    /// Total calls to alloc with size > 0.
    pub alloc_count: u64,
    /// Total calls to dealloc with size > 0.
    pub dealloc_count: u64,
    /// Total calls to realloc (after null/zero-size guards).
    pub realloc_count: u64,
    /// Sum of all requested byte sizes passed to alloc.
    pub alloc_bytes: u64,
    /// Allocations served from thread/CPU cache (fast path, no lock).
    pub thread_cache_hits: u64,
    /// Allocations that fell through to central/page heap (slow path).
    pub thread_cache_misses: u64,
    /// Allocations served by the central free list.
    pub central_cache_hits: u64,
    /// Large allocations going directly to the page heap.
    pub page_heap_allocs: u64,
    /// Calls to `platform::page_alloc`.
    pub os_alloc_count: u64,
    /// Bytes requested from the OS via `platform::page_alloc`.
    pub os_alloc_bytes: u64,
    /// Times a span was split (carve_span produced a remainder).
    pub span_splits: u64,
    /// Times two adjacent free spans were merged.
    pub span_coalesces: u64,
}

/// Load all counters with `Relaxed` ordering and return a [`Snapshot`].
pub fn snapshot() -> Snapshot {
    let s = &STATS;
    Snapshot {
        alloc_count: s.alloc_count.load(Ordering::Relaxed),
        dealloc_count: s.dealloc_count.load(Ordering::Relaxed),
        realloc_count: s.realloc_count.load(Ordering::Relaxed),
        alloc_bytes: s.alloc_bytes.load(Ordering::Relaxed),
        thread_cache_hits: s.thread_cache_hits.load(Ordering::Relaxed),
        thread_cache_misses: s.thread_cache_misses.load(Ordering::Relaxed),
        central_cache_hits: s.central_cache_hits.load(Ordering::Relaxed),
        page_heap_allocs: s.page_heap_allocs.load(Ordering::Relaxed),
        os_alloc_count: s.os_alloc_count.load(Ordering::Relaxed),
        os_alloc_bytes: s.os_alloc_bytes.load(Ordering::Relaxed),
        span_splits: s.span_splits.load(Ordering::Relaxed),
        span_coalesces: s.span_coalesces.load(Ordering::Relaxed),
    }
}

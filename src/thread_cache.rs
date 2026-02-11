//! Thread Cache (front-end): per-thread free lists for lock-free allocation.
//!
//! Each thread gets its own ThreadCache via `thread_local!`. The fast path
//! (thread cache hit) requires zero synchronization. When the thread cache
//! is empty or full, it batches transfers to/from the central free list.

use crate::central_free_list::CentralCache;
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class::{self, NUM_SIZE_CLASSES};
use crate::span::FreeObject;
use crate::sync::SpinMutex;
use crate::transfer_cache::TransferCacheArray;
use core::ptr;
use core::sync::atomic::{AtomicIsize, Ordering};

/// Overall thread cache budget shared across all threads (32 MiB).
const OVERALL_THREAD_CACHE_SIZE: usize = 32 * 1024 * 1024;

/// Minimum per-thread cache size (512 KiB).
const MIN_PER_THREAD_CACHE_SIZE: usize = 512 * 1024;

/// Amount to steal from global budget during scavenge (64 KiB).
const STEAL_AMOUNT: usize = 64 * 1024;

/// Maximum dynamic free list length per size class.
const MAX_DYNAMIC_FREE_LIST_LENGTH: u32 = 8192;

/// Number of consecutive overages before shrinking max_length (gperftools: 3).
const MAX_OVERAGES: u32 = 3;

/// Unclaimed cache budget available for thread caches to claim.
/// Starts at OVERALL_THREAD_CACHE_SIZE; each thread claims/returns portions.
static UNCLAIMED_CACHE_SPACE: AtomicIsize = AtomicIsize::new(OVERALL_THREAD_CACHE_SIZE as isize);

/// Per-size-class free list within the thread cache.
struct FreeList {
    /// Head of the singly-linked intrusive free list.
    head: *mut FreeObject,
    /// Number of objects currently in this list.
    length: u32,
    /// Maximum length before we return objects to central cache.
    max_length: u32,
    /// Consecutive overage count (for shrinking max_length).
    length_overages: u32,
    /// Minimum length since last scavenge (low-water mark).
    /// Objects above this level were never needed and are safe to release.
    low_water_mark: u32,
}

impl FreeList {
    const fn new() -> Self {
        Self {
            head: ptr::null_mut(),
            length: 0,
            max_length: 1, // Start small, grows adaptively
            length_overages: 0,
            low_water_mark: 0,
        }
    }

    #[inline]
    fn pop(&mut self) -> *mut FreeObject {
        let obj = self.head;
        if !obj.is_null() {
            self.head = unsafe { (*obj).next };
            self.length -= 1;
            if self.length < self.low_water_mark {
                self.low_water_mark = self.length;
            }
        }
        obj
    }

    #[inline]
    fn push(&mut self, obj: *mut FreeObject) {
        unsafe { (*obj).next = self.head };
        self.head = obj;
        self.length += 1;
    }

    /// Push a linked list of `count` objects.
    fn push_batch(&mut self, head: *mut FreeObject, count: u32) {
        if head.is_null() || count == 0 {
            return;
        }
        // Find the tail of the batch
        let mut tail = head;
        for _ in 1..count {
            let next = unsafe { (*tail).next };
            if next.is_null() {
                break;
            }
            tail = next;
        }
        unsafe { (*tail).next = self.head };
        self.head = head;
        self.length += count;
    }

    /// Pop up to `count` objects into a linked list. Returns (actual_count, head, tail).
    fn pop_batch(&mut self, count: u32) -> (u32, *mut FreeObject, *mut FreeObject) {
        let mut head: *mut FreeObject = ptr::null_mut();
        let mut tail: *mut FreeObject = ptr::null_mut();
        let mut popped = 0u32;
        while popped < count && !self.head.is_null() {
            let obj = self.head;
            self.head = unsafe { (*obj).next };
            unsafe { (*obj).next = head };
            if tail.is_null() {
                tail = obj; // First popped becomes tail after reversal
            }
            head = obj;
            self.length -= 1;
            popped += 1;
        }
        (popped, head, tail)
    }
}

/// Per-thread cache holding free lists for each size class.
pub struct ThreadCache {
    lists: [FreeList; NUM_SIZE_CLASSES],
    /// Total bytes cached across all size classes.
    total_size: usize,
    /// Per-thread cache size limit.
    max_size: usize,
}

impl ThreadCache {
    /// Const-constructible ThreadCache with `max_size = 0` as "not initialized" sentinel.
    /// Used with `#[thread_local]` for zero-cost TLS. Call `init()` before first use.
    pub const fn new_const() -> Self {
        Self {
            lists: [const { FreeList::new() }; NUM_SIZE_CLASSES],
            total_size: 0,
            max_size: 0, // Sentinel: not yet initialized
        }
    }

    pub fn new() -> Self {
        // Claim initial budget from global pool
        UNCLAIMED_CACHE_SPACE.fetch_sub(MIN_PER_THREAD_CACHE_SIZE as isize, Ordering::Relaxed);

        Self {
            lists: [const { FreeList::new() }; NUM_SIZE_CLASSES],
            total_size: 0,
            max_size: MIN_PER_THREAD_CACHE_SIZE,
        }
    }

    /// Check if this thread cache has been initialized (max_size > 0).
    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        self.max_size > 0
    }

    /// Initialize a const-constructed ThreadCache. Claims budget from global pool.
    #[cold]
    pub fn init(&mut self) {
        UNCLAIMED_CACHE_SPACE.fetch_sub(MIN_PER_THREAD_CACHE_SIZE as isize, Ordering::Relaxed);
        self.max_size = MIN_PER_THREAD_CACHE_SIZE;
    }

    /// Flush all cached objects back to the central cache and return budget.
    /// Called on thread exit via the TcFlush guard.
    pub unsafe fn flush_and_destroy(
        &mut self,
        transfer_cache: &TransferCacheArray,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        for cls in 1..NUM_SIZE_CLASSES {
            let list = &mut self.lists[cls];
            if list.length > 0 {
                let info = size_class::class_info(cls);
                let (count, head, tail) = list.pop_batch(list.length);
                if count > 0 {
                    self.total_size -= count as usize * info.size;
                    unsafe {
                        transfer_cache.insert_range(
                            cls, head, tail, count as usize,
                            central, page_heap, pagemap,
                        )
                    };
                }
            }
        }
        // Return budget to global pool
        if self.max_size > 0 {
            UNCLAIMED_CACHE_SPACE.fetch_add(self.max_size as isize, Ordering::Relaxed);
            self.max_size = 0;
        }
    }

    /// Allocate an object of the given size class.
    /// Returns null if allocation fails.
    #[inline]
    pub unsafe fn allocate(
        &mut self,
        size_class: usize,
        transfer_cache: &TransferCacheArray,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) -> *mut u8 {
        let list = &mut self.lists[size_class];
        let obj = list.pop();
        if !obj.is_null() {
            let obj_size = size_class::class_to_size(size_class);
            self.total_size -= obj_size;
            return obj as *mut u8;
        }
        // Slow path: fetch from transfer cache / central cache
        unsafe { self.fetch_from_central(size_class, transfer_cache, central, page_heap, pagemap) }
    }

    /// Deallocate an object of the given size class.
    #[inline]
    pub unsafe fn deallocate(
        &mut self,
        ptr: *mut u8,
        size_class: usize,
        transfer_cache: &TransferCacheArray,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        let list = &mut self.lists[size_class];
        let obj = ptr as *mut FreeObject;
        list.push(obj);

        let obj_size = size_class::class_to_size(size_class);
        self.total_size += obj_size;

        // Check if we should return objects to transfer/central cache
        if list.length > list.max_length {
            unsafe { self.release_to_central(size_class, transfer_cache, central, page_heap, pagemap) };
        }

        // Check total cache size for GC
        if self.total_size > self.max_size {
            unsafe { self.scavenge(transfer_cache, central, page_heap, pagemap) };
        }
    }

    /// Slow path: fetch a batch of objects from the transfer cache / central free list.
    ///
    /// Uses slow-start: fetches min(max_length, batch_size) objects and
    /// grows max_length on each slow-path call, matching Google tcmalloc.
    #[cold]
    unsafe fn fetch_from_central(
        &mut self,
        size_class: usize,
        transfer_cache: &TransferCacheArray,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) -> *mut u8 {
        let info = size_class::class_info(size_class);
        let batch = info.batch_size;
        let list = &mut self.lists[size_class];

        // Slow start: only fetch min(max_length, batch) objects
        let num_to_move = (list.max_length as usize).min(batch).max(1);

        let (count, head) = unsafe {
            transfer_cache.remove_range(size_class, num_to_move, central, page_heap, pagemap)
        };

        if count == 0 || head.is_null() {
            return ptr::null_mut();
        }

        // Take the first object for the caller
        let result = head;
        let remaining_head = unsafe { (*head).next };
        let remaining_count = count - 1;

        // Put the rest in our thread-local free list
        if remaining_count > 0 {
            list.push_batch(remaining_head, remaining_count as u32);
            self.total_size += remaining_count * info.size;
        }

        // Grow max_length: slow start then linear growth
        Self::grow_max_length_on_fetch(list, batch);

        result as *mut u8
    }

    /// Release excess objects from a size class back to transfer/central cache.
    ///
    /// Matches Google tcmalloc's ListTooLong:
    /// - Release exactly batch_size objects
    /// - Slow start: grow max_length while < batch_size
    /// - After that, track overages and shrink max_length after MAX_OVERAGES
    unsafe fn release_to_central(
        &mut self,
        size_class: usize,
        transfer_cache: &TransferCacheArray,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        let info = size_class::class_info(size_class);
        let batch = info.batch_size as u32;
        let list = &mut self.lists[size_class];

        // Release exactly batch_size objects (or all if fewer)
        let to_release = batch.min(list.length);
        if to_release == 0 {
            return;
        }

        let (count, head, tail) = list.pop_batch(to_release);
        self.total_size -= count as usize * info.size;

        unsafe {
            transfer_cache.insert_range(
                size_class, head, tail, count as usize,
                central, page_heap, pagemap,
            )
        };

        // Adjust max_length per gperftools logic:
        if list.max_length < batch {
            // Slow start: grow by 1
            list.max_length += 1;
        } else if list.max_length > batch {
            // Track overages: if we keep overflowing, shrink max_length
            list.length_overages += 1;
            if list.length_overages > MAX_OVERAGES {
                list.max_length = list.max_length.saturating_sub(batch).max(batch);
                list.length_overages = 0;
            }
        }
    }

    /// Grow max_length on fetch: slow-start then linear growth.
    /// Matches gperftools FetchFromCentralCache growth logic.
    #[inline]
    fn grow_max_length_on_fetch(list: &mut FreeList, batch_size: usize) {
        if (list.max_length as usize) < batch_size {
            list.max_length += 1;
        } else {
            let batch = batch_size as u32;
            let new_len = list.max_length + batch;
            // Round down to multiple of batch_size (per gperftools)
            let new_len = new_len - (new_len % batch);
            list.max_length = new_len.min(MAX_DYNAMIC_FREE_LIST_LENGTH);
        }
        list.length_overages = 0;
    }

    /// GC: release idle objects across all size classes.
    ///
    /// Uses low-water-mark scavenging (matches gperftools): only releases objects
    /// above the minimum list length since the last scavenge. These objects were
    /// never needed and are safe to release without causing re-fetches.
    unsafe fn scavenge(
        &mut self,
        transfer_cache: &TransferCacheArray,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        for cls in 1..NUM_SIZE_CLASSES {
            let list = &mut self.lists[cls];
            let lwm = list.low_water_mark;

            if lwm > 0 {
                // Release half the idle objects (above low-water mark)
                let to_release = if lwm > 1 { lwm / 2 } else { 1 };

                let info = size_class::class_info(cls);
                let (count, head, tail) = list.pop_batch(to_release);
                self.total_size -= count as usize * info.size;

                unsafe {
                    transfer_cache.insert_range(
                        cls, head, tail, count as usize,
                        central, page_heap, pagemap,
                    )
                };
            }

            // Shrink max_length if it's grown beyond batch_size
            let batch = size_class::class_info(cls).batch_size as u32;
            if list.max_length > batch {
                list.max_length = list.max_length.saturating_sub(batch).max(batch);
            }

            // Reset low-water mark for next epoch
            list.low_water_mark = list.length;
        }

        // After scavenging, try to grow our budget so we don't scavenge as often.
        // Active threads that allocate heavily will naturally grow their caches.
        self.increase_cache_limit();
    }

    /// Try to steal budget from the global pool to grow this thread's cache.
    /// Uses CAS to atomically claim STEAL_AMOUNT from unclaimed space.
    fn increase_cache_limit(&mut self) {
        loop {
            let current = UNCLAIMED_CACHE_SPACE.load(Ordering::Relaxed);
            if current < STEAL_AMOUNT as isize {
                return; // Not enough budget available
            }
            match UNCLAIMED_CACHE_SPACE.compare_exchange_weak(
                current,
                current - STEAL_AMOUNT as isize,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.max_size += STEAL_AMOUNT;
                    return;
                }
                Err(_) => continue, // Retry
            }
        }
    }
}

// Note: Drop is NOT implemented. Thread cache cleanup is handled by
// the TcFlush guard in allocator.rs which calls flush_and_destroy().
// For ThreadCache::new() users (tests), budget leak is acceptable.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_heap::PageHeap;
    use crate::pagemap::PageMap;
    use crate::transfer_cache::TransferCacheArray;

    fn make_test_env() -> (
        &'static PageMap,
        SpinMutex<PageHeap>,
        CentralCache,
        TransferCacheArray,
    ) {
        let pm = Box::leak(Box::new(PageMap::new()));
        let heap = SpinMutex::new(PageHeap::new(pm));
        let cache = CentralCache::new();
        let xfer = TransferCacheArray::new();
        (pm, heap, cache, xfer)
    }

    #[test]
    fn test_allocate_and_deallocate() {
        let (pm, heap, central, xfer) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            // Allocate a small object (size class 1 = 8 bytes)
            let ptr = tc.allocate(1, &xfer, &central, &heap, pm);
            assert!(!ptr.is_null());

            // Deallocate it
            tc.deallocate(ptr, 1, &xfer, &central, &heap, pm);
        }
    }

    #[test]
    fn test_allocate_many() {
        let (pm, heap, central, xfer) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            let mut ptrs = Vec::new();
            // Allocate 1000 objects of size class 4 = 32 bytes
            for _ in 0..1000 {
                let ptr = tc.allocate(4, &xfer, &central, &heap, pm);
                assert!(!ptr.is_null());
                ptrs.push(ptr);
            }
            // Free them all
            for ptr in ptrs {
                tc.deallocate(ptr, 4, &xfer, &central, &heap, pm);
            }
        }
    }

    #[test]
    fn test_mixed_sizes() {
        let (pm, heap, central, xfer) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            let mut allocs: Vec<(usize, *mut u8)> = Vec::new();
            for cls in [1, 4, 8, 12, 16, 20, 24] {
                for _ in 0..50 {
                    let ptr = tc.allocate(cls, &xfer, &central, &heap, pm);
                    assert!(!ptr.is_null());
                    allocs.push((cls, ptr));
                }
            }
            for (cls, ptr) in allocs {
                tc.deallocate(ptr, cls, &xfer, &central, &heap, pm);
            }
        }
    }

    #[test]
    fn test_reuse_from_cache() {
        let (pm, heap, central, xfer) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            // Allocate and free to populate thread cache
            let ptr1 = tc.allocate(2, &xfer, &central, &heap, pm);
            assert!(!ptr1.is_null());
            tc.deallocate(ptr1, 2, &xfer, &central, &heap, pm);

            // Next allocation should come from thread cache (same pointer)
            let ptr2 = tc.allocate(2, &xfer, &central, &heap, pm);
            assert!(!ptr2.is_null());
            assert_eq!(ptr1, ptr2);

            tc.deallocate(ptr2, 2, &xfer, &central, &heap, pm);
        }
    }
}

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
use core::ptr;

/// Maximum total bytes a thread cache can hold before triggering GC.
const MAX_THREAD_CACHE_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

/// Maximum dynamic free list length per size class.
const MAX_DYNAMIC_FREE_LIST_LENGTH: u32 = 8192;

/// Number of consecutive overages before shrinking max_length (gperftools: 3).
const MAX_OVERAGES: u32 = 3;

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
}

impl FreeList {
    const fn new() -> Self {
        Self {
            head: ptr::null_mut(),
            length: 0,
            max_length: 1, // Start small, grows adaptively
            length_overages: 0,
        }
    }

    #[inline]
    fn pop(&mut self) -> *mut FreeObject {
        let obj = self.head;
        if !obj.is_null() {
            self.head = unsafe { (*obj).next };
            self.length -= 1;
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

    /// Pop up to `count` objects into a linked list. Returns (actual_count, head).
    fn pop_batch(&mut self, count: u32) -> (u32, *mut FreeObject) {
        let mut head: *mut FreeObject = ptr::null_mut();
        let mut popped = 0u32;
        while popped < count && !self.head.is_null() {
            let obj = self.head;
            self.head = unsafe { (*obj).next };
            unsafe { (*obj).next = head };
            head = obj;
            self.length -= 1;
            popped += 1;
        }
        (popped, head)
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
    pub fn new() -> Self {
        Self {
            lists: [const { FreeList::new() }; NUM_SIZE_CLASSES],
            total_size: 0,
            max_size: MAX_THREAD_CACHE_SIZE,
        }
    }

    /// Allocate an object of the given size class.
    /// Returns null if allocation fails.
    #[inline]
    pub unsafe fn allocate(
        &mut self,
        size_class: usize,
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
        // Slow path: fetch from central cache
        unsafe { self.fetch_from_central(size_class, central, page_heap, pagemap) }
    }

    /// Deallocate an object of the given size class.
    #[inline]
    pub unsafe fn deallocate(
        &mut self,
        ptr: *mut u8,
        size_class: usize,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        let list = &mut self.lists[size_class];
        let obj = ptr as *mut FreeObject;
        list.push(obj);

        let obj_size = size_class::class_to_size(size_class);
        self.total_size += obj_size;

        // Check if we should return objects to central cache
        if list.length > list.max_length {
            unsafe { self.release_to_central(size_class, central, page_heap, pagemap) };
        }

        // Check total cache size for GC
        if self.total_size > self.max_size {
            unsafe { self.scavenge(central, page_heap, pagemap) };
        }
    }

    /// Slow path: fetch a batch of objects from the central free list.
    ///
    /// Uses slow-start: fetches min(max_length, batch_size) objects and
    /// grows max_length on each slow-path call, matching Google tcmalloc.
    #[cold]
    unsafe fn fetch_from_central(
        &mut self,
        size_class: usize,
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
            central
                .get(size_class)
                .lock()
                .remove_range(num_to_move, page_heap, pagemap)
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

    /// Release excess objects from a size class back to central cache.
    ///
    /// Matches Google tcmalloc's ListTooLong:
    /// - Release exactly batch_size objects
    /// - Slow start: grow max_length while < batch_size
    /// - After that, track overages and shrink max_length after MAX_OVERAGES
    unsafe fn release_to_central(
        &mut self,
        size_class: usize,
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

        let (count, head) = list.pop_batch(to_release);
        self.total_size -= count as usize * info.size;

        unsafe {
            central
                .get(size_class)
                .lock()
                .insert_range(head, count as usize, page_heap, pagemap)
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

    /// GC: release objects across all size classes to bring total_size under max_size.
    unsafe fn scavenge(
        &mut self,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        // Target: bring total_size down to max_size / 2
        let target = self.max_size / 2;

        for cls in 1..NUM_SIZE_CLASSES {
            if self.total_size <= target {
                break;
            }

            let list = &mut self.lists[cls];
            if list.length == 0 {
                continue;
            }

            let info = size_class::class_info(cls);
            let to_release = list.length / 2;
            if to_release == 0 {
                continue;
            }

            let (count, head) = list.pop_batch(to_release);
            self.total_size -= count as usize * info.size;

            unsafe {
                central
                    .get(cls)
                    .lock()
                    .insert_range(head, count as usize, page_heap, pagemap)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_heap::PageHeap;
    use crate::pagemap::PageMap;

    fn make_test_env() -> (
        &'static PageMap,
        SpinMutex<PageHeap>,
        CentralCache,
    ) {
        let pm = Box::leak(Box::new(PageMap::new()));
        let heap = SpinMutex::new(PageHeap::new(pm));
        let cache = CentralCache::new();
        (pm, heap, cache)
    }

    #[test]
    fn test_allocate_and_deallocate() {
        let (pm, heap, central) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            // Allocate a small object (size class 1 = 8 bytes)
            let ptr = tc.allocate(1, &central, &heap, pm);
            assert!(!ptr.is_null());

            // Deallocate it
            tc.deallocate(ptr, 1, &central, &heap, pm);
        }
    }

    #[test]
    fn test_allocate_many() {
        let (pm, heap, central) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            let mut ptrs = Vec::new();
            // Allocate 1000 objects of size class 4 = 32 bytes
            for _ in 0..1000 {
                let ptr = tc.allocate(4, &central, &heap, pm);
                assert!(!ptr.is_null());
                ptrs.push(ptr);
            }
            // Free them all
            for ptr in ptrs {
                tc.deallocate(ptr, 4, &central, &heap, pm);
            }
        }
    }

    #[test]
    fn test_mixed_sizes() {
        let (pm, heap, central) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            let mut allocs: Vec<(usize, *mut u8)> = Vec::new();
            for cls in [1, 4, 8, 12, 16, 20, 24] {
                for _ in 0..50 {
                    let ptr = tc.allocate(cls, &central, &heap, pm);
                    assert!(!ptr.is_null());
                    allocs.push((cls, ptr));
                }
            }
            for (cls, ptr) in allocs {
                tc.deallocate(ptr, cls, &central, &heap, pm);
            }
        }
    }

    #[test]
    fn test_reuse_from_cache() {
        let (pm, heap, central) = make_test_env();
        let mut tc = ThreadCache::new();

        unsafe {
            // Allocate and free to populate thread cache
            let ptr1 = tc.allocate(2, &central, &heap, pm);
            assert!(!ptr1.is_null());
            tc.deallocate(ptr1, 2, &central, &heap, pm);

            // Next allocation should come from thread cache (same pointer)
            let ptr2 = tc.allocate(2, &central, &heap, pm);
            assert!(!ptr2.is_null());
            assert_eq!(ptr1, ptr2);

            tc.deallocate(ptr2, 2, &central, &heap, pm);
        }
    }
}

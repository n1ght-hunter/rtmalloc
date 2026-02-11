//! Central Free List (middle-end): per-size-class shared object pools.
//!
//! Each size class has its own CentralFreeList with its own lock (fine-grained).
//! The thread cache fetches/returns batches of objects from/to here.
//! When the central free list is empty, it requests a new span from the page heap
//! and carves it into objects.

use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class::{self, NUM_SIZE_CLASSES};
use crate::span::{FreeObject, SpanList, SpanState};
use crate::sync::SpinMutex;
use crate::PAGE_SHIFT;
use crate::PAGE_SIZE;
use core::ptr;

/// Central free list for a single size class.
pub struct CentralFreeList {
    /// Size class index this list manages.
    size_class: usize,
    /// Spans that have free objects available.
    nonempty_spans: SpanList,
    /// Total number of free objects across all spans.
    num_free: usize,
}

// SAFETY: Only accessed through external SpinMutex synchronization.
unsafe impl Send for CentralFreeList {}

impl CentralFreeList {
    pub const fn new(size_class: usize) -> Self {
        Self {
            size_class,
            nonempty_spans: SpanList::new(),
            num_free: 0,
        }
    }

    /// Remove up to `batch_size` objects from this central free list.
    /// Returns (count, head_of_linked_list).
    /// If the list is empty, fetches a new span from the page heap.
    pub unsafe fn remove_range(
        &mut self,
        batch_size: usize,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) -> (usize, *mut FreeObject) {
        if self.num_free == 0 {
            // Need to fetch a new span
            unsafe { self.populate(page_heap, pagemap) };
            if self.num_free == 0 {
                return (0, ptr::null_mut());
            }
        }

        // Collect objects from spans
        let mut head: *mut FreeObject = ptr::null_mut();
        let mut count = 0;

        while count < batch_size && !self.nonempty_spans.is_empty() {
            let span = self.nonempty_spans.head;
            unsafe {
                while count < batch_size && !(*span).freelist.is_null() {
                    let obj = (*span).freelist;
                    (*span).freelist = (*obj).next;
                    (*obj).next = head;
                    head = obj;
                    (*span).allocated_count += 1;
                    count += 1;
                    self.num_free -= 1;
                }

                // If span has no more free objects, remove from nonempty list
                if (*span).freelist.is_null() {
                    self.nonempty_spans.remove(span);
                }
            }
        }

        (count, head)
    }

    /// Insert a batch of objects back into the central free list.
    /// If any span becomes completely free, returns it to the page heap.
    pub unsafe fn insert_range(
        &mut self,
        mut head: *mut FreeObject,
        count: usize,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        let mut remaining = count;

        while !head.is_null() && remaining > 0 {
            let obj = head;
            unsafe { head = (*obj).next };
            remaining -= 1;

            // Find which span this object belongs to
            let page_id = (obj as usize) >> PAGE_SHIFT;
            let span = pagemap.get(page_id);
            if span.is_null() {
                continue; // Shouldn't happen, but be defensive
            }

            unsafe {
                let was_full = (*span).freelist.is_null();

                // Add object back to span's free list
                (*obj).next = (*span).freelist;
                (*span).freelist = obj;
                (*span).allocated_count -= 1;
                self.num_free += 1;

                // If span was previously full (not in nonempty list), add it back
                if was_full {
                    self.nonempty_spans.push(span);
                }

                // If span is completely free, return it to page heap
                if (*span).allocated_count == 0 {
                    self.nonempty_spans.remove(span);
                    // Drain the freelist count
                    self.num_free -= (*span).total_count as usize;

                    // Clear span fields before returning to page heap
                    (*span).freelist = ptr::null_mut();
                    page_heap.lock().deallocate_span(span);
                }
            }
        }
    }

    /// Fetch a new span from the page heap and carve it into objects.
    unsafe fn populate(&mut self, page_heap: &SpinMutex<PageHeap>, pagemap: &PageMap) {
        let info = size_class::class_info(self.size_class);
        let obj_size = info.size;
        let pages = info.pages;

        let span = unsafe { page_heap.lock().allocate_span(pages) };
        if span.is_null() {
            return;
        }

        unsafe {
            (*span).size_class = self.size_class;
            (*span).state = SpanState::InUse;

            // Register in pagemap (page_heap already did this, but update size_class)
            pagemap.register_span(span);

            // Carve the span into objects
            let base = (*span).start_addr();
            let span_bytes = (*span).num_pages * PAGE_SIZE;
            let num_objects = span_bytes / obj_size;

            (*span).total_count = num_objects as u32;
            (*span).allocated_count = 0;

            // Thread objects into an intrusive free list
            let mut freelist: *mut FreeObject = ptr::null_mut();
            for i in (0..num_objects).rev() {
                let obj = base.add(i * obj_size) as *mut FreeObject;
                (*obj).next = freelist;
                freelist = obj;
            }

            (*span).freelist = freelist;
            self.num_free += num_objects;
            self.nonempty_spans.push(span);
        }
    }
}

/// Array of central free lists, one per size class.
/// Each is individually locked for fine-grained concurrency.
pub struct CentralCache {
    lists: [SpinMutex<CentralFreeList>; NUM_SIZE_CLASSES],
}

// Can't use a simple const init with a loop for SpinMutex<CentralFreeList>,
// so we use a macro.
macro_rules! central_cache_init {
    ($($i:literal),* $(,)?) => {
        [$(SpinMutex::new(CentralFreeList::new($i))),*]
    };
}

impl CentralCache {
    pub const fn new() -> Self {
        Self {
            lists: central_cache_init![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
                10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
                20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
                30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
                40, 41, 42, 43, 44, 45,
            ],
        }
    }

    /// Get a reference to the central free list for a size class.
    #[inline]
    pub fn get(&self, size_class: usize) -> &SpinMutex<CentralFreeList> {
        &self.lists[size_class]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pagemap::PageMap;

    fn make_test_env() -> (&'static PageMap, SpinMutex<PageHeap>, CentralCache) {
        let pm = Box::leak(Box::new(PageMap::new()));
        let heap = SpinMutex::new(PageHeap::new(pm));
        let cache = CentralCache::new();
        (pm, heap, cache)
    }

    #[test]
    fn test_remove_range_populates() {
        let (pm, heap, cache) = make_test_env();
        // Size class 1 = 8 bytes
        let mut cfl = cache.get(1).lock();
        unsafe {
            let (count, head) = cfl.remove_range(32, &heap, pm);
            assert!(count > 0);
            assert!(!head.is_null());

            // Walk the list and verify count
            let mut node = head;
            let mut actual = 0;
            while !node.is_null() {
                actual += 1;
                node = (*node).next;
            }
            assert_eq!(actual, count);
        }
    }

    #[test]
    fn test_insert_range_returns() {
        let (pm, heap, cache) = make_test_env();
        // Use size class 2 = 16 bytes
        let mut cfl = cache.get(2).lock();
        unsafe {
            // First get some objects
            let (count, head) = cfl.remove_range(16, &heap, pm);
            assert!(count > 0);

            // Return them
            cfl.insert_range(head, count, &heap, pm);
        }
    }

    #[test]
    fn test_remove_insert_cycle() {
        let (pm, heap, cache) = make_test_env();
        // Use size class 8 = 64 bytes
        let mut cfl = cache.get(8).lock();
        unsafe {
            for _ in 0..10 {
                let (count, head) = cfl.remove_range(4, &heap, pm);
                assert!(count > 0);
                cfl.insert_range(head, count, &heap, pm);
            }
        }
    }
}

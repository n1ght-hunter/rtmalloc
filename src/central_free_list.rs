//! Central Free List (middle-end): per-size-class shared object pools.
//!
//! Each size class has its own CentralFreeList with its own lock (fine-grained).
//! The thread cache fetches/returns batches of objects from/to here.
//! When the central free list is empty, it requests a new span from the page heap
//! and carves it into objects.

use crate::config::{PAGE_SIZE, PAGE_SHIFT};
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class::{self, NUM_SIZE_CLASSES};
use crate::span::{FreeObject, Span, SpanList, SpanState};
use crate::sync::SpinMutex;
use core::ptr;
#[cfg(feature = "debug")]
use std::println;

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
    ///
    /// # Safety
    ///
    /// Caller must hold exclusive access (via the enclosing `SpinMutex`).
    /// `page_heap` and `pagemap` must be the global instances.
    pub unsafe fn remove_range(
        &mut self,
        batch_size: usize,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) -> (usize, *mut FreeObject) {
        let mut head: *mut FreeObject = ptr::null_mut();
        let mut count = 0;

        while count < batch_size {
            if self.nonempty_spans.is_empty() {
                unsafe { self.populate(page_heap, pagemap) };
                if self.nonempty_spans.is_empty() {
                    break; // OOM or can't grow
                }
            }

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

                if (*span).freelist.is_null() {
                    self.nonempty_spans.remove(span);
                }
            }
        }

        (count, head)
    }

    /// Insert a batch of objects back into the central free list.
    /// If any span becomes completely free, returns it to the page heap.
    ///
    /// # Safety
    ///
    /// `head` must point to a valid linked list of `count` `FreeObject`s
    /// that were previously allocated from this allocator.
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

                // If span is completely free, return it to page heap.
                // Keep at least one span cached to avoid populate/return churn
                // on single alloc+dealloc cycles.
                if (*span).allocated_count == 0 && self.nonempty_spans.count > 1 {
                    self.nonempty_spans.remove(span);
                    self.num_free -= (*span).total_count as usize;
                    (*span).freelist = ptr::null_mut();
                    page_heap.lock().deallocate_span(span);
                }
            }
        }
    }

    /// Fetch a new span from the page heap and carve it into objects.
    unsafe fn populate(&mut self, page_heap: &SpinMutex<PageHeap>, pagemap: &PageMap) {
        let info = size_class::class_info(self.size_class);
        let span = unsafe { page_heap.lock().allocate_span(info.pages) };
        if span.is_null() {
            return;
        }
        unsafe { self.inject_span(span, pagemap) };
    }

    /// Carve a pre-allocated span into objects and add to the nonempty list.
    /// Called while holding the central lock.
    unsafe fn inject_span(&mut self, span: *mut Span, pagemap: &PageMap) {
        let info = size_class::class_info(self.size_class);
        let obj_size = info.size;

        unsafe {
            (*span).size_class = self.size_class;
            (*span).state = SpanState::InUse;

            #[cfg(feature = "debug")]
            println!("[inject] register_span");

            pagemap.register_span(span);

            let base = (*span).start_addr();
            let span_bytes = (*span).num_pages * PAGE_SIZE;
            let num_objects = span_bytes / obj_size;

            #[cfg(feature = "debug")]
            println!("[inject] build freelist");

            (*span).total_count = num_objects as u32;
            (*span).allocated_count = 0;

            let mut freelist: *mut FreeObject = ptr::null_mut();
            for i in (0..num_objects).rev() {
                let obj = base.add(i * obj_size) as *mut FreeObject;
                (*obj).next = freelist;
                freelist = obj;
            }

            #[cfg(feature = "debug")]
            println!("[inject] done");

            (*span).freelist = freelist;
            self.num_free += num_objects;
            self.nonempty_spans.push(span);
        }
    }
}

/// Remove up to `batch_size` objects, dropping the central lock during page heap calls.
///
/// This prevents threads wanting the same size class from blocking while another
/// thread waits for OS memory in VirtualAlloc/mmap.
///
/// # Safety
///
/// `page_heap` and `pagemap` must be the global instances.
pub unsafe fn remove_range_dropping_lock(
    cfl_lock: &SpinMutex<CentralFreeList>,
    size_class: usize,
    batch_size: usize,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) -> (usize, *mut FreeObject) {
    let info = size_class::class_info(size_class);
    let mut head: *mut FreeObject = ptr::null_mut();
    let mut count = 0;

    loop {
        // Phase 1: Collect from existing spans (central lock held)
        {
            let mut cfl = cfl_lock.lock();

            while count < batch_size && !cfl.nonempty_spans.is_empty() {
                let span = cfl.nonempty_spans.head;
                unsafe {
                    while count < batch_size && !(*span).freelist.is_null() {
                        let obj = (*span).freelist;
                        (*span).freelist = (*obj).next;
                        (*obj).next = head;
                        head = obj;
                        (*span).allocated_count += 1;
                        count += 1;
                        cfl.num_free -= 1;
                    }
                    if (*span).freelist.is_null() {
                        cfl.nonempty_spans.remove(span);
                    }
                }
            }

            if count >= batch_size {
                return (count, head);
            }

            // nonempty_spans empty -- need to populate
            // Central lock drops here
        }

        // Phase 2: Allocate span from page heap (NO central lock held)
        let span = unsafe { page_heap.lock().allocate_span(info.pages) };
        if span.is_null() {
            return (count, head); // OOM, return what we have
        }

        // Phase 3: Inject span under central lock
        {
            let mut cfl = cfl_lock.lock();
            unsafe { cfl.inject_span(span, pagemap) };
        }
    }
}

/// Insert objects back, dropping the central lock for page heap span deallocation.
///
/// # Safety
///
/// `head` must point to a valid linked list of `count` `FreeObject`s.
pub unsafe fn insert_range_dropping_lock(
    cfl_lock: &SpinMutex<CentralFreeList>,
    mut head: *mut FreeObject,
    count: usize,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) {
    const MAX_FREED: usize = 8;
    let mut freed_spans: [*mut Span; MAX_FREED] = [ptr::null_mut(); MAX_FREED];
    let mut num_freed = 0;

    // Phase 1: Insert all objects (central lock held)
    {
        let mut cfl = cfl_lock.lock();
        let mut remaining = count;

        while !head.is_null() && remaining > 0 {
            let obj = head;
            unsafe { head = (*obj).next };
            remaining -= 1;

            let page_id = (obj as usize) >> PAGE_SHIFT;
            let span = pagemap.get(page_id);
            if span.is_null() {
                continue;
            }

            unsafe {
                let was_full = (*span).freelist.is_null();

                (*obj).next = (*span).freelist;
                (*span).freelist = obj;
                (*span).allocated_count -= 1;
                cfl.num_free += 1;

                if was_full {
                    cfl.nonempty_spans.push(span);
                }

                // Keep at least one span cached to avoid populate/return churn
                if (*span).allocated_count == 0 && cfl.nonempty_spans.count > 1 {
                    cfl.nonempty_spans.remove(span);
                    cfl.num_free -= (*span).total_count as usize;
                    (*span).freelist = ptr::null_mut();

                    if num_freed < MAX_FREED {
                        freed_spans[num_freed] = span;
                        num_freed += 1;
                    } else {
                        page_heap.lock().deallocate_span(span);
                    }
                }
            }
        }
    }
    // Central lock dropped

    // Phase 2: Return freed spans to page heap (NO central lock held)
    for span in freed_spans.iter().take(num_freed) {
        unsafe { page_heap.lock().deallocate_span(*span) };
    }
}

/// Array of central free lists, one per size class.
/// Each is individually locked for fine-grained concurrency.
pub struct CentralCache {
    lists: [SpinMutex<CentralFreeList>; NUM_SIZE_CLASSES],
}

impl Default for CentralCache {
    fn default() -> Self {
        Self::new()
    }
}

impl CentralCache {
    pub const fn new() -> Self {
        let mut lists = [const { SpinMutex::new(CentralFreeList::new(0)) }; NUM_SIZE_CLASSES];
        let mut i = 0;
        while i < NUM_SIZE_CLASSES {
            lists[i] = SpinMutex::new(CentralFreeList::new(i));
            i += 1;
        }
        Self { lists }
    }

    /// Get a reference to the central free list for a size class.
    #[inline]
    pub fn get(&self, size_class: usize) -> &SpinMutex<CentralFreeList> {
        &self.lists[size_class]
    }
}

#[cfg(test)]
mod tests {
    use std::boxed::Box;

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

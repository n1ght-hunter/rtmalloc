//! Page Heap (back-end): manages spans of contiguous pages.
//!
//! Responsibilities:
//! - Allocate spans of N pages (searching free lists, splitting larger spans)
//! - Deallocate spans (coalescing with adjacent free spans)
//! - Grow the heap by requesting memory from the OS
//! - Register/unregister spans in the page map

use crate::pagemap::PageMap;
use crate::platform;
use crate::span::{self, Span, SpanList, SpanState};
use crate::PAGE_SHIFT;
use crate::PAGE_SIZE;
use core::ptr;

/// Maximum number of pages tracked in the free list array.
/// Spans larger than this go into `large_spans`.
const MAX_PAGES: usize = 128;

pub struct PageHeap {
    /// free_lists[k] holds free spans of exactly k pages (index 0 unused).
    free_lists: [SpanList; MAX_PAGES + 1],
    /// Free spans larger than MAX_PAGES pages.
    large_spans: SpanList,
    /// Reference to the global page map.
    pagemap: &'static PageMap,
}

// SAFETY: PageHeap is only accessed through a SpinMutex. Raw pointers within
// point to OS-allocated memory that outlives any thread.
unsafe impl Send for PageHeap {}

/// Helper to create a const array of SpanLists.
const fn new_span_list_array<const N: usize>() -> [SpanList; N] {
    let mut arr = [const { SpanList::new() }; N];
    let mut i = 0;
    while i < N {
        arr[i] = SpanList::new();
        i += 1;
    }
    arr
}

impl PageHeap {
    pub const fn new(pagemap: &'static PageMap) -> Self {
        Self {
            free_lists: new_span_list_array(),
            large_spans: SpanList::new(),
            pagemap,
        }
    }

    /// Allocate a span of at least `num_pages` pages.
    /// Returns a pointer to the Span, or null on failure.
    pub unsafe fn allocate_span(&mut self, num_pages: usize) -> *mut Span {
        assert!(num_pages > 0);

        // Search free lists for an exact or larger match
        if num_pages <= MAX_PAGES {
            // Try exact match first, then larger
            for n in num_pages..=MAX_PAGES {
                if !self.free_lists[n].is_empty() {
                    let s = unsafe { self.free_lists[n].pop() };
                    return unsafe { self.carve_span(s, num_pages) };
                }
            }
        }

        // Search large spans (best-fit)
        let best = unsafe { self.find_best_large_span(num_pages) };
        if !best.is_null() {
            unsafe { self.large_spans.remove(best) };
            return unsafe { self.carve_span(best, num_pages) };
        }

        // Nothing in free lists. Grow the heap from the OS.
        unsafe { self.grow_heap(num_pages) }
    }

    /// Deallocate a span, returning it to the free lists.
    /// Attempts to coalesce with adjacent free spans.
    pub unsafe fn deallocate_span(&mut self, span: *mut Span) {
        unsafe {
            (*span).state = SpanState::Free;
            (*span).size_class = 0;
            (*span).freelist = ptr::null_mut();
            (*span).allocated_count = 0;
            (*span).total_count = 0;
        }

        // Try to coalesce with the span before this one
        let span = unsafe { self.coalesce_left(span) };
        // Try to coalesce with the span after this one
        let span = unsafe { self.coalesce_right(span) };

        // Register the (possibly merged) span in the pagemap
        unsafe { self.pagemap.register_span(span) };

        // Insert into the appropriate free list
        unsafe { self.insert_free(span) };
    }

    /// Split a span: use the first `num_pages` pages, return the remainder
    /// to the free lists. Returns the (now in-use) span.
    unsafe fn carve_span(&mut self, span: *mut Span, num_pages: usize) -> *mut Span {
        let total = unsafe { (*span).num_pages };
        assert!(total >= num_pages);

        if total > num_pages {
            // Create a remainder span
            let remainder = span::alloc_span();
            if remainder.is_null() {
                // Can't allocate span metadata - return the whole thing
                unsafe {
                    (*span).state = SpanState::InUse;
                    self.pagemap.register_span(span);
                }
                return span;
            }

            unsafe {
                (*remainder).start_page = (*span).start_page + num_pages;
                (*remainder).num_pages = total - num_pages;
                (*remainder).state = SpanState::Free;

                // Update original span
                (*span).num_pages = num_pages;

                // Register both in pagemap
                self.pagemap.register_span(remainder);
                self.insert_free(remainder);
            }
        }

        unsafe {
            (*span).state = SpanState::InUse;
            self.pagemap.register_span(span);
        }
        span
    }

    /// Insert a free span into the appropriate free list.
    unsafe fn insert_free(&mut self, span: *mut Span) {
        let n = unsafe { (*span).num_pages };
        if n <= MAX_PAGES {
            unsafe { self.free_lists[n].push(span) };
        } else {
            unsafe { self.large_spans.push(span) };
        }
    }

    /// Find the best-fit span in large_spans that has >= num_pages.
    unsafe fn find_best_large_span(&self, num_pages: usize) -> *mut Span {
        let mut best: *mut Span = ptr::null_mut();
        let mut best_pages = usize::MAX;
        let mut current = self.large_spans.head;

        while !current.is_null() {
            let n = unsafe { (*current).num_pages };
            if n >= num_pages && n < best_pages {
                best = current;
                best_pages = n;
                if n == num_pages {
                    break; // Exact match
                }
            }
            current = unsafe { (*current).next };
        }
        best
    }

    /// Request pages from the OS and create a new span.
    unsafe fn grow_heap(&mut self, num_pages: usize) -> *mut Span {
        // Allocate at least 128 pages (1 MiB) at a time to reduce OS calls
        let alloc_pages = num_pages.max(128);
        let alloc_size = alloc_pages * PAGE_SIZE;

        let ptr = unsafe { platform::page_alloc(alloc_size) };
        if ptr.is_null() {
            // Try exact size as fallback
            if alloc_pages > num_pages {
                return unsafe { self.grow_heap_exact(num_pages) };
            }
            return ptr::null_mut();
        }

        let start_page = (ptr as usize) >> PAGE_SHIFT;

        let s = span::alloc_span();
        if s.is_null() {
            unsafe { platform::page_dealloc(ptr, alloc_size) };
            return ptr::null_mut();
        }

        unsafe {
            (*s).start_page = start_page;
            (*s).num_pages = alloc_pages;
            (*s).state = SpanState::InUse; // Will be carved immediately
        }

        // Carve out what we need, remainder goes to free list
        unsafe { self.carve_span(s, num_pages) }
    }

    /// Fallback: allocate exactly num_pages from the OS.
    unsafe fn grow_heap_exact(&mut self, num_pages: usize) -> *mut Span {
        let alloc_size = num_pages * PAGE_SIZE;
        let ptr = unsafe { platform::page_alloc(alloc_size) };
        if ptr.is_null() {
            return ptr::null_mut();
        }

        let start_page = (ptr as usize) >> PAGE_SHIFT;

        let s = span::alloc_span();
        if s.is_null() {
            unsafe { platform::page_dealloc(ptr, alloc_size) };
            return ptr::null_mut();
        }

        unsafe {
            (*s).start_page = start_page;
            (*s).num_pages = num_pages;
            (*s).state = SpanState::InUse;
            self.pagemap.register_span(s);
        }
        s
    }

    /// Try to merge with the free span immediately before `span`.
    unsafe fn coalesce_left(&mut self, span: *mut Span) -> *mut Span {
        let start = unsafe { (*span).start_page };
        if start == 0 {
            return span;
        }

        let left = self.pagemap.get(start - 1);
        if left.is_null() {
            return span;
        }

        unsafe {
            if (*left).state != SpanState::Free {
                return span;
            }
            // Verify the left span actually ends right before us
            if (*left).start_page + (*left).num_pages != start {
                return span;
            }

            // Remove left from its free list
            let left_pages = (*left).num_pages;
            if left_pages <= MAX_PAGES {
                self.free_lists[left_pages].remove(left);
            } else {
                self.large_spans.remove(left);
            }

            // Merge: extend left span to include our pages
            (*left).num_pages += (*span).num_pages;

            // Free the now-redundant span struct
            span::dealloc_span(span);

            left
        }
    }

    /// Try to merge with the free span immediately after `span`.
    unsafe fn coalesce_right(&mut self, span: *mut Span) -> *mut Span {
        let end_page = unsafe { (*span).end_page() };

        let right = self.pagemap.get(end_page);
        if right.is_null() {
            return span;
        }

        unsafe {
            if (*right).state != SpanState::Free {
                return span;
            }
            // Verify the right span actually starts right after us
            if (*right).start_page != end_page {
                return span;
            }

            // Remove right from its free list
            let right_pages = (*right).num_pages;
            if right_pages <= MAX_PAGES {
                self.free_lists[right_pages].remove(right);
            } else {
                self.large_spans.remove(right);
            }

            // Merge: extend our span to include right's pages
            (*span).num_pages += (*right).num_pages;

            // Free the now-redundant span struct
            span::dealloc_span(right);

            span
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pagemap::PageMap;

    // Each test creates its own PageMap to avoid interference
    fn make_heap() -> (&'static PageMap, PageHeap) {
        // Leak a PageMap so we get a &'static reference
        let pm = Box::leak(Box::new(PageMap::new()));
        let heap = PageHeap::new(pm);
        (pm, heap)
    }

    #[test]
    fn test_allocate_single_page() {
        let (pm, mut heap) = make_heap();
        unsafe {
            let span = heap.allocate_span(1);
            assert!(!span.is_null());
            assert!((*span).num_pages >= 1);
            assert_eq!((*span).state, SpanState::InUse);

            // Should be registered in pagemap
            let found = pm.get((*span).start_page);
            assert_eq!(found, span);

            heap.deallocate_span(span);
        }
    }

    #[test]
    fn test_allocate_multiple_pages() {
        let (_pm, mut heap) = make_heap();
        unsafe {
            let span = heap.allocate_span(10);
            assert!(!span.is_null());
            assert!((*span).num_pages >= 10);

            heap.deallocate_span(span);
        }
    }

    #[test]
    fn test_reuse_freed_span() {
        let (_pm, mut heap) = make_heap();
        unsafe {
            let s1 = heap.allocate_span(1);
            assert!(!s1.is_null());
            let _page1 = (*s1).start_page;
            heap.deallocate_span(s1);

            // Allocate again - should reuse from free list
            let s2 = heap.allocate_span(1);
            assert!(!s2.is_null());

            heap.deallocate_span(s2);
        }
    }

    #[test]
    fn test_splitting() {
        let (_pm, mut heap) = make_heap();
        unsafe {
            // Allocate a large span first to populate free lists
            let big = heap.allocate_span(50);
            assert!(!big.is_null());
            heap.deallocate_span(big);

            // Now allocate a small span - should split from the free span
            let small = heap.allocate_span(5);
            assert!(!small.is_null());
            assert_eq!((*small).num_pages, 5);

            heap.deallocate_span(small);
        }
    }

    #[test]
    fn test_many_allocations() {
        let (_pm, mut heap) = make_heap();
        let mut spans = Vec::new();
        unsafe {
            for _ in 0..100 {
                let s = heap.allocate_span(1);
                assert!(!s.is_null());
                spans.push(s);
            }
            for s in spans {
                heap.deallocate_span(s);
            }
        }
    }
}

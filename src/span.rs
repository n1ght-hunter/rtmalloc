//! Span management: metadata for contiguous runs of pages, and a slab allocator
//! for Span structs themselves.

use crate::platform;
use crate::sync::SpinMutex;
use crate::PAGE_SIZE;
use core::ptr;

/// State of a span.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SpanState {
    /// Span is free in the page heap's free lists.
    Free = 0,
    /// Span is in use (holding allocated objects or a large allocation).
    InUse = 1,
}

/// An intrusive free list node stored inside freed memory.
/// The `next` pointer occupies the first 8 bytes of the freed object.
#[repr(C)]
pub struct FreeObject {
    pub next: *mut FreeObject,
}

/// Metadata for a contiguous run of pages.
///
/// Span structs are allocated from a dedicated slab allocator (not from the
/// allocator we're building) to avoid bootstrapping issues.
#[repr(C)]
pub struct Span {
    /// Starting page ID (address >> PAGE_SHIFT).
    pub start_page: usize,
    /// Number of pages in this span.
    pub num_pages: usize,
    /// Size class index (1..NUM_SIZE_CLASSES-1 for small, 0 for large).
    pub size_class: usize,
    /// Current state.
    pub state: SpanState,
    /// Number of objects currently allocated from this span.
    pub allocated_count: u32,
    /// Total number of objects that fit in this span (for the assigned size class).
    pub total_count: u32,
    /// Head of the intrusive free list of unallocated objects within this span.
    pub freelist: *mut FreeObject,
    /// Previous span in a doubly-linked list (page heap free lists, central cache span lists).
    pub prev: *mut Span,
    /// Next span in a doubly-linked list.
    pub next: *mut Span,
}

impl Span {
    /// The base address of the memory region this span covers.
    #[inline]
    pub fn start_addr(&self) -> *mut u8 {
        (self.start_page << crate::PAGE_SHIFT) as *mut u8
    }

    /// Total bytes covered by this span.
    #[inline]
    pub fn byte_size(&self) -> usize {
        self.num_pages * PAGE_SIZE
    }

    /// One past the last page ID in this span.
    #[inline]
    pub fn end_page(&self) -> usize {
        self.start_page + self.num_pages
    }
}

/// A doubly-linked list of spans.
pub struct SpanList {
    pub head: *mut Span,
    pub count: usize,
}

impl SpanList {
    pub const fn new() -> Self {
        Self {
            head: ptr::null_mut(),
            count: 0,
        }
    }

    /// Prepend a span to the front of the list.
    pub unsafe fn push(&mut self, span: *mut Span) {
        unsafe {
            (*span).next = self.head;
            (*span).prev = ptr::null_mut();
            if !self.head.is_null() {
                (*self.head).prev = span;
            }
            self.head = span;
            self.count += 1;
        }
    }

    /// Remove a specific span from the list.
    pub unsafe fn remove(&mut self, span: *mut Span) {
        unsafe {
            let prev = (*span).prev;
            let next = (*span).next;
            if !prev.is_null() {
                (*prev).next = next;
            } else {
                self.head = next;
            }
            if !next.is_null() {
                (*next).prev = prev;
            }
            (*span).prev = ptr::null_mut();
            (*span).next = ptr::null_mut();
            self.count -= 1;
        }
    }

    /// Pop the first span from the list.
    pub unsafe fn pop(&mut self) -> *mut Span {
        let span = self.head;
        if !span.is_null() {
            unsafe { self.remove(span) };
        }
        span
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }
}

// =============================================================================
// Span Slab Allocator
// =============================================================================

/// Allocates Span structs from OS pages, avoiding use of the main allocator.
/// Uses bump allocation within slabs, with a free list for recycled spans.
struct SpanSlabInner {
    /// Free list of recycled Span structs.
    free_list: *mut Span,
    /// Current bump pointer within the active slab.
    bump_ptr: *mut u8,
    /// End of the active slab.
    bump_end: *mut u8,
}

// SAFETY: SpanSlabInner is only accessed through a SpinMutex, which provides
// exclusive access. The raw pointers point to memory that outlives any thread.
unsafe impl Send for SpanSlabInner {}

impl SpanSlabInner {
    const fn new() -> Self {
        Self {
            free_list: ptr::null_mut(),
            bump_ptr: ptr::null_mut(),
            bump_end: ptr::null_mut(),
        }
    }

    unsafe fn alloc_span(&mut self) -> *mut Span {
        // Try the free list first
        if !self.free_list.is_null() {
            let span = self.free_list;
            unsafe { self.free_list = (*span).next };
            return span;
        }

        // Try bump allocation
        let span_size = core::mem::size_of::<Span>();
        let span_align = core::mem::align_of::<Span>();

        // Align bump_ptr
        let ptr = self.bump_ptr as usize;
        let aligned = (ptr + span_align - 1) & !(span_align - 1);
        let end = aligned + span_size;

        if end <= self.bump_end as usize {
            self.bump_ptr = end as *mut u8;
            return aligned as *mut Span;
        }

        // Need a new slab. Allocate one page (8 KiB) for span metadata.
        let slab = unsafe { platform::page_alloc(PAGE_SIZE) };
        if slab.is_null() {
            return ptr::null_mut();
        }

        self.bump_ptr = slab;
        self.bump_end = unsafe { slab.add(PAGE_SIZE) };

        // Recurse (will succeed via bump allocation now)
        unsafe { self.alloc_span() }
    }

    unsafe fn dealloc_span(&mut self, span: *mut Span) {
        // Add to free list for reuse. We store the next pointer in span.next.
        unsafe {
            (*span).next = self.free_list;
        }
        self.free_list = span;
    }
}

/// Global span slab allocator, protected by a spinlock.
static SPAN_SLAB: SpinMutex<SpanSlabInner> = SpinMutex::new(SpanSlabInner::new());

/// Allocate a new Span struct, zero-initialized.
pub fn alloc_span() -> *mut Span {
    let span = unsafe { SPAN_SLAB.lock().alloc_span() };
    if !span.is_null() {
        unsafe {
            ptr::write_bytes(span as *mut u8, 0, core::mem::size_of::<Span>());
        }
    }
    span
}

/// Return a Span struct to the slab allocator for reuse.
pub unsafe fn dealloc_span(span: *mut Span) {
    unsafe { SPAN_SLAB.lock().dealloc_span(span) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn test_alloc_dealloc_span() {
        let span = alloc_span();
        assert!(!span.is_null());
        unsafe {
            // Should be zero-initialized
            assert_eq!((*span).start_page, 0);
            assert_eq!((*span).num_pages, 0);
            assert_eq!((*span).size_class, 0);
            assert_eq!((*span).state, SpanState::Free);
            assert!((*span).freelist.is_null());
            assert!((*span).prev.is_null());
            assert!((*span).next.is_null());

            // Set some fields
            (*span).start_page = 42;
            (*span).num_pages = 10;

            dealloc_span(span);
        }

        // Reallocate - should reuse the freed span
        let span2 = alloc_span();
        assert!(!span2.is_null());
        // After zero-init, fields should be clean
        unsafe {
            assert_eq!((*span2).start_page, 0);
            dealloc_span(span2);
        }
    }

    #[test]
    fn test_alloc_many_spans() {
        let mut spans = Vec::new();
        // Allocate more spans than fit in one slab page
        let count = PAGE_SIZE / core::mem::size_of::<Span>() + 10;
        for _ in 0..count {
            let span = alloc_span();
            assert!(!span.is_null());
            spans.push(span);
        }
        // Free them all
        for span in spans {
            unsafe { dealloc_span(span) };
        }
    }

    #[test]
    fn test_span_list() {
        let mut list = SpanList::new();
        assert!(list.is_empty());
        assert_eq!(list.count, 0);

        let s1 = alloc_span();
        let s2 = alloc_span();
        let s3 = alloc_span();
        assert!(!s1.is_null());
        assert!(!s2.is_null());
        assert!(!s3.is_null());

        unsafe {
            (*s1).start_page = 1;
            (*s2).start_page = 2;
            (*s3).start_page = 3;

            list.push(s1);
            assert_eq!(list.count, 1);
            assert_eq!(list.head, s1);

            list.push(s2);
            assert_eq!(list.count, 2);
            assert_eq!(list.head, s2);

            list.push(s3);
            assert_eq!(list.count, 3);
            assert_eq!(list.head, s3);

            // Remove middle element
            list.remove(s2);
            assert_eq!(list.count, 2);
            assert_eq!((*s3).next, s1);

            // Pop front
            let popped = list.pop();
            assert_eq!(popped, s3);
            assert_eq!(list.count, 1);

            let popped = list.pop();
            assert_eq!(popped, s1);
            assert_eq!(list.count, 0);
            assert!(list.is_empty());

            dealloc_span(s1);
            dealloc_span(s2);
            dealloc_span(s3);
        }
    }
}

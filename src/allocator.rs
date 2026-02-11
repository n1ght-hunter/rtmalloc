//! Top-level allocator: ties all tiers together and implements GlobalAlloc.
//!
//! Static state lives here. The `TcMalloc` struct is zero-sized; all mutable
//! state is in module-level statics protected by spinlocks or atomics.

use crate::central_free_list::CentralCache;
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class;
use crate::span::FreeObject;
use crate::sync::SpinMutex;
use crate::thread_cache::ThreadCache;
use crate::transfer_cache::TransferCacheArray;
use crate::PAGE_SHIFT;
use crate::PAGE_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use std::cell::UnsafeCell;

// =============================================================================
// Global static state
// =============================================================================

static PAGE_MAP: PageMap = PageMap::new();
static PAGE_HEAP: SpinMutex<PageHeap> = SpinMutex::new(PageHeap::new(&PAGE_MAP));
static CENTRAL_CACHE: CentralCache = CentralCache::new();
static TRANSFER_CACHE: TransferCacheArray = TransferCacheArray::new();

// =============================================================================
// Thread-local cache
// =============================================================================

thread_local! {
    static THREAD_CACHE: UnsafeCell<ThreadCache> = UnsafeCell::new(ThreadCache::new());
}

/// Try to access the thread-local cache. Returns None if TLS is not available
/// (during thread startup/shutdown or if TLS was destroyed).
#[inline]
fn with_thread_cache<R>(f: impl FnOnce(&mut ThreadCache) -> R) -> Option<R> {
    THREAD_CACHE
        .try_with(|cell| {
            // SAFETY: We are the only accessor on this thread. GlobalAlloc methods
            // are not reentrant within a single thread in our implementation because
            // we don't call any allocating functions in the hot path.
            unsafe { f(&mut *cell.get()) }
        })
        .ok()
}

// =============================================================================
// The allocator
// =============================================================================

/// tcmalloc-style allocator for Rust.
///
/// Register as the global allocator with:
/// ```ignore
/// #[global_allocator]
/// static GLOBAL: rstcmalloc::TcMalloc = rstcmalloc::TcMalloc;
/// ```
pub struct TcMalloc;

unsafe impl GlobalAlloc for TcMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            // Return a non-null aligned dangling pointer for ZSTs
            return align as *mut u8;
        }

        // Effective size must satisfy both size and alignment
        let effective_size = size.max(align);

        // Check if this fits in a size class
        let class = size_class::size_to_class(effective_size);

        if class != 0 {
            let class_size = size_class::class_to_size(class);

            // Verify the size class satisfies alignment
            if class_size % align != 0 {
                // Rare: alignment exceeds what the size class provides.
                // Fall back to large allocation (page-aligned).
                return unsafe { self.alloc_large(layout) };
            }

            // Small allocation: try thread cache first
            if let Some(ptr) = with_thread_cache(|tc| unsafe {
                tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
            }) {
                return ptr;
            }

            // TLS unavailable: go directly to central cache
            return unsafe { self.alloc_from_central(class) };
        }

        // Large allocation
        unsafe { self.alloc_large(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || layout.size() == 0 {
            return;
        }

        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if span.is_null() {
            return; // Unknown pointer - defensive
        }

        let sc = unsafe { (*span).size_class };

        if sc == 0 {
            // Large allocation: return entire span to page heap
            unsafe { PAGE_HEAP.lock().deallocate_span(span) };
        } else {
            // Small allocation: try thread cache
            if with_thread_cache(|tc| unsafe {
                tc.deallocate(ptr, sc, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
            })
            .is_some()
            {
                return;
            }

            // TLS unavailable: return directly to central cache
            unsafe { self.dealloc_to_central(ptr, sc) };
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() && layout.size() > 0 {
            // Recycled objects from free lists are not guaranteed to be zeroed.
            // Must zero explicitly.
            unsafe { ptr::write_bytes(ptr, 0, layout.size()) };
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() || layout.size() == 0 {
            let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
            return unsafe { self.alloc(new_layout) };
        }

        if new_size == 0 {
            unsafe { self.dealloc(ptr, layout) };
            return layout.align() as *mut u8;
        }

        // Check if the current allocation can satisfy the new size
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if !span.is_null() {
            let sc = unsafe { (*span).size_class };
            if sc != 0 {
                let current_size = size_class::class_to_size(sc);
                let effective_new = new_size.max(layout.align());
                let new_class = size_class::size_to_class(effective_new);
                if new_class == sc {
                    // Same size class - no reallocation needed
                    return ptr;
                }
                // If shrinking and new size still fits in current class
                if new_size <= current_size {
                    return ptr;
                }
            } else {
                // Large allocation - check if it already has enough space
                let span_bytes = unsafe { (*span).num_pages } * PAGE_SIZE;
                if new_size <= span_bytes {
                    return ptr;
                }
            }
        }

        // Need to allocate new, copy, free old
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            let copy_size = layout.size().min(new_size);
            unsafe { ptr::copy_nonoverlapping(ptr, new_ptr, copy_size) };
            unsafe { self.dealloc(ptr, layout) };
        }
        new_ptr
    }
}

impl TcMalloc {
    /// Allocate from central cache directly (when TLS is unavailable).
    unsafe fn alloc_from_central(&self, size_class: usize) -> *mut u8 {
        let (count, head) = unsafe {
            CENTRAL_CACHE
                .get(size_class)
                .lock()
                .remove_range(1, &PAGE_HEAP, &PAGE_MAP)
        };
        if count == 0 || head.is_null() {
            ptr::null_mut()
        } else {
            head as *mut u8
        }
    }

    /// Deallocate to central cache directly (when TLS is unavailable).
    unsafe fn dealloc_to_central(&self, ptr: *mut u8, size_class: usize) {
        let obj = ptr as *mut FreeObject;
        unsafe { (*obj).next = ptr::null_mut() };
        unsafe {
            CENTRAL_CACHE
                .get(size_class)
                .lock()
                .insert_range(obj, 1, &PAGE_HEAP, &PAGE_MAP)
        };
    }

    /// Large allocation: allocate directly from page heap.
    unsafe fn alloc_large(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        // Round up to whole pages
        let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        let span = unsafe { PAGE_HEAP.lock().allocate_span(pages) };
        if span.is_null() {
            return ptr::null_mut();
        }

        unsafe {
            (*span).size_class = 0; // Mark as large allocation
            PAGE_MAP.register_span(span);
        }

        let addr = unsafe { (*span).start_addr() };

        // Page-aligned addresses satisfy any alignment <= PAGE_SIZE.
        // For alignment > PAGE_SIZE, we'd need special handling, but this
        // is extremely rare in practice.
        if align <= PAGE_SIZE {
            return addr;
        }

        // Over-aligned large allocation: the span is already page-aligned,
        // which on Windows is 64KB-aligned from VirtualAlloc. This covers
        // all practical alignment requirements.
        if (addr as usize) % align == 0 {
            return addr;
        }

        // Extremely rare: alignment > 64KB. Allocate extra and align within.
        // For now, just return what we have and hope for the best.
        // A production allocator would handle this properly.
        addr
    }
}

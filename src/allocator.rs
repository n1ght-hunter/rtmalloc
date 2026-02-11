//! Top-level allocator: ties all tiers together and implements GlobalAlloc.
//!
//! Static state lives here. The `TcMalloc` struct is zero-sized; all mutable
//! state is in module-level statics protected by spinlocks or atomics.
//!
//! With the `nightly` feature, uses `#[thread_local]` for direct TLS access
//! (single segment register read). Without it, allocations go directly through
//! the central free list (locked, slower).

use crate::central_free_list::CentralCache;
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class;
#[cfg(not(feature = "nightly"))]
use crate::span::FreeObject;
use crate::sync::SpinMutex;
#[cfg(feature = "nightly")]
use crate::thread_cache::ThreadCache;
#[cfg(feature = "nightly")]
use crate::transfer_cache::TransferCacheArray;
use crate::PAGE_SHIFT;
use crate::PAGE_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

// =============================================================================
// Global static state
// =============================================================================

static PAGE_MAP: PageMap = PageMap::new();
static PAGE_HEAP: SpinMutex<PageHeap> = SpinMutex::new(PageHeap::new(&PAGE_MAP));
static CENTRAL_CACHE: CentralCache = CentralCache::new();
#[cfg(feature = "nightly")]
static TRANSFER_CACHE: TransferCacheArray = TransferCacheArray::new();

// =============================================================================
// Thread-local cache (nightly only: #[thread_local] for direct TLS access)
// =============================================================================

#[cfg(feature = "nightly")]
#[thread_local]
static mut TC: ThreadCache = ThreadCache::new_const();

/// Get a mutable reference to the thread-local cache, initializing on first use.
#[cfg(feature = "nightly")]
#[inline(always)]
unsafe fn get_tc() -> &'static mut ThreadCache {
    let tc = unsafe { &mut *ptr::addr_of_mut!(TC) };
    if !tc.is_initialized() {
        tc_init_cold(tc);
    }
    tc
}

/// Cold path: initialize thread cache.
#[cfg(feature = "nightly")]
#[cold]
#[inline(never)]
fn tc_init_cold(tc: &mut ThreadCache) {
    tc.init();
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
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        if size == 0 {
            return layout.align() as *mut u8;
        }

        let align = layout.align();

        if align <= 8 {
            // Fast path: all size classes are 8-aligned, no alignment check needed
            let class = size_class::size_to_class(size);
            if class != 0 {
                return unsafe { self.alloc_small(class) };
            }
        } else {
            // Rare path: alignment > 8
            let effective_size = size.max(align);
            let class = size_class::size_to_class(effective_size);
            if class != 0 {
                let class_size = size_class::class_to_size(class);
                if class_size % align != 0 {
                    return unsafe { self.alloc_large(layout) };
                }
                return unsafe { self.alloc_small(class) };
            }
        }

        // Large allocation
        unsafe { self.alloc_large(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        if size == 0 {
            return;
        }

        let align = layout.align();

        if align <= 8 {
            // Fast path: compute size class from Layout, no pagemap lookup needed
            let class = size_class::size_to_class(size);
            if class != 0 {
                unsafe { self.dealloc_small(ptr, class) };
                return;
            }
        }

        // Slow path: large allocs or align > 8 need pagemap lookup
        unsafe { self.dealloc_slow(ptr) };
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() && layout.size() > 0 {
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

        let align = layout.align();

        // Fast path for align <= 8: compute size class from Layout, no pagemap
        if align <= 8 {
            let old_class = size_class::size_to_class(layout.size());
            if old_class != 0 {
                let current_size = size_class::class_to_size(old_class);
                if new_size <= current_size {
                    return ptr;
                }
                let new_class = size_class::size_to_class(new_size);
                if new_class == old_class {
                    return ptr;
                }
                // Need new allocation
                let new_layout =
                    unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
                let new_ptr = unsafe { self.alloc(new_layout) };
                if !new_ptr.is_null() {
                    let copy_size = layout.size().min(new_size);
                    unsafe { ptr::copy_nonoverlapping(ptr, new_ptr, copy_size) };
                    unsafe { self.dealloc(ptr, layout) };
                }
                return new_ptr;
            }
        }

        // Slow path: pagemap lookup for large allocs or align > 8
        unsafe { self.realloc_slow(ptr, layout, new_size) }
    }
}

impl TcMalloc {
    /// Small allocation: thread cache (nightly) or central cache (no_std fallback).
    #[cfg(feature = "nightly")]
    #[inline(always)]
    unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
        let tc = unsafe { get_tc() };
        unsafe { tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP) }
    }

    #[cfg(not(feature = "nightly"))]
    #[inline(always)]
    unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
        unsafe { self.alloc_from_central(class) }
    }

    /// Small deallocation: thread cache (nightly) or central cache (no_std fallback).
    #[cfg(feature = "nightly")]
    #[inline(always)]
    unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
        let tc = unsafe { get_tc() };
        unsafe {
            tc.deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
        };
    }

    #[cfg(not(feature = "nightly"))]
    #[inline(always)]
    unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
        unsafe { self.dealloc_to_central(ptr, class) };
    }

    /// Allocate from central cache directly (no thread cache).
    #[cfg(not(feature = "nightly"))]
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

    /// Deallocate to central cache directly (no thread cache).
    #[cfg(not(feature = "nightly"))]
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

    /// Slow dealloc path: pagemap lookup for large allocs or align > 8.
    #[cold]
    unsafe fn dealloc_slow(&self, ptr: *mut u8) {
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if span.is_null() {
            return;
        }

        let sc = unsafe { (*span).size_class };

        if sc == 0 {
            unsafe { PAGE_HEAP.lock().deallocate_span(span) };
        } else {
            unsafe { self.dealloc_small(ptr, sc) };
        }
    }

    /// Slow realloc path: pagemap lookup for large allocs or align > 8.
    #[cold]
    unsafe fn realloc_slow(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if !span.is_null() {
            let sc = unsafe { (*span).size_class };
            if sc != 0 {
                let current_size = size_class::class_to_size(sc);
                let effective_new = new_size.max(layout.align());
                let new_class = size_class::size_to_class(effective_new);
                if new_class == sc {
                    return ptr;
                }
                if new_size <= current_size {
                    return ptr;
                }
            } else {
                let span_bytes = unsafe { (*span).num_pages } * PAGE_SIZE;
                if new_size <= span_bytes {
                    return ptr;
                }
            }
        }

        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            let copy_size = layout.size().min(new_size);
            unsafe { ptr::copy_nonoverlapping(ptr, new_ptr, copy_size) };
            unsafe { self.dealloc(ptr, layout) };
        }
        new_ptr
    }

    /// Large allocation: allocate directly from page heap.
    unsafe fn alloc_large(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        let span = unsafe { PAGE_HEAP.lock().allocate_span(pages) };
        if span.is_null() {
            return ptr::null_mut();
        }

        unsafe {
            (*span).size_class = 0;
            PAGE_MAP.register_span(span);
        }

        let addr = unsafe { (*span).start_addr() };

        if align <= PAGE_SIZE {
            return addr;
        }

        if (addr as usize) % align == 0 {
            return addr;
        }

        addr
    }
}

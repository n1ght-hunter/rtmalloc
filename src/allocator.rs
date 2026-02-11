//! Top-level allocator: ties all tiers together and implements GlobalAlloc.
//!
//! Static state lives here. The `TcMalloc` struct is zero-sized; all mutable
//! state is in module-level statics protected by spinlocks or atomics.
//!
//! Uses nightly `#[thread_local]` for direct TLS access (single segment
//! register read) instead of `thread_local!` + `try_with` overhead.

use crate::central_free_list::CentralCache;
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class;
use crate::sync::SpinMutex;
use crate::thread_cache::ThreadCache;
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
static TRANSFER_CACHE: TransferCacheArray = TransferCacheArray::new();

// =============================================================================
// Thread-local cache (nightly #[thread_local] for direct TLS access)
// =============================================================================

#[thread_local]
static mut TC: ThreadCache = ThreadCache::new_const();

/// Guard that flushes the thread cache on thread exit.
/// Only touched once during init (cold path) to register the destructor.
struct TcFlush;

impl Drop for TcFlush {
    fn drop(&mut self) {
        unsafe {
            let tc = &mut *ptr::addr_of_mut!(TC);
            tc.flush_and_destroy(&TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP);
        }
    }
}

thread_local! {
    static TC_FLUSH: TcFlush = const { TcFlush };
}

/// Get a mutable reference to the thread-local cache, initializing on first use.
#[inline(always)]
unsafe fn get_tc() -> &'static mut ThreadCache {
    let tc = unsafe { &mut *ptr::addr_of_mut!(TC) };
    if !tc.is_initialized() {
        tc_init_cold(tc);
    }
    tc
}

/// Cold path: initialize thread cache and register flush destructor.
#[cold]
#[inline(never)]
fn tc_init_cold(tc: &mut ThreadCache) {
    tc.init();
    // Register the flush destructor for thread exit.
    // try_with avoids panic if TLS is being destroyed (shouldn't happen on init).
    let _ = TC_FLUSH.try_with(|_| {});
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
                let tc = unsafe { get_tc() };
                return unsafe {
                    tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                };
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
                let tc = unsafe { get_tc() };
                return unsafe {
                    tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                };
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
                let tc = unsafe { get_tc() };
                unsafe {
                    tc.deallocate(
                        ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP,
                    )
                };
                return;
            }
        }

        // Slow path: large allocs or align > 8 need pagemap lookup
        unsafe { self.dealloc_slow(ptr, layout) };
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
                    // Still fits in current size class
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
    /// Slow dealloc path: pagemap lookup for large allocs or align > 8.
    #[cold]
    unsafe fn dealloc_slow(&self, ptr: *mut u8, _layout: Layout) {
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if span.is_null() {
            return;
        }

        let sc = unsafe { (*span).size_class };

        if sc == 0 {
            // Large allocation: return entire span to page heap
            unsafe { PAGE_HEAP.lock().deallocate_span(span) };
        } else {
            // Small allocation with align > 8 that went through size class path
            let tc = unsafe { get_tc() };
            unsafe {
                tc.deallocate(
                    ptr, sc, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP,
                )
            };
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
            (*span).size_class = 0; // Mark as large allocation
            PAGE_MAP.register_span(span);
        }

        let addr = unsafe { (*span).start_addr() };

        if align <= PAGE_SIZE {
            return addr;
        }

        // Over-aligned: VirtualAlloc returns 64KB-aligned on Windows
        if (addr as usize) % align == 0 {
            return addr;
        }

        addr
    }
}

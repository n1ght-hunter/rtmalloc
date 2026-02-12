//! Top-level allocator: ties all tiers together and implements GlobalAlloc.
//!
//! Static state lives here. The `TcMalloc` struct is zero-sized; all mutable
//! state is in module-level statics protected by spinlocks or atomics.
//!
//! Cache strategy (fastest to slowest):
//! - `percpu` feature: per-CPU slab via rseq (Linux x86_64, fastest)
//! - `nightly` feature: `#[thread_local]` for direct TLS (single register read)
//! - `std` feature: `std::thread_local!` macro (slower TLS, but still cached)
//! - neither: central free list only (locked, slowest)

use crate::PAGE_SHIFT;
use crate::PAGE_SIZE;
use crate::central_free_list::CentralCache;
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class;
use crate::sync::SpinMutex;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

cfg_if::cfg_if! {
    if #[cfg(feature = "percpu")] {
        use crate::cpu_cache;
        use crate::transfer_cache::TransferCacheArray;
    } else if #[cfg(any(feature = "nightly", feature = "std"))] {
        use crate::thread_cache::ThreadCache;
        use crate::transfer_cache::TransferCacheArray;
    }
}

cfg_if::cfg_if! {
    if #[cfg(not(any(feature = "nightly", feature = "percpu")))] {
        use crate::span::FreeObject;
    }
}

// =============================================================================
// Global static state
// =============================================================================

static PAGE_MAP: PageMap = PageMap::new();
static PAGE_HEAP: SpinMutex<PageHeap> = SpinMutex::new(PageHeap::new(&PAGE_MAP));
static CENTRAL_CACHE: CentralCache = CentralCache::new();

cfg_if::cfg_if! {
    if #[cfg(any(feature = "percpu", feature = "nightly", feature = "std"))] {
        static TRANSFER_CACHE: TransferCacheArray = TransferCacheArray::new();
    }
}

// =============================================================================
// Thread-local cache
// =============================================================================

cfg_if::cfg_if! {
    if #[cfg(feature = "percpu")] {
        // Per-CPU cache via rseq — no thread-local cache needed.
        // All state lives in cpu_cache module.
    } else if #[cfg(feature = "nightly")] {
        // Direct TLS via #[thread_local] — single register read, fastest.
        #[thread_local]
        static mut TC: ThreadCache = ThreadCache::new_const();

        #[inline(always)]
        unsafe fn get_tc() -> &'static mut ThreadCache {
            let tc = unsafe { &mut *ptr::addr_of_mut!(TC) };
            if !tc.is_initialized() {
                tc_init_cold(tc);
            }
            tc
        }

        #[cold]
        #[inline(never)]
        fn tc_init_cold(tc: &mut ThreadCache) {
            tc.init();
        }
    } else if #[cfg(feature = "std")] {
        // std::thread_local! — slower TLS access, but still thread-cached.
        std::thread_local! {
            static THREAD_CACHE: core::cell::UnsafeCell<ThreadCache> =
                core::cell::UnsafeCell::new(ThreadCache::new());
        }

        #[inline]
        fn with_thread_cache<R>(f: impl FnOnce(&mut ThreadCache) -> R) -> Option<R> {
            THREAD_CACHE
                .try_with(|cell| unsafe { f(&mut *cell.get()) })
                .ok()
        }
    }
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
            let class = size_class::size_to_class(size);
            if class != 0 {
                return unsafe { self.alloc_small(class) };
            }
        } else {
            let effective_size = size.max(align);
            let class = size_class::size_to_class(effective_size);
            if class != 0 {
                let class_size = size_class::class_to_size(class);
                if !class_size.is_multiple_of(align) {
                    return unsafe { self.alloc_large(layout) };
                }
                return unsafe { self.alloc_small(class) };
            }
        }

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
            let class = size_class::size_to_class(size);
            if class != 0 {
                unsafe { self.dealloc_small(ptr, class) };
                return;
            }
        }

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

        unsafe { self.realloc_slow(ptr, layout, new_size) }
    }
}

impl TcMalloc {
    // =========================================================================
    // alloc_small / dealloc_small — three tiers via cfg_if
    // =========================================================================

    cfg_if::cfg_if! {
        if #[cfg(feature = "percpu")] {
            #[inline(always)]
            unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
                unsafe {
                    cpu_cache::alloc(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                unsafe {
                    cpu_cache::dealloc(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                };
            }
        } else if #[cfg(feature = "nightly")] {
            #[inline(always)]
            unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
                let tc = unsafe { get_tc() };
                unsafe { tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP) }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                let tc = unsafe { get_tc() };
                unsafe {
                    tc.deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                };
            }
        } else if #[cfg(feature = "std")] {
            #[inline(always)]
            unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
                if let Some(ptr) = with_thread_cache(|tc| unsafe {
                    tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                }) {
                    ptr
                } else {
                    unsafe { self.alloc_from_central(class) }
                }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                if with_thread_cache(|tc| unsafe {
                    tc.deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                })
                .is_none()
                {
                    unsafe { self.dealloc_to_central(ptr, class) };
                }
            }
        } else {
            #[inline(always)]
            unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
                unsafe { self.alloc_from_central(class) }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                unsafe { self.dealloc_to_central(ptr, class) };
            }
        }
    }

    // =========================================================================
    // Central cache fallback (std and no-std-no-nightly paths)
    // =========================================================================

    cfg_if::cfg_if! {
        if #[cfg(not(any(feature = "nightly", feature = "percpu")))] {
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
        }
    }

    // =========================================================================
    // Slow paths (shared by all tiers)
    // =========================================================================

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

    #[cold]
    unsafe fn realloc_slow(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
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

    unsafe fn alloc_large(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let pages = size.div_ceil(PAGE_SIZE);
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

        if (addr as usize).is_multiple_of(align) {
            return addr;
        }

        addr
    }
}

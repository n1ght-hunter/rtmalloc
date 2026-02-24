//! Top-level allocator: ties all tiers together and implements GlobalAlloc.
//!
//! Static state lives here. The `RtMalloc` struct is zero-sized; all mutable
//! state is in module-level statics protected by spinlocks or atomics.
//!
//! Cache strategy (fastest to slowest):
//! - `percpu` feature: per-CPU slab via rseq (Linux x86_64, fastest)
//! - `nightly` feature: `#[thread_local]` with const-init (single TLS read, no branches)
//! - `std` feature: `std::thread_local!` with const-init (no lazy init overhead)
//! - neither: central free list only (locked, slowest)

use crate::config::{PAGE_SIZE, PAGE_SHIFT};
use crate::{hist_record, stat_add, stat_inc};
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

use crate::span;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "percpu"))] {
        use crate::span::FreeObject;
    }
}

pub(crate) static PAGE_MAP: PageMap = PageMap::new();
pub(crate) static PAGE_HEAP: SpinMutex<PageHeap> = SpinMutex::new(PageHeap::new(&PAGE_MAP));
pub(crate) static CENTRAL_CACHE: CentralCache = CentralCache::new();

cfg_if::cfg_if! {
    if #[cfg(any(feature = "percpu", feature = "nightly", feature = "std"))] {
        pub(crate) static TRANSFER_CACHE: TransferCacheArray = TransferCacheArray::new();
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "percpu")] {
        // Per-CPU cache via rseq — no thread-local cache needed.
    } else if #[cfg(feature = "nightly")] {
        #[derive(Clone, Copy, PartialEq)]
        #[repr(u8)]
        enum TlsState {
            Uninitialized = 0,
            Active = 1,
            Destroyed = 2,
        }

        struct TlsSlot<T> {
            state: TlsState,
            content: T,
        }

        /// Get a raw mutable pointer to the thread-local ThreadCache.
        #[inline(always)]
        unsafe fn tc() -> *mut ThreadCache {
            unsafe { core::ptr::addr_of_mut!(TC.content) }
        }

        #[thread_local]
        static mut TC: TlsSlot<ThreadCache> = TlsSlot {
            state: TlsState::Uninitialized,
            content: ThreadCache::new_const(),
        };

        /// Flush the ThreadCache and mark TC as Destroyed (reentrancy-safe).
        #[cold]
        #[allow(dead_code)] // Only called from cleanup modules (std feature)
        unsafe fn tc_destroy() {
            unsafe {
                if TC.state == TlsState::Active {
                    TC.state = TlsState::Destroyed;
                    (*tc()).flush_and_destroy(
                        &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP,
                    );
                }
            }
        }

        /// Initialize the thread-local ThreadCache.
        #[cold]
        #[inline(never)]
        unsafe fn tc_init() {
            unsafe { (*tc()).init() };
            // Set BEFORE cleanup registration — if register() triggers allocation,
            // the reentrant call sees TC as Active and uses it normally.
            unsafe { TC.state = TlsState::Active };
            tc_cleanup::register();
        }

        // -- Cleanup: nightly + std --
        #[cfg(feature = "std")]
        mod tc_cleanup {
            struct Guard;

            impl Drop for Guard {
                fn drop(&mut self) {
                    if unsafe { super::TC.state } == super::TlsState::Active {
                        unsafe { super::tc_destroy() };
                    }
                }
            }

            std::thread_local! {
                static GUARD: Guard = const { Guard };
            }

            pub(super) fn register() {
                // Use try_with: if std's TLS is already destroyed (rare edge case
                // during thread shutdown), silently skip — the ThreadCache leaks.
                let _ = GUARD.try_with(|_| {});
            }
        }

        // -- Cleanup: nightly, no std --
        #[cfg(not(feature = "std"))]
        mod tc_cleanup {
            pub(super) fn register() {}
        }
    } else if #[cfg(feature = "std")] {
        std::thread_local! {
            static TC_CELL: core::cell::UnsafeCell<ThreadCache> = const {
                core::cell::UnsafeCell::new(ThreadCache::new_const())
            };
        }
    }
}

/// tcmalloc-style allocator for Rust.
///
/// Register as the global allocator with:
/// ```ignore
/// #[global_allocator]
/// static GLOBAL: rtmalloc::RtMalloc = rtmalloc::RtMalloc;
/// ```
pub struct RtMalloc;

unsafe impl GlobalAlloc for RtMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        if size == 0 {
            return layout.align() as *mut u8;
        }

        stat_inc!(alloc_count);
        stat_add!(alloc_bytes, size as u64);
        hist_record!(size);

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
                if align > PAGE_SIZE || !class_size.is_multiple_of(align) {
                    return unsafe { self.alloc_large(layout) };
                }
                return unsafe { self.alloc_small(class) };
            }
        }

        unsafe { self.alloc_large(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }

        stat_inc!(dealloc_count);

        // Look up the actual size class from the span metadata, like tcmalloc.
        // We cannot trust layout.size() because realloc may return the same
        // pointer for a shrink (staying in-place when new_size fits in the
        // existing size class), so the caller's layout may not match the
        // span's real size class.
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if span.is_null() {
            return;
        }

        let sc = unsafe { (*span).size_class };
        if sc != 0 {
            unsafe { self.dealloc_small(ptr, sc) };
        } else {
            unsafe { PAGE_HEAP.lock().deallocate_span(span) };
        }
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

        stat_inc!(realloc_count);

        // Look up the REAL allocation size from span metadata, like tcmalloc.
        // We cannot trust layout.size() because prior reallocs may have returned
        // the same pointer for an in-place shrink, so the caller's layout may
        // carry a smaller size than the span's actual size class.
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        let old_usable = if !span.is_null() {
            let sc = unsafe { (*span).size_class };
            if sc != 0 {
                size_class::class_to_size(sc)
            } else {
                (unsafe { (*span).num_pages }) * PAGE_SIZE
            }
        } else {
            layout.size() // Defensive fallback
        };

        // Fits in current allocation — return same pointer
        if new_size <= old_usable {
            return ptr;
        }

        // Must grow — allocate, copy, free
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            unsafe { ptr::copy_nonoverlapping(ptr, new_ptr, old_usable.min(new_size)) };
            unsafe { self.dealloc(ptr, layout) };
        }
        new_ptr
    }
}

impl RtMalloc {
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
                if unsafe { TC.state } == TlsState::Active {
                    return unsafe {
                        (*tc())
                            .allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                    };
                }
                unsafe { self.alloc_small_slow(class) }
            }

            #[cold]
            #[inline(never)]
            unsafe fn alloc_small_slow(&self, class: usize) -> *mut u8 {
                if unsafe { TC.state } == TlsState::Uninitialized {
                    unsafe { tc_init() };
                    return unsafe {
                        (*tc())
                            .allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                    };
                }
                unsafe { self.alloc_from_central(class) }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                if unsafe { TC.state } == TlsState::Active {
                    unsafe {
                        (*tc())
                            .deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP);
                    }
                    return;
                }
                unsafe { self.dealloc_to_central(ptr, class) };
            }
        } else if #[cfg(feature = "std")] {
            #[inline(always)]
            unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
                match TC_CELL.try_with(|cell| unsafe {
                    let tc = &mut *cell.get();
                    tc.allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                }) {
                    Ok(ptr) => ptr,
                    Err(_) => unsafe { self.alloc_from_central(class) },
                }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                if TC_CELL.try_with(|cell| unsafe {
                    let tc = &mut *cell.get();
                    tc.deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP);
                })
                .is_err()
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

    cfg_if::cfg_if! {
        if #[cfg(not(feature = "percpu"))] {
            unsafe fn alloc_from_central(&self, size_class: usize) -> *mut u8 {
                stat_inc!(thread_cache_misses);
                stat_inc!(central_cache_hits);
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

    unsafe fn alloc_large(&self, layout: Layout) -> *mut u8 {
        stat_inc!(page_heap_allocs);

        let size = layout.size();
        let align = layout.align();
        let size_pages = size.div_ceil(PAGE_SIZE);

        if align <= PAGE_SIZE {
            // Page alignment is sufficient — simple allocation
            let span = unsafe { PAGE_HEAP.lock().allocate_span(size_pages) };
            if span.is_null() {
                return ptr::null_mut();
            }
            unsafe {
                (*span).size_class = 0;
                PAGE_MAP.register_span(span);
            }
            return unsafe { (*span).start_addr() };
        }

        // Over-aligned: align > PAGE_SIZE.
        // Over-allocate to guarantee an aligned region exists within.
        // Like tcmalloc's do_memalign: allocate extra, trim prefix/suffix.
        let align_pages = align / PAGE_SIZE;
        let total_pages = size_pages + align_pages - 1;

        let mut heap = PAGE_HEAP.lock();
        let span = unsafe { heap.allocate_span(total_pages) };
        if span.is_null() {
            return ptr::null_mut();
        }

        let start_addr = unsafe { (*span).start_addr() } as usize;
        let aligned_addr = (start_addr + align - 1) & !(align - 1);
        let prefix_pages = (aligned_addr - start_addr) / PAGE_SIZE;
        let suffix_pages = total_pages - prefix_pages - size_pages;

        unsafe {
            // Clear pagemap entries for the original span
            PAGE_MAP.unregister_span(span);

            // Return prefix pages to page heap
            if prefix_pages > 0 {
                let prefix = span::alloc_span();
                if !prefix.is_null() {
                    (*prefix).start_page = (*span).start_page;
                    (*prefix).num_pages = prefix_pages;
                    heap.deallocate_span(prefix);
                }
            }

            // Resize main span to the aligned region
            (*span).start_page += prefix_pages;
            (*span).num_pages = size_pages;
            (*span).size_class = 0;
            PAGE_MAP.register_span(span);

            // Return suffix pages to page heap
            if suffix_pages > 0 {
                let suffix = span::alloc_span();
                if !suffix.is_null() {
                    (*suffix).start_page = (*span).start_page + size_pages;
                    (*suffix).num_pages = suffix_pages;
                    heap.deallocate_span(suffix);
                }
            }
        }

        aligned_addr as *mut u8
    }
}

#[cfg(feature = "nightly")]
unsafe impl core::alloc::Allocator for RtMalloc {
    fn allocate(
        &self,
        layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        let ptr = unsafe { GlobalAlloc::alloc(self, layout) };
        if ptr.is_null() {
            Err(core::alloc::AllocError)
        } else {
            let slice = core::ptr::slice_from_raw_parts_mut(ptr, layout.size());
            Ok(unsafe { core::ptr::NonNull::new_unchecked(slice) })
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        unsafe { GlobalAlloc::dealloc(self, ptr.as_ptr(), layout) }
    }
}

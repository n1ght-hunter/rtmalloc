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

use crate::central_free_list::CentralCache;
use crate::config::{PAGE_SHIFT, PAGE_SIZE};
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class;
use crate::sync::SpinMutex;
use crate::{hist_record, stat_add, stat_inc};
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

cfg_if::cfg_if! {
    if #[cfg(feature = "percpu")] {
        use crate::cpu_cache;
        use crate::transfer_cache::TransferCacheArray;
    } else if #[cfg(all(not(feature = "percpu"), any(feature = "nightly", feature = "std")))] {
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

// --- Shared types and functions for nightly + std paths ---

#[cfg(all(not(feature = "percpu"), any(feature = "nightly", feature = "std")))]
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum TlsState {
    Uninitialized = 0,
    Active = 1,
    Destroyed = 2,
}

/// Thread-local slot holding the state machine and cache. ThreadCache has no
/// Drop impl, so std::thread_local! won't call __cxa_thread_atexit_impl —
/// no LD_PRELOAD recursion. Cleanup is explicit via `destroy()` from Guard::drop.
#[cfg(all(not(feature = "percpu"), any(feature = "nightly", feature = "std")))]
struct TcSlot {
    state: TlsState,
    cache: ThreadCache,
}

#[cfg(all(not(feature = "percpu"), any(feature = "nightly", feature = "std")))]
impl TcSlot {
    #[inline(always)]
    fn tc(&mut self) -> &mut ThreadCache {
        &mut self.cache
    }

    #[cold]
    #[inline(never)]
    unsafe fn init(&mut self) {
        self.cache.init();
        // Active BEFORE register — reentrant mallocs from Guard registration
        // see Active state and use the thread cache normally.
        self.state = TlsState::Active;
        tc_cleanup::register();
    }

    #[cold]
    #[allow(dead_code)] // Only called from tc_cleanup::Guard::drop (requires std)
    unsafe fn destroy(&mut self) {
        if self.state == TlsState::Active {
            self.state = TlsState::Destroyed;
            unsafe {
                self.cache.flush_and_destroy(
                    &TRANSFER_CACHE,
                    &CENTRAL_CACHE,
                    &PAGE_HEAP,
                    &PAGE_MAP,
                );
            }
        }
    }
}

// --- Thread-local storage declarations ---

cfg_if::cfg_if! {
    if #[cfg(feature = "percpu")] {
        // Per-CPU cache via rseq — no thread-local cache needed.
    } else if #[cfg(feature = "nightly")] {
        #[thread_local]
        static mut TC: TcSlot = TcSlot {
            state: TlsState::Uninitialized,
            cache: ThreadCache::new_const(),
        };

        #[inline(always)]
        unsafe fn tc_slot() -> &'static mut TcSlot {
            unsafe { &mut *core::ptr::addr_of_mut!(TC) }
        }
    } else if #[cfg(feature = "std")] {
        std::thread_local! {
            static TC_CELL: core::cell::UnsafeCell<TcSlot> = const {
                core::cell::UnsafeCell::new(TcSlot {
                    state: TlsState::Uninitialized,
                    cache: ThreadCache::new_const(),
                })
            };
        }
    }
}

// --- Thread cache cleanup ---

#[cfg(all(not(feature = "percpu"), any(feature = "nightly", feature = "std")))]
mod tc_cleanup {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            struct Guard;

            impl Drop for Guard {
                fn drop(&mut self) {
                    cfg_if::cfg_if! {
                        if #[cfg(feature = "nightly")] {
                            unsafe { super::tc_slot().destroy() };
                        } else {
                            let _ = super::TC_CELL.try_with(|cell| {
                                unsafe { (*cell.get()).destroy() };
                            });
                        }
                    }
                }
            }

            std::thread_local! {
                static GUARD: Guard = const { Guard };
            }

            pub(super) fn register() {
                let _ = GUARD.try_with(|_| {});
            }
        } else {
            // Nightly only, no std: #[thread_local] statics are never dropped,
            // and without std we cannot use std::thread_local! for a Guard.
            pub(super) fn register() {}
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
                let slot = unsafe { tc_slot() };
                match slot.state {
                    TlsState::Active => unsafe {
                        slot.tc().allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                    },
                    TlsState::Uninitialized => unsafe {
                        slot.init();
                        slot.tc().allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                    },
                    TlsState::Destroyed => unsafe { self.alloc_from_central(class) },
                }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                let slot = unsafe { tc_slot() };
                match slot.state {
                    TlsState::Active => unsafe {
                        slot.tc().deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP);
                    },
                    _ => unsafe { self.dealloc_to_central(ptr, class) },
                }
            }
        } else if #[cfg(feature = "std")] {
            #[inline(always)]
            unsafe fn alloc_small(&self, class: usize) -> *mut u8 {
                match TC_CELL.try_with(|cell| unsafe {
                    let slot = &mut *cell.get();
                    match slot.state {
                        TlsState::Active => {
                            slot.tc().allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                        }
                        TlsState::Uninitialized => {
                            slot.init();
                            slot.tc().allocate(class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP)
                        }
                        TlsState::Destroyed => ptr::null_mut(),
                    }
                }) {
                    Ok(ptr) if !ptr.is_null() => ptr,
                    _ => unsafe { self.alloc_from_central(class) },
                }
            }

            #[inline(always)]
            unsafe fn dealloc_small(&self, ptr: *mut u8, class: usize) {
                let used_tc = TC_CELL.try_with(|cell| unsafe {
                    let slot = &mut *cell.get();
                    match slot.state {
                        TlsState::Active => {
                            slot.tc().deallocate(ptr, class, &TRANSFER_CACHE, &CENTRAL_CACHE, &PAGE_HEAP, &PAGE_MAP);
                            true
                        }
                        _ => false,
                    }
                });
                if !matches!(used_tc, Ok(true)) {
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

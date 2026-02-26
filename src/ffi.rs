//! C-ABI exports for external linking (e.g., from bench via build.rs).
//!
//! Gated behind `features = ["ffi"]`. Built as part of the staticlib.
//! With `testing` feature, export names are prefixed by variant:
//!   - `percpu`  → `rtmalloc_percpu_*`
//!   - `nightly` → `rtmalloc_nightly_*`
//!   - `std`     → `rtmalloc_std_*`
//!   - neither   → `rtmalloc_nostd_*`
//!
//! Without `testing`, exports plain `rtmalloc_*` names.

use crate::allocator::RtMalloc;
use core::alloc::{GlobalAlloc, Layout};

static ALLOC: RtMalloc = RtMalloc;

// Note: percpu implies nightly, so the percpu check must come first.

#[cfg_attr(not(feature = "testing"), unsafe(no_mangle))]
#[cfg_attr(
    all(feature = "testing", feature = "percpu"),
    unsafe(export_name = "rtmalloc_percpu_alloc")
)]
#[cfg_attr(
    all(feature = "testing", feature = "nightly", not(feature = "percpu")),
    unsafe(export_name = "rtmalloc_nightly_alloc")
)]
#[cfg_attr(
    all(
        feature = "testing",
        feature = "std",
        not(any(feature = "nightly", feature = "percpu"))
    ),
    unsafe(export_name = "rtmalloc_std_alloc")
)]
#[cfg_attr(
    all(
        feature = "testing",
        not(any(feature = "nightly", feature = "std", feature = "percpu"))
    ),
    unsafe(export_name = "rtmalloc_nostd_alloc")
)]
/// # Safety
///
/// `align` must be a power of two. `size` must be a multiple of `align` or zero.
pub unsafe extern "C" fn rtmalloc_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.alloc(layout) }
}

#[cfg_attr(not(feature = "testing"), unsafe(no_mangle))]
#[cfg_attr(
    all(feature = "testing", feature = "percpu"),
    unsafe(export_name = "rtmalloc_percpu_dealloc")
)]
#[cfg_attr(
    all(feature = "testing", feature = "nightly", not(feature = "percpu")),
    unsafe(export_name = "rtmalloc_nightly_dealloc")
)]
#[cfg_attr(
    all(
        feature = "testing",
        feature = "std",
        not(any(feature = "nightly", feature = "percpu"))
    ),
    unsafe(export_name = "rtmalloc_std_dealloc")
)]
#[cfg_attr(
    all(
        feature = "testing",
        not(any(feature = "nightly", feature = "std", feature = "percpu"))
    ),
    unsafe(export_name = "rtmalloc_nostd_dealloc")
)]
/// # Safety
///
/// `ptr` must have been returned by `rtmalloc_alloc` with the same `size`/`align`.
pub unsafe extern "C" fn rtmalloc_dealloc(ptr: *mut u8, size: usize, align: usize) {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.dealloc(ptr, layout) }
}

#[cfg_attr(not(feature = "testing"), unsafe(no_mangle))]
#[cfg_attr(
    all(feature = "testing", feature = "percpu"),
    unsafe(export_name = "rtmalloc_percpu_realloc")
)]
#[cfg_attr(
    all(feature = "testing", feature = "nightly", not(feature = "percpu")),
    unsafe(export_name = "rtmalloc_nightly_realloc")
)]
#[cfg_attr(
    all(
        feature = "testing",
        feature = "std",
        not(any(feature = "nightly", feature = "percpu"))
    ),
    unsafe(export_name = "rtmalloc_std_realloc")
)]
#[cfg_attr(
    all(
        feature = "testing",
        not(any(feature = "nightly", feature = "std", feature = "percpu"))
    ),
    unsafe(export_name = "rtmalloc_nostd_realloc")
)]
/// # Safety
///
/// `ptr` must have been returned by `rtmalloc_alloc` with the same `size`/`align`.
pub unsafe extern "C" fn rtmalloc_realloc(
    ptr: *mut u8,
    size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.realloc(ptr, layout, new_size) }
}

#[cfg(feature = "c-abi")]
#[allow(clippy::missing_safety_doc)]
pub mod c_abi {
    use super::ALLOC;
    use crate::allocator::PAGE_MAP;
    use crate::config::{PAGE_SHIFT, PAGE_SIZE};
    use crate::size_class;
    use core::alloc::{GlobalAlloc, Layout};

    const MIN_ALIGN: usize = if core::mem::size_of::<usize>() >= 8 {
        16
    } else {
        8
    };

    unsafe fn usable_size(ptr: *mut u8) -> usize {
        if ptr.is_null() {
            return 0;
        }
        let page_id = (ptr as usize) >> PAGE_SHIFT;
        let span = PAGE_MAP.get(page_id);
        if span.is_null() {
            return 0;
        }
        let sc = unsafe { (*span).size_class };
        if sc != 0 {
            size_class::class_to_size(sc)
        } else {
            (unsafe { (*span).num_pages }) * PAGE_SIZE
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
        if size == 0 {
            return MIN_ALIGN as *mut u8;
        }
        let layout = unsafe { Layout::from_size_align_unchecked(size, MIN_ALIGN) };
        unsafe { ALLOC.alloc(layout) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn free(ptr: *mut u8) {
        if ptr.is_null() || (ptr as usize) <= MIN_ALIGN {
            return;
        }
        let layout = unsafe { Layout::from_size_align_unchecked(MIN_ALIGN, MIN_ALIGN) };
        unsafe { ALLOC.dealloc(ptr, layout) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
        if ptr.is_null() || (ptr as usize) <= MIN_ALIGN {
            return unsafe { malloc(new_size) };
        }
        if new_size == 0 {
            unsafe { free(ptr) };
            return core::ptr::null_mut();
        }
        let layout = unsafe { Layout::from_size_align_unchecked(MIN_ALIGN, MIN_ALIGN) };
        unsafe { ALLOC.realloc(ptr, layout, new_size) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
        let total = match count.checked_mul(size) {
            Some(t) => t,
            None => return core::ptr::null_mut(),
        };
        if total == 0 {
            return MIN_ALIGN as *mut u8;
        }
        let layout = unsafe { Layout::from_size_align_unchecked(total, MIN_ALIGN) };
        unsafe { ALLOC.alloc_zeroed(layout) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn posix_memalign(
        memptr: *mut *mut u8,
        align: usize,
        size: usize,
    ) -> core::ffi::c_int {
        if !align.is_power_of_two() || align < core::mem::size_of::<usize>() {
            return 22; // EINVAL
        }
        if size == 0 {
            unsafe { *memptr = core::ptr::null_mut() };
            return 0;
        }
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        let ptr = unsafe { ALLOC.alloc(layout) };
        if ptr.is_null() {
            12 // ENOMEM
        } else {
            unsafe { *memptr = ptr };
            0
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn aligned_alloc(align: usize, size: usize) -> *mut u8 {
        if !align.is_power_of_two() || (size > 0 && !size.is_multiple_of(align)) {
            return core::ptr::null_mut();
        }
        if size == 0 {
            return align as *mut u8;
        }
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        unsafe { ALLOC.alloc(layout) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn malloc_usable_size(ptr: *mut u8) -> usize {
        unsafe { usable_size(ptr) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn memalign(align: usize, size: usize) -> *mut u8 {
        if !align.is_power_of_two() || size == 0 {
            return core::ptr::null_mut();
        }
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        unsafe { ALLOC.alloc(layout) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn pvalloc(size: usize) -> *mut u8 {
        let rounded = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        unsafe { memalign(PAGE_SIZE, rounded) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn valloc(size: usize) -> *mut u8 {
        unsafe { memalign(PAGE_SIZE, size) }
    }
}

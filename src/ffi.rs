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

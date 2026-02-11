//! C-ABI exports for external linking (e.g., from bench via build.rs).
//!
//! Gated behind `features = ["ffi"]`. Built as part of the staticlib.
//! When built with `--features nightly,ffi`, exports `rstcmalloc_nightly_*`.
//! When built with `--features ffi` only, exports `rstcmalloc_stable_*`.

use crate::allocator::TcMalloc;
use core::alloc::{GlobalAlloc, Layout};

static ALLOC: TcMalloc = TcMalloc;

#[cfg_attr(all(feature = "nightly", feature = "testing"), unsafe(export_name = "rstcmalloc_nightly_alloc"))]
#[cfg_attr(not(all(feature = "nightly", feature = "testing")), unsafe(export_name = "rstcmalloc_stable_alloc"))]
pub unsafe extern "C" fn rstcmalloc_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.alloc(layout) }
}

#[cfg_attr(all(feature = "nightly", feature = "testing"), unsafe(export_name = "rstcmalloc_nightly_dealloc"))]
#[cfg_attr(not(all(feature = "nightly", feature = "testing")), unsafe(export_name = "rstcmalloc_stable_dealloc"))]
pub unsafe extern "C" fn rstcmalloc_dealloc(ptr: *mut u8, size: usize, align: usize) {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.dealloc(ptr, layout) }
}

#[cfg_attr(all(feature = "nightly", feature = "testing"), unsafe(export_name = "rstcmalloc_nightly_realloc"))]
#[cfg_attr(not(all(feature = "nightly", feature = "testing")), unsafe(export_name = "rstcmalloc_stable_realloc"))]
pub unsafe extern "C" fn rstcmalloc_realloc(
    ptr: *mut u8,
    size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.realloc(ptr, layout, new_size) }
}

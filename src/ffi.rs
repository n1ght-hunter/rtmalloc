//! C-ABI exports for external linking (e.g., from bench via build.rs).
//!
//! Gated behind `features = ["ffi"]`. Built as part of the staticlib.
//! With `testing` feature, export names are prefixed by variant:
//!   - `percpu`  → `rstcmalloc_percpu_*`
//!   - `nightly` → `rstcmalloc_nightly_*`
//!   - `std`     → `rstcmalloc_std_*`
//!   - neither   → `rstcmalloc_nostd_*`
//! Without `testing`, exports plain `rstcmalloc_*` names.

use crate::allocator::TcMalloc;
use core::alloc::{GlobalAlloc, Layout};

static ALLOC: TcMalloc = TcMalloc;

// Note: percpu implies nightly, so the percpu check must come first.

#[cfg_attr(not(feature = "testing"), unsafe(no_mangle))]
#[cfg_attr(all(feature = "testing", feature = "percpu"), unsafe(export_name = "rstcmalloc_percpu_alloc"))]
#[cfg_attr(all(feature = "testing", feature = "nightly", not(feature = "percpu")), unsafe(export_name = "rstcmalloc_nightly_alloc"))]
#[cfg_attr(all(feature = "testing", feature = "std", not(any(feature = "nightly", feature = "percpu"))), unsafe(export_name = "rstcmalloc_std_alloc"))]
#[cfg_attr(all(feature = "testing", not(any(feature = "nightly", feature = "std", feature = "percpu"))), unsafe(export_name = "rstcmalloc_nostd_alloc"))]
pub unsafe extern "C" fn rstcmalloc_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.alloc(layout) }
}

#[cfg_attr(not(feature = "testing"), unsafe(no_mangle))]
#[cfg_attr(all(feature = "testing", feature = "percpu"), unsafe(export_name = "rstcmalloc_percpu_dealloc"))]
#[cfg_attr(all(feature = "testing", feature = "nightly", not(feature = "percpu")), unsafe(export_name = "rstcmalloc_nightly_dealloc"))]
#[cfg_attr(all(feature = "testing", feature = "std", not(any(feature = "nightly", feature = "percpu"))), unsafe(export_name = "rstcmalloc_std_dealloc"))]
#[cfg_attr(all(feature = "testing", not(any(feature = "nightly", feature = "std", feature = "percpu"))), unsafe(export_name = "rstcmalloc_nostd_dealloc"))]
pub unsafe extern "C" fn rstcmalloc_dealloc(ptr: *mut u8, size: usize, align: usize) {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.dealloc(ptr, layout) }
}

#[cfg_attr(not(feature = "testing"), unsafe(no_mangle))]
#[cfg_attr(all(feature = "testing", feature = "percpu"), unsafe(export_name = "rstcmalloc_percpu_realloc"))]
#[cfg_attr(all(feature = "testing", feature = "nightly", not(feature = "percpu")), unsafe(export_name = "rstcmalloc_nightly_realloc"))]
#[cfg_attr(all(feature = "testing", feature = "std", not(any(feature = "nightly", feature = "percpu"))), unsafe(export_name = "rstcmalloc_std_realloc"))]
#[cfg_attr(all(feature = "testing", not(any(feature = "nightly", feature = "std", feature = "percpu"))), unsafe(export_name = "rstcmalloc_nostd_realloc"))]
pub unsafe extern "C" fn rstcmalloc_realloc(
    ptr: *mut u8,
    size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { ALLOC.realloc(ptr, layout, new_size) }
}

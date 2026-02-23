#![no_std]
#![cfg_attr(feature = "nightly", feature(thread_local))]

//! rtmalloc: A tcmalloc-style memory allocator for Rust.
//!
//! Implements Google's tcmalloc architecture with three tiers:
//! - Thread-local caches (fast path, no locks) — requires `nightly` feature
//! - Central free lists (per-size-class locking)
//! - Page heap (span management, OS interface)
//!
//! # Usage
//!
//! ```ignore
//! #[global_allocator]
//! static GLOBAL: rtmalloc::RtMalloc = rtmalloc::RtMalloc;
//! ```

#[cfg(test)]
extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod allocator;
pub mod central_free_list;
#[cfg(feature = "percpu")]
pub mod cpu_cache;
#[cfg(feature = "ffi")]
pub mod ffi;
pub mod page_heap;
pub mod pagemap;
pub mod platform;
pub mod size_class;
pub mod span;
pub mod sync;
#[cfg(feature = "stats")]
pub mod stats;
pub mod thread_cache;
pub mod transfer_cache;

/// Page size used by the allocator (8 KiB).
pub const PAGE_SHIFT: usize = 13;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

// Re-export the allocator at crate root for convenience
pub use allocator::RtMalloc;

/// Increment a stats counter by 1.
///
/// Compiles to nothing when the `stats` feature is disabled.
#[macro_export]
macro_rules! stat_inc {
    ($counter:ident) => {
        #[cfg(feature = "stats")]
        {
            $crate::stats::STATS
                .$counter
                .fetch_add(1, ::core::sync::atomic::Ordering::Relaxed);
        }
    };
}

/// Add a value to a stats counter.
///
/// Compiles to nothing (including the value expression) when the `stats`
/// feature is disabled.
#[macro_export]
macro_rules! stat_add {
    ($counter:ident, $val:expr) => {
        #[cfg(feature = "stats")]
        {
            $crate::stats::STATS
                .$counter
                .fetch_add($val as u64, ::core::sync::atomic::Ordering::Relaxed);
        }
    };
}

// Panic handler for staticlib builds (no_std has no default panic handler).
// Only active when panic="abort" (i.e., the `fast` profile), not during normal checks.
#[cfg(all(feature = "ffi", not(test), not(feature = "std"), panic = "abort"))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe extern "C" {
        fn abort() -> !;
    }
    unsafe { abort() }
}

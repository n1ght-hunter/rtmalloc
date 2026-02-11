#![no_std]
#![cfg_attr(feature = "nightly", feature(thread_local))]

//! rstcmalloc: A tcmalloc-style memory allocator for Rust.
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
//! static GLOBAL: rstcmalloc::TcMalloc = rstcmalloc::TcMalloc;
//! ```

#[cfg(test)]
extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod size_class;
pub mod platform;
pub mod sync;
pub mod span;
pub mod pagemap;
pub mod page_heap;
pub mod central_free_list;
pub mod transfer_cache;
pub mod thread_cache;
pub mod allocator;
#[cfg(feature = "ffi")]
pub mod ffi;

/// Page size used by the allocator (8 KiB).
pub const PAGE_SHIFT: usize = 13;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

// Re-export the allocator at crate root for convenience
pub use allocator::TcMalloc;

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

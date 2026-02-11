#![feature(thread_local)]

//! rstcmalloc: A tcmalloc-style memory allocator for Rust.
//!
//! Implements Google's tcmalloc architecture with three tiers:
//! - Thread-local caches (fast path, no locks)
//! - Central free lists (per-size-class locking)
//! - Page heap (span management, OS interface)
//!
//! # Usage
//!
//! ```ignore
//! #[global_allocator]
//! static GLOBAL: rstcmalloc::TcMalloc = rstcmalloc::TcMalloc;
//! ```

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

/// Page size used by the allocator (8 KiB).
pub const PAGE_SHIFT: usize = 13;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

// Re-export the allocator at crate root for convenience
pub use allocator::TcMalloc;

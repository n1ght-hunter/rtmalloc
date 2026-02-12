//! `rseq` — Linux restartable sequences for Rust.
//!
//! Zero-dependency, `no_std` wrapper around the Linux rseq(2) syscall.
//! Provides per-CPU atomic operations without hardware atomics on the
//! fast path — the kernel handles preemption detection.
//!
//! # Features
//!
//! - `nightly` — enables `#[thread_local]` for the self-managed rseq area
//!   and weak-symbol glibc detection. Without this feature, only the raw
//!   ABI types, constants, and syscall wrappers are available.
//!
//! # Architecture support
//!
//! Currently x86_64 only.

#![no_std]
#![cfg_attr(feature = "nightly", feature(thread_local, linkage))]

pub mod abi;
pub mod ops;
pub mod percpu;
pub mod syscall;
pub mod thread;

// Re-export key types at crate root.
pub use abi::{RSEQ_SIG, Rseq, RseqCs};
pub use ops::{percpu_add, percpu_cmpxchg, percpu_load, percpu_store};
pub use percpu::{PerCpuSlab, SlabHeader};
pub use thread::{RseqLocal, current_cpu, current_rseq, rseq_available};

//! Isolated debug test: does 65536 + 131072 crash WITHOUT test-framework interference?
//!
//! Key: we do NOT set #[global_allocator], so the test framework uses the system
//! allocator. Only our explicit calls go through RtMalloc.

use rtmalloc::RtMalloc;
use std::alloc::{GlobalAlloc, Layout};

static ALLOC: RtMalloc = RtMalloc;

/// Just 65536 then 131072, no global_allocator, no warmup
#[test]
fn isolated_65k_then_131k() {
    let l1 = Layout::from_size_align(65536, 1).unwrap();
    let p1 = unsafe { ALLOC.alloc(l1) };
    assert!(!p1.is_null(), "alloc 65536 returned null");
    unsafe { std::ptr::write_bytes(p1, 0xCC, 65536) };
    unsafe { ALLOC.dealloc(p1, l1) };

    let l2 = Layout::from_size_align(131072, 1).unwrap();
    let p2 = unsafe { ALLOC.alloc(l2) };
    assert!(!p2.is_null(), "alloc 131072 returned null");
    unsafe { std::ptr::write_bytes(p2, 0xDD, 131072) };
    unsafe { ALLOC.dealloc(p2, l2) };
}

/// Same but without the write_bytes — is the write_bytes corrupting something?
#[test]
fn isolated_65k_then_131k_no_write() {
    let l1 = Layout::from_size_align(65536, 1).unwrap();
    let p1 = unsafe { ALLOC.alloc(l1) };
    assert!(!p1.is_null(), "alloc 65536 returned null");
    unsafe { ALLOC.dealloc(p1, l1) };

    let l2 = Layout::from_size_align(131072, 1).unwrap();
    let p2 = unsafe { ALLOC.alloc(l2) };
    assert!(!p2.is_null(), "alloc 131072 returned null");
    unsafe { ALLOC.dealloc(p2, l2) };
}

/// Full warmup (8..65536) then 131072, WITHOUT global_allocator
#[test]
fn isolated_full_warmup_then_131k() {
    let sizes = [
        8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
    ];
    for &size in &sizes {
        let layout = Layout::from_size_align(size, 1).unwrap();
        let ptr = unsafe { ALLOC.alloc(layout) };
        assert!(!ptr.is_null(), "alloc {} returned null", size);
        unsafe { std::ptr::write_bytes(ptr, 0xCC, size) };
        unsafe { ALLOC.dealloc(ptr, layout) };
    }

    let l = Layout::from_size_align(131072, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null(), "alloc 131072 returned null");
    unsafe { std::ptr::write_bytes(p, 0xDD, 131072) };
    unsafe { ALLOC.dealloc(p, l) };
}

/// Alloc 131072 cold (no warmup), no global allocator
#[test]
fn isolated_cold_131k() {
    let l = Layout::from_size_align(131072, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null(), "alloc 131072 returned null");
    unsafe { std::ptr::write_bytes(p, 0xDD, 131072) };
    unsafe { ALLOC.dealloc(p, l) };
}

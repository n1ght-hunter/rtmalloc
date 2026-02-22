//! Test each size class's page count in isolation.
//! No #[global_allocator].

use rtmalloc::RtMalloc;
use std::alloc::{GlobalAlloc, Layout};

static ALLOC: RtMalloc = RtMalloc;

// Pages=1 tests
#[test]
fn pages1_size8() {
    let l = Layout::from_size_align(8, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=2 tests
#[test]
fn pages2_size2048() {
    let l = Layout::from_size_align(2048, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=4 tests
#[test]
fn pages4_size4096() {
    let l = Layout::from_size_align(4096, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=5 tests
#[test]
fn pages5_size5120() {
    let l = Layout::from_size_align(5120, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=6 tests
#[test]
fn pages6_size6144() {
    let l = Layout::from_size_align(6144, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=7 tests
#[test]
fn pages7_size7168() {
    let l = Layout::from_size_align(7168, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=8 tests
#[test]
fn pages8_size8192() {
    let l = Layout::from_size_align(8192, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=10 tests
#[test]
fn pages10_size10240() {
    let l = Layout::from_size_align(10240, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=12 tests
#[test]
fn pages12_size12288() {
    let l = Layout::from_size_align(12288, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=16 tests
#[test]
fn pages16_size32768() {
    let l = Layout::from_size_align(32768, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=20 tests
#[test]
fn pages20_size40960() {
    let l = Layout::from_size_align(40960, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

// Pages=32 tests
#[test]
fn pages32_size65536() {
    let l = Layout::from_size_align(65536, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

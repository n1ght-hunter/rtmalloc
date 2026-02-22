//! Ultra-minimal tests: which size first crashes?
//! No #[global_allocator], no println!, no write_bytes.

use rtmalloc::RtMalloc;
use std::alloc::{GlobalAlloc, Layout};

static ALLOC: RtMalloc = RtMalloc;

#[test]
fn alloc_8() {
    let l = Layout::from_size_align(8, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

#[test]
fn alloc_4096() {
    let l = Layout::from_size_align(4096, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

#[test]
fn alloc_8192() {
    let l = Layout::from_size_align(8192, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

#[test]
fn alloc_32768() {
    let l = Layout::from_size_align(32768, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

#[test]
fn alloc_65536() {
    let l = Layout::from_size_align(65536, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

#[test]
fn alloc_131072() {
    let l = Layout::from_size_align(131072, 1).unwrap();
    let p = unsafe { ALLOC.alloc(l) };
    assert!(!p.is_null());
    unsafe { ALLOC.dealloc(p, l) };
}

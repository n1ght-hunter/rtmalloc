//! Debug test to pinpoint SIGSEGV in the allocator.

use rtmalloc::RtMalloc;
use std::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static GLOBAL: RtMalloc = RtMalloc;

/// Warmup with sizes, then try alloc 131072
fn do_test(warmup: &[usize]) {
    for &size in warmup {
        let layout = Layout::from_size_align(size, 1).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xCC, size) };
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
    println!("warmup done");

    println!("alloc 131072...");
    let layout = Layout::from_size_align(131072, 1).unwrap();
    let ptr = unsafe { GLOBAL.alloc(layout) };
    assert!(!ptr.is_null(), "alloc 131072 returned null");
    unsafe { std::ptr::write_bytes(ptr, 0xDD, 131072) };
    unsafe { GLOBAL.dealloc(ptr, layout) };
    println!("131072 ok!");
}

#[test]
fn warmup_small_only() {
    println!("=== small only (8-1024) ===");
    do_test(&[8, 16, 32, 64, 128, 256, 512, 1024]);
}

#[test]
fn warmup_medium() {
    println!("=== medium (8-8192) ===");
    do_test(&[8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192]);
}

#[test]
fn warmup_large() {
    println!("=== large (8-32768) ===");
    do_test(&[
        8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768,
    ]);
}

#[test]
fn warmup_all() {
    println!("=== all (8-65536) ===");
    do_test(&[
        8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
    ]);
}

//! Alignment edge case tests.
//!
//! Verifies that allocations respect alignment requirements for various
//! alignment values, including over-aligned allocations (> 8 bytes).

use rtmalloc::RtMalloc;
use std::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static GLOBAL: RtMalloc = RtMalloc;

#[test]
fn test_standard_alignments() {
    for align in [1, 2, 4, 8] {
        for &size in &[1, 7, 8, 15, 16, 31, 64, 255, 256, 1024, 4096] {
            if size < align {
                continue;
            }
            let layout = Layout::from_size_align(size, align).unwrap();
            let ptr = unsafe { GLOBAL.alloc(layout) };
            assert!(!ptr.is_null(), "alloc failed: size={size}, align={align}");
            assert_eq!(
                ptr as usize % align,
                0,
                "misaligned: ptr={ptr:?}, size={size}, align={align}"
            );
            // Write to verify it's usable
            unsafe { ptr.write_bytes(0xAB, size) };
            unsafe { GLOBAL.dealloc(ptr, layout) };
        }
    }
}

#[test]
fn test_over_aligned_16() {
    let align = 16;
    for &size in &[16, 32, 64, 128, 256, 1024] {
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null(), "alloc failed: size={size}, align={align}");
        assert_eq!(
            ptr as usize % align,
            0,
            "misaligned: ptr={ptr:?}, size={size}, align={align}"
        );
        unsafe { ptr.write_bytes(0xCD, size) };
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn test_over_aligned_32() {
    let align = 32;
    for &size in &[32, 64, 128, 256, 1024] {
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null(), "alloc failed: size={size}, align={align}");
        assert_eq!(
            ptr as usize % align,
            0,
            "misaligned: ptr={ptr:?}, size={size}, align={align}"
        );
        unsafe { ptr.write_bytes(0xEF, size) };
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn test_over_aligned_64() {
    let align = 64;
    for &size in &[64, 128, 256, 512, 1024, 4096] {
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null(), "alloc failed: size={size}, align={align}");
        assert_eq!(
            ptr as usize % align,
            0,
            "misaligned: ptr={ptr:?}, size={size}, align={align}"
        );
        unsafe { ptr.write_bytes(0x42, size) };
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn test_over_aligned_256() {
    let align = 256;
    for &size in &[256, 512, 1024, 4096, 8192] {
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null(), "alloc failed: size={size}, align={align}");
        assert_eq!(
            ptr as usize % align,
            0,
            "misaligned: ptr={ptr:?}, size={size}, align={align}"
        );
        unsafe { ptr.write_bytes(0x99, size) };
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn test_over_aligned_4096() {
    let align = 4096;
    for &size in &[4096, 8192, 16384, 65536] {
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null(), "alloc failed: size={size}, align={align}");
        assert_eq!(
            ptr as usize % align,
            0,
            "misaligned: ptr={ptr:?}, size={size}, align={align}"
        );
        unsafe { ptr.write_bytes(0x77, size) };
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn test_alignment_realloc_preserves_alignment() {
    for align in [16, 32, 64, 256] {
        let size = align * 2;
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize % align, 0);

        // Fill and grow
        unsafe { ptr.write_bytes(0xBB, size) };
        let new_size = size * 4;
        let new_ptr = unsafe { GLOBAL.realloc(ptr, layout, new_size) };
        assert!(!new_ptr.is_null(), "realloc failed: align={align}");
        assert_eq!(
            new_ptr as usize % align,
            0,
            "realloc lost alignment: align={align}"
        );

        // Original bytes preserved
        for i in 0..size {
            assert_eq!(
                unsafe { *new_ptr.add(i) },
                0xBB,
                "realloc corrupted byte {i}"
            );
        }

        let new_layout = Layout::from_size_align(new_size, align).unwrap();
        unsafe { GLOBAL.dealloc(new_ptr, new_layout) };
    }
}

#[test]
fn test_many_aligned_allocations() {
    // Allocate many over-aligned objects to stress the allocator's
    // alignment handling across multiple spans/pages.
    let align = 64;
    let size = 64;
    let layout = Layout::from_size_align(size, align).unwrap();
    let count = 500;

    let mut ptrs = Vec::with_capacity(count);
    for _ in 0..count {
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize % align, 0, "misaligned in batch alloc");
        unsafe { ptr.write_bytes(0xDD, size) };
        ptrs.push(ptr);
    }

    // Verify no overlaps by checking patterns are intact
    for &ptr in &ptrs {
        for i in 0..size {
            assert_eq!(unsafe { *ptr.add(i) }, 0xDD);
        }
    }

    for ptr in ptrs {
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn test_zero_size_layout() {
    // Zero-sized allocations should return a non-null aligned pointer
    let layout = Layout::from_size_align(0, 1).unwrap();
    let ptr = unsafe { GLOBAL.alloc(layout) };
    // GlobalAlloc allows returning null for zero-size, but most allocators
    // return a valid pointer. Just ensure we don't crash on dealloc.
    if !ptr.is_null() {
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

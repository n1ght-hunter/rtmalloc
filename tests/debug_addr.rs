//! Diagnostic: print mmap addresses and page IDs to check alignment.

use rtmalloc::RtMalloc;
use std::alloc::{GlobalAlloc, Layout};

static ALLOC: RtMalloc = RtMalloc;

/// Alloc several sizes and print the returned pointers
#[test]
fn print_addrs() {
    let sizes = [8, 64, 256, 1024, 2048, 4096, 8192, 16384, 32768];
    for &size in &sizes {
        let layout = Layout::from_size_align(size, 1).unwrap();
        let ptr = unsafe { ALLOC.alloc(layout) };
        if ptr.is_null() {
            println!("size {:>6}: NULL", size);
        } else {
            let addr = ptr as usize;
            let page_id = addr >> 13;
            let page_offset = addr & 0x1FFF;
            let reconstructed = page_id << 13;
            println!(
                "size {:>6}: addr=0x{:016x} page_id={} offset_in_page={} reconstructed=0x{:016x} match={}",
                size,
                addr,
                page_id,
                page_offset,
                reconstructed,
                reconstructed == addr
            );
            unsafe { ALLOC.dealloc(ptr, layout) };
        }
    }
}

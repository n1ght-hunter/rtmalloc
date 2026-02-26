//! Miri-compatible platform shim using std::alloc as backing store.
//!
//! Miri can't execute real OS syscalls (mmap/VirtualAlloc), so we use the
//! system allocator to provide page-aligned memory. This lets Miri check
//! all the unsafe pointer logic in the allocator internals.

extern crate alloc;

use core::alloc::Layout;

pub unsafe fn page_alloc(size: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size, crate::config::PAGE_SIZE).unwrap();
    unsafe { alloc::alloc::alloc_zeroed(layout) }
}

pub unsafe fn page_dealloc(ptr: *mut u8, size: usize) {
    let layout = Layout::from_size_align(size, crate::config::PAGE_SIZE).unwrap();
    unsafe { alloc::alloc::dealloc(ptr, layout) };
}

pub unsafe fn page_decommit(_ptr: *mut u8, _size: usize) {}

pub unsafe fn page_recommit(_ptr: *mut u8, _size: usize) {}

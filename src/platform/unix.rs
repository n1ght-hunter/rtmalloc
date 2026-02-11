//! Unix virtual memory implementation using mmap/munmap.

use core::ffi::c_void;

const PROT_READ: i32 = 0x1;
const PROT_WRITE: i32 = 0x2;
const MAP_PRIVATE: i32 = 0x02;
const MAP_ANONYMOUS: i32 = 0x20;
const MAP_FAILED: *mut c_void = !0usize as *mut c_void;
const MADV_DONTNEED: i32 = 4;

unsafe extern "C" {
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void;

    fn munmap(addr: *mut c_void, length: usize) -> i32;

    fn madvise(addr: *mut c_void, length: usize, advice: i32) -> i32;
}

pub unsafe fn page_alloc(size: usize) -> *mut u8 {
    let ptr = mmap(
        core::ptr::null_mut(),
        size,
        PROT_READ | PROT_WRITE,
        MAP_PRIVATE | MAP_ANONYMOUS,
        -1,
        0,
    );
    if ptr == MAP_FAILED {
        core::ptr::null_mut()
    } else {
        ptr as *mut u8
    }
}

pub unsafe fn page_dealloc(ptr: *mut u8, size: usize) {
    munmap(ptr as *mut c_void, size);
}

pub unsafe fn page_decommit(ptr: *mut u8, size: usize) {
    madvise(ptr as *mut c_void, size, MADV_DONTNEED);
}

//! Windows virtual memory implementation using VirtualAlloc/VirtualFree.

use core::ffi::c_void;

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const MEM_DECOMMIT: u32 = 0x4000;
const PAGE_READWRITE: u32 = 0x04;

// Windows allocation granularity is 64 KiB.
const ALLOC_GRANULARITY: usize = 65536;

unsafe extern "system" {
    #[link_name = "VirtualAlloc"]
    fn virtual_alloc(
        lp_address: *mut c_void,
        dw_size: usize,
        fl_allocation_type: u32,
        fl_protect: u32,
    ) -> *mut c_void;

    #[link_name = "VirtualFree"]
    fn virtual_free(
        lp_address: *mut c_void,
        dw_size: usize,
        dw_free_type: u32,
    ) -> i32;
}

/// Round up to the next multiple of `align` (must be a power of 2).
#[inline]
const fn round_up(size: usize, align: usize) -> usize {
    (size + align - 1) & !(align - 1)
}

pub unsafe fn page_alloc(size: usize) -> *mut u8 {
    let alloc_size = round_up(size, ALLOC_GRANULARITY);
    let ptr = unsafe {
        virtual_alloc(
            core::ptr::null_mut(),
            alloc_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    ptr as *mut u8
}

pub unsafe fn page_dealloc(ptr: *mut u8) {
    // MEM_RELEASE requires dwSize = 0 (releases entire allocation)
    unsafe { virtual_free(ptr as *mut c_void, 0, MEM_RELEASE) };
}

pub unsafe fn page_decommit(ptr: *mut u8, size: usize) {
    unsafe { virtual_free(ptr as *mut c_void, size, MEM_DECOMMIT) };
}

pub unsafe fn page_recommit(ptr: *mut u8, size: usize) {
    unsafe {
        virtual_alloc(
            ptr as *mut c_void,
            size,
            MEM_COMMIT,
            PAGE_READWRITE,
        )
    };
}

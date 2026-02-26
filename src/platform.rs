//! OS platform abstraction for virtual memory allocation.
//!
//! Provides `page_alloc` and `page_dealloc` that wrap platform-specific
//! virtual memory APIs (VirtualAlloc on Windows, mmap on Unix).
//! Under Miri, uses std::alloc as a backing store instead.

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        mod miri;
    } else if #[cfg(windows)] {
        mod windows;
    } else if #[cfg(unix)] {
        mod unix;
    }
}

/// Allocate `size` bytes of virtual memory, page-aligned.
/// Returns null on failure. Memory is zero-initialized by the OS.
/// `size` is rounded up to the platform allocation granularity.
///
/// # Safety
/// Caller must eventually call `page_dealloc` with the returned pointer and the
/// same `size` (before rounding).
#[inline]
pub unsafe fn page_alloc(size: usize) -> *mut u8 {
    cfg_if::cfg_if! {
        if #[cfg(miri)] {
            unsafe { miri::page_alloc(size) }
        } else if #[cfg(windows)] {
            unsafe { windows::page_alloc(size) }
        } else if #[cfg(unix)] {
            unsafe { unix::page_alloc(size) }
        }
    }
}

/// Free virtual memory previously allocated by `page_alloc`.
///
/// # Safety
/// `ptr` must have been returned by `page_alloc`, and `size` must match
/// the original allocation size.
#[inline]
pub unsafe fn page_dealloc(ptr: *mut u8, size: usize) {
    cfg_if::cfg_if! {
        if #[cfg(miri)] {
            unsafe { miri::page_dealloc(ptr, size) }
        } else if #[cfg(windows)] {
            let _ = size;
            unsafe { windows::page_dealloc(ptr) }
        } else if #[cfg(unix)] {
            unsafe { unix::page_dealloc(ptr, size) }
        }
    }
}

/// Decommit pages (return physical memory to OS but keep virtual address range).
/// On Windows this uses MEM_DECOMMIT; on Unix this uses madvise(MADV_DONTNEED).
///
/// # Safety
/// `ptr` and `size` must refer to a range within a live `page_alloc` allocation.
#[inline]
pub unsafe fn page_decommit(ptr: *mut u8, size: usize) {
    cfg_if::cfg_if! {
        if #[cfg(miri)] {
            unsafe { miri::page_decommit(ptr, size) }
        } else if #[cfg(windows)] {
            unsafe { windows::page_decommit(ptr, size) }
        } else if #[cfg(unix)] {
            unsafe { unix::page_decommit(ptr, size) }
        }
    }
}

/// Recommit previously decommitted pages.
///
/// # Safety
/// `ptr` and `size` must refer to a range within a live `page_alloc` allocation
/// that was previously decommitted.
#[inline]
pub unsafe fn page_recommit(ptr: *mut u8, size: usize) {
    cfg_if::cfg_if! {
        if #[cfg(miri)] {
            unsafe { miri::page_recommit(ptr, size) }
        } else if #[cfg(windows)] {
            unsafe { windows::page_recommit(ptr, size) }
        } else if #[cfg(unix)] {
            // madvise MADV_DONTNEED doesn't unmap, so accessing the
            // pages again automatically recommits them. Nothing to do.
            let _ = (ptr, size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PAGE_SIZE;

    #[test]
    fn test_alloc_and_dealloc() {
        unsafe {
            let ptr = page_alloc(PAGE_SIZE);
            assert!(!ptr.is_null());
            for i in 0..PAGE_SIZE {
                assert_eq!(*ptr.add(i), 0);
            }
            for i in 0..PAGE_SIZE {
                *ptr.add(i) = (i & 0xFF) as u8;
            }
            for i in 0..PAGE_SIZE {
                assert_eq!(*ptr.add(i), (i & 0xFF) as u8);
            }
            page_dealloc(ptr, PAGE_SIZE);
        }
    }

    #[test]
    fn test_alloc_multiple_pages() {
        unsafe {
            let size = PAGE_SIZE * 8;
            let ptr = page_alloc(size);
            assert!(!ptr.is_null());
            *ptr = 0xAA;
            *ptr.add(size - 1) = 0xBB;
            assert_eq!(*ptr, 0xAA);
            assert_eq!(*ptr.add(size - 1), 0xBB);
            page_dealloc(ptr, size);
        }
    }

    #[test]
    fn test_alloc_large() {
        unsafe {
            let size = 1024 * 1024; // 1 MiB
            let ptr = page_alloc(size);
            assert!(!ptr.is_null());
            page_dealloc(ptr, size);
        }
    }
}

//! OS platform abstraction for virtual memory allocation.
//!
//! Provides `page_alloc` and `page_dealloc` that wrap platform-specific
//! virtual memory APIs (VirtualAlloc on Windows, mmap on Unix).

#[cfg(windows)]
mod windows;

#[cfg(unix)]
mod unix;


/// Allocate `size` bytes of virtual memory, page-aligned.
/// Returns null on failure. Memory is zero-initialized by the OS.
/// `size` is rounded up to the platform allocation granularity.
///
/// # Safety
/// Caller must eventually call `page_dealloc` with the returned pointer and the
/// same `size` (before rounding).
#[inline]
pub unsafe fn page_alloc(size: usize) -> *mut u8 {
    #[cfg(windows)]
    {
        unsafe { windows::page_alloc(size) }
    }
    #[cfg(unix)]
    {
        unsafe { unix::page_alloc(size) }
    }
}

/// Free virtual memory previously allocated by `page_alloc`.
///
/// # Safety
/// `ptr` must have been returned by `page_alloc`, and `size` must match
/// the original allocation size.
#[inline]
pub unsafe fn page_dealloc(ptr: *mut u8, _size: usize) {
    #[cfg(windows)]
    {
        unsafe { windows::page_dealloc(ptr) };
    }
    #[cfg(unix)]
    {
        unsafe { unix::page_dealloc(ptr, _size) };
    }
}

/// Decommit pages (return physical memory to OS but keep virtual address range).
/// On Windows this uses MEM_DECOMMIT; on Unix this uses madvise(MADV_DONTNEED).
///
/// # Safety
/// `ptr` and `size` must refer to a range within a live `page_alloc` allocation.
#[inline]
pub unsafe fn page_decommit(ptr: *mut u8, size: usize) {
    #[cfg(windows)]
    {
        unsafe { windows::page_decommit(ptr, size) };
    }
    #[cfg(unix)]
    {
        unsafe { unix::page_decommit(ptr, size) };
    }
}

/// Recommit previously decommitted pages.
///
/// # Safety
/// `ptr` and `size` must refer to a range within a live `page_alloc` allocation
/// that was previously decommitted.
#[inline]
pub unsafe fn page_recommit(ptr: *mut u8, size: usize) {
    #[cfg(windows)]
    {
        unsafe { windows::page_recommit(ptr, size) };
    }
    #[cfg(unix)]
    {
        // On Unix, madvise MADV_DONTNEED doesn't unmap, so accessing the
        // pages again automatically recommits them. Nothing to do.
        let _ = (ptr, size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PAGE_SIZE;

    #[test]
    fn test_alloc_and_dealloc() {
        unsafe {
            let ptr = page_alloc(PAGE_SIZE);
            assert!(!ptr.is_null());
            // Memory should be zero-initialized
            for i in 0..PAGE_SIZE {
                assert_eq!(*ptr.add(i), 0);
            }
            // Write a pattern
            for i in 0..PAGE_SIZE {
                *ptr.add(i) = (i & 0xFF) as u8;
            }
            // Read it back
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
            // Write and read back
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

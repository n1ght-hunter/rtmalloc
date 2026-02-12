//! Raw rseq syscall via inline assembly.
//!
//! Invokes syscall #334 directly — no libc wrapper.

use core::arch::asm;

use crate::abi::{RSEQ_FLAG_UNREGISTER, RSEQ_MIN_SIZE, RSEQ_SIG, Rseq, SYS_RSEQ};

/// Issue the raw rseq syscall.
///
/// # Safety
///
/// - `rseq` must point to a valid, 32-byte-aligned `Rseq` that lives for
///   the lifetime of the calling thread (or until unregistered).
/// - `len` must be >= `RSEQ_MIN_SIZE`.
/// - Must only be called on Linux x86_64.
#[inline(always)]
pub unsafe fn raw_rseq(rseq: *mut Rseq, len: u32, flags: i32, sig: u32) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") SYS_RSEQ,
            in("rdi") rseq as u64,
            in("rsi") len as u64,
            in("rdx") flags as u64,
            in("r10") sig as u64,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    ret
}

/// Register this thread's rseq area with the kernel.
///
/// On success the kernel will maintain `cpu_id`, `cpu_id_start`, `node_id`,
/// and `mm_cid` across context switches.
///
/// # Safety
///
/// - `rseq` must point to a properly aligned `Rseq` in thread-local storage.
/// - The area must remain valid for the thread's lifetime (or until [`rseq_unregister`]).
pub unsafe fn rseq_register(rseq: *mut Rseq) -> Result<(), i32> {
    let ret = unsafe { raw_rseq(rseq, RSEQ_MIN_SIZE, 0, RSEQ_SIG) };
    if ret == 0 { Ok(()) } else { Err(ret as i32) }
}

/// Unregister this thread's rseq area.
///
/// After this call the kernel stops updating the rseq fields and the
/// memory may be freed.
///
/// # Safety
///
/// - `rseq` must be the same pointer that was previously registered.
pub unsafe fn rseq_unregister(rseq: *mut Rseq) -> Result<(), i32> {
    let ret = unsafe { raw_rseq(rseq, RSEQ_MIN_SIZE, RSEQ_FLAG_UNREGISTER, RSEQ_SIG) };
    if ret == 0 { Ok(()) } else { Err(ret as i32) }
}

/// Linux syscall returns negative errno on failure.
pub const ENOSYS: i32 = -38;
pub const EBUSY: i32 = -16;
pub const EINVAL: i32 = -22;
pub const EFAULT: i32 = -14;
pub const EPERM: i32 = -1;

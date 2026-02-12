//! Per-CPU atomic operations using rseq critical sections (x86_64).
//!
//! Each primitive uses a restartable sequence: the kernel monitors the
//! thread's instruction pointer and redirects to the abort handler if a
//! preemption, signal, or CPU migration occurs within the critical section.
//!
//! The fast path executes with zero atomic instructions — all
//! synchronisation is handled by the kernel's preemption detection.
//!
//! # Abort handler contract
//!
//! Every abort label must be preceded by the 4-byte `RSEQ_SIG` signature
//! (`0x53053053`). On x86_64 this encodes as `ud1 %edi, %eax`, a
//! guaranteed-illegal instruction that provides control-flow integrity.

use core::arch::asm;

use crate::abi::Rseq;

/// Byte offset of `rseq_cs` within `struct Rseq`.
const RSEQ_CS_OFFSET: u32 = 8;

/// Byte offset of `cpu_id` within `struct Rseq`.
const CPU_ID_OFFSET: u32 = 4;

/// Load a `u64` value from `array[cpu_id]`.
///
/// Returns `Some((cpu, value))` on success, or `None` if rseq aborted
/// (caller should retry).
///
/// # Safety
///
/// - `rseq` must be a valid, registered rseq pointer for the current thread.
/// - `array` must point to a valid array of `u64` with at least
///   `num_possible_cpus` elements.
#[inline(never)]
pub unsafe fn percpu_load(rseq: *mut Rseq, array: *const u64) -> Option<(u32, u64)> {
    let cpu: u64;
    let value: u64;
    let success: u64;

    unsafe {
        asm!(
            // rseq_cs descriptor in a relocatable data section.
            ".pushsection __rseq_cs, \"aw\"",
            ".balign 32",
            "77:",
            ".long 0",                         // version
            ".long 0",                         // flags
            ".quad 3f",                        // start_ip
            ".quad (4f - 3f)",                 // post_commit_offset
            ".quad 6f",                        // abort_ip
            ".popsection",

            // Store rseq_cs pointer into rseq->rseq_cs
            "lea {tmp}, [rip + 77b]",
            "mov qword ptr [{rseq} + {rseq_cs_off}], {tmp}",

            // -- start_ip --
            "3:",
            // Read cpu_id
            "mov {cpu:e}, dword ptr [{rseq} + {cpu_id_off}]",

            // Load value from array[cpu_id]
            "mov {val}, qword ptr [{array} + {cpu} * 8]",

            // -- commit (the load itself is the commit point) --
            "4:",

            // Clear rseq_cs
            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",

            // Signal success
            "mov {succ}, 1",
            "jmp 5f",

            ".long 0x53053053",                // RSEQ_SIG before abort_ip
            "6:",
            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "xor {succ:e}, {succ:e}",          // success = 0

            "5:",

            rseq = in(reg) rseq,
            array = in(reg) array,
            cpu = out(reg) cpu,
            val = out(reg) value,
            succ = out(reg) success,
            tmp = out(reg) _,
            rseq_cs_off = const RSEQ_CS_OFFSET,
            cpu_id_off = const CPU_ID_OFFSET,
            options(nostack),
        );
    }

    if success != 0 {
        Some((cpu as u32, value))
    } else {
        None
    }
}

/// Store a `u64` value to `array[cpu_id]`.
///
/// Returns `Some(cpu)` on success, or `None` if aborted (retry).
///
/// # Safety
///
/// - `rseq` must be a valid, registered rseq pointer for the current thread.
/// - `array` must point to a valid array of `u64` with at least
///   `num_possible_cpus` elements.
#[inline(never)]
pub unsafe fn percpu_store(rseq: *mut Rseq, array: *mut u64, value: u64) -> Option<u32> {
    let cpu: u64;
    let success: u64;

    unsafe {
        asm!(
            ".pushsection __rseq_cs, \"aw\"",
            ".balign 32",
            "77:",
            ".long 0",
            ".long 0",
            ".quad 3f",
            ".quad (4f - 3f)",
            ".quad 6f",
            ".popsection",

            "lea {tmp}, [rip + 77b]",
            "mov qword ptr [{rseq} + {rseq_cs_off}], {tmp}",

            "3:",
            "mov {cpu:e}, dword ptr [{rseq} + {cpu_id_off}]",

            // Commit: store value into array[cpu_id]
            "mov qword ptr [{array} + {cpu} * 8], {val}",
            "4:",

            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "mov {succ}, 1",
            "jmp 5f",

            ".long 0x53053053",
            "6:",
            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "xor {succ:e}, {succ:e}",

            "5:",

            rseq = in(reg) rseq,
            array = in(reg) array,
            val = in(reg) value,
            cpu = out(reg) cpu,
            succ = out(reg) success,
            tmp = out(reg) _,
            rseq_cs_off = const RSEQ_CS_OFFSET,
            cpu_id_off = const CPU_ID_OFFSET,
            options(nostack),
        );
    }

    if success != 0 { Some(cpu as u32) } else { None }
}

/// Add `delta` to `array[cpu_id]` (u64 element).
///
/// Returns `Some(cpu)` on success, or `None` if aborted (retry).
///
/// # Safety
///
/// Same requirements as [`percpu_store`].
#[inline(never)]
pub unsafe fn percpu_add(rseq: *mut Rseq, array: *mut u64, delta: u64) -> Option<u32> {
    let cpu: u64;
    let success: u64;

    unsafe {
        asm!(
            ".pushsection __rseq_cs, \"aw\"",
            ".balign 32",
            "77:",
            ".long 0",
            ".long 0",
            ".quad 3f",
            ".quad (4f - 3f)",
            ".quad 6f",
            ".popsection",

            "lea {tmp}, [rip + 77b]",
            "mov qword ptr [{rseq} + {rseq_cs_off}], {tmp}",

            "3:",
            "mov {cpu:e}, dword ptr [{rseq} + {cpu_id_off}]",

            // Load current value, add delta, store back (commit).
            "mov {scratch}, qword ptr [{array} + {cpu} * 8]",
            "add {scratch}, {delta}",
            "mov qword ptr [{array} + {cpu} * 8], {scratch}",
            "4:",

            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "mov {succ}, 1",
            "jmp 5f",

            ".long 0x53053053",
            "6:",
            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "xor {succ:e}, {succ:e}",

            "5:",

            rseq = in(reg) rseq,
            array = in(reg) array,
            delta = in(reg) delta,
            cpu = out(reg) cpu,
            succ = out(reg) success,
            tmp = out(reg) _,
            scratch = out(reg) _,
            rseq_cs_off = const RSEQ_CS_OFFSET,
            cpu_id_off = const CPU_ID_OFFSET,
            options(nostack),
        );
    }

    if success != 0 { Some(cpu as u32) } else { None }
}

/// Compare-and-exchange on `array[cpu_id]`.
///
/// If `array[cpu_id] == expected`, stores `new` and returns
/// `Ok((cpu, expected))`. Otherwise returns `Err(actual)` with the
/// value that was found. Also returns `Err` on abort (CPU migration).
///
/// # Safety
///
/// Same requirements as [`percpu_store`].
#[inline(never)]
pub unsafe fn percpu_cmpxchg(
    rseq: *mut Rseq,
    array: *mut u64,
    expected: u64,
    new: u64,
) -> Result<(u32, u64), u64> {
    let cpu: u64;
    let old_val: u64;
    let success: u64;

    unsafe {
        asm!(
            ".pushsection __rseq_cs, \"aw\"",
            ".balign 32",
            "77:",
            ".long 0",
            ".long 0",
            ".quad 3f",
            ".quad (4f - 3f)",
            ".quad 6f",
            ".popsection",

            "lea {tmp}, [rip + 77b]",
            "mov qword ptr [{rseq} + {rseq_cs_off}], {tmp}",

            "3:",
            "mov {cpu:e}, dword ptr [{rseq} + {cpu_id_off}]",

            // Load current value
            "mov {old}, qword ptr [{array} + {cpu} * 8]",

            // Compare with expected
            "cmp {old}, {exp}",
            "jne 7f",                          // mismatch → fail path

            // Commit: store new value
            "mov qword ptr [{array} + {cpu} * 8], {new}",
            "4:",

            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "mov {succ}, 1",
            "jmp 5f",

            "7:",
            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "xor {succ:e}, {succ:e}",
            "jmp 5f",

            ".long 0x53053053",
            "6:",
            "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
            "xor {succ:e}, {succ:e}",
            // Set old to expected so caller knows it was an abort,
            // not a value mismatch (real value unknown after abort).
            "mov {old}, {exp}",

            "5:",

            rseq = in(reg) rseq,
            array = in(reg) array,
            exp = in(reg) expected,
            new = in(reg) new,
            cpu = out(reg) cpu,
            old = out(reg) old_val,
            succ = out(reg) success,
            tmp = out(reg) _,
            rseq_cs_off = const RSEQ_CS_OFFSET,
            cpu_id_off = const CPU_ID_OFFSET,
            options(nostack),
        );
    }

    if success != 0 {
        Ok((cpu as u32, old_val))
    } else {
        Err(old_val)
    }
}

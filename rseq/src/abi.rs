//! Linux rseq kernel ABI types and constants.
//!
//! Defines the structures shared between userspace and the kernel for
//! restartable sequences (rseq). These must match the kernel's layout exactly.

// ── Syscall ──────────────────────────────────────────────────────────────────

/// rseq syscall number on x86_64.
pub const SYS_RSEQ: u64 = 334;

// ── Registration flags (passed to syscall `flags` parameter) ─────────────────

/// Unregister the current thread's rseq area.
pub const RSEQ_FLAG_UNREGISTER: i32 = 1 << 0;

// ── Signature ────────────────────────────────────────────────────────────────

/// x86_64 rseq abort signature. Must appear as the 4 bytes immediately
/// before every abort handler IP. Encodes as `ud1 %edi, %eax` which is
/// a guaranteed-illegal instruction, providing control-flow integrity.
pub const RSEQ_SIG: u32 = 0x53053053;

// ── CPU ID sentinel values ───────────────────────────────────────────────────

/// cpu_id value before the kernel first schedules the thread.
pub const RSEQ_CPU_ID_UNINITIALIZED: u32 = u32::MAX; // -1 as u32

/// cpu_id value if registration failed.
pub const RSEQ_CPU_ID_REGISTRATION_FAILED: u32 = u32::MAX - 1; // -2 as u32

// ── Critical section flags (rseq_cs::flags) ──────────────────────────────────

/// Don't restart the critical section on preemption.
pub const RSEQ_CS_FLAG_NO_RESTART_ON_PREEMPT: u32 = 1 << 0;

/// Don't restart the critical section on signal delivery.
pub const RSEQ_CS_FLAG_NO_RESTART_ON_SIGNAL: u32 = 1 << 1;

/// Don't restart the critical section on CPU migration.
pub const RSEQ_CS_FLAG_NO_RESTART_ON_MIGRATE: u32 = 1 << 2;

// ── Struct offsets (for use in inline asm) ───────────────────────────────────

/// Byte offset of `cpu_id_start` within `struct rseq`.
pub const RSEQ_OFF_CPU_ID_START: u32 = 0;

/// Byte offset of `cpu_id` within `struct rseq`.
pub const RSEQ_OFF_CPU_ID: u32 = 4;

/// Byte offset of `rseq_cs` pointer within `struct rseq`.
pub const RSEQ_OFF_RSEQ_CS: u32 = 8;

/// Byte offset of `flags` within `struct rseq`.
pub const RSEQ_OFF_FLAGS: u32 = 16;

/// Byte offset of `node_id` within `struct rseq`.
pub const RSEQ_OFF_NODE_ID: u32 = 20;

/// Byte offset of `mm_cid` within `struct rseq`.
pub const RSEQ_OFF_MM_CID: u32 = 24;

// ── struct rseq ──────────────────────────────────────────────────────────────

/// Per-thread rseq area shared with the kernel.
///
/// Must be 32-byte aligned. The kernel reads and writes `cpu_id`,
/// `cpu_id_start`, `node_id`, and `mm_cid` on context switches.
/// Userspace reads these fields and writes `rseq_cs` to activate
/// a critical section.
#[repr(C, align(32))]
pub struct Rseq {
    /// CPU number at the start of the current critical section.
    /// Always reflects a valid CPU number even outside a critical section.
    pub cpu_id_start: u32,

    /// Current CPU number. Set to `RSEQ_CPU_ID_UNINITIALIZED` before
    /// the first schedule, or `RSEQ_CPU_ID_REGISTRATION_FAILED` if
    /// registration failed.
    pub cpu_id: u32,

    /// Pointer to the active `RseqCs` descriptor, or 0 if no critical
    /// section is active. Userspace stores a pointer here before entering
    /// a critical section; the kernel clears it on abort.
    pub rseq_cs: u64,

    /// Flags controlling restart behavior.
    pub flags: u32,

    /// NUMA node ID (kernel >= 5.17).
    pub node_id: u32,

    /// Memory-map concurrency ID (kernel >= 5.17).
    pub mm_cid: u32,

    /// NUMA-aware memory-map concurrency ID.
    pub mm_numa_cid: u32,
}

/// Minimum size to pass to the rseq syscall for the original ABI (v0).
pub const RSEQ_MIN_SIZE: u32 = 32;

impl Default for Rseq {
    fn default() -> Self {
        Self::new()
    }
}

impl Rseq {
    /// Create a zeroed, uninitialized rseq area.
    pub const fn new() -> Self {
        Self {
            cpu_id_start: 0,
            cpu_id: RSEQ_CPU_ID_UNINITIALIZED,
            rseq_cs: 0,
            flags: 0,
            node_id: 0,
            mm_cid: 0,
            mm_numa_cid: 0,
        }
    }
}

// ── struct rseq_cs ───────────────────────────────────────────────────────────

/// Critical section descriptor.
///
/// Describes the boundaries of a restartable sequence. Must be 32-byte
/// aligned. The kernel checks if the thread's instruction pointer falls
/// within `[start_ip, start_ip + post_commit_offset)` on preemption;
/// if so, it redirects execution to `abort_ip`.
#[repr(C, align(32))]
pub struct RseqCs {
    /// Structure version. Must be 0.
    pub version: u32,

    /// Flags controlling restart behavior for this critical section.
    pub flags: u32,

    /// Address of the first instruction in the critical section.
    pub start_ip: u64,

    /// Byte offset from `start_ip` to the first instruction *after*
    /// the commit point. The critical section covers
    /// `[start_ip, start_ip + post_commit_offset)`.
    pub post_commit_offset: u64,

    /// Address of the abort handler. The 4 bytes immediately before
    /// this address must contain `RSEQ_SIG`.
    pub abort_ip: u64,
}

impl Default for RseqCs {
    fn default() -> Self {
        Self::new()
    }
}

impl RseqCs {
    /// Create a zeroed critical section descriptor.
    pub const fn new() -> Self {
        Self {
            version: 0,
            flags: 0,
            start_ip: 0,
            post_commit_offset: 0,
            abort_ip: 0,
        }
    }
}

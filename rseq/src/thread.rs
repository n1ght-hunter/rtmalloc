//! Per-thread rseq area management and glibc interop.
//!
//! Supports two modes:
//!
//! **Mode A — glibc-managed (glibc >= 2.35):**
//! glibc registers rseq automatically. We detect this via weak symbols
//! (`__rseq_offset`, `__rseq_size`) and compute the pointer from the
//! thread pointer.
//!
//! **Mode B — self-managed:**
//! We own a `#[thread_local]` `Rseq` and register it ourselves via
//! the raw syscall. Requires the `nightly` feature.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::abi::{RSEQ_CPU_ID_REGISTRATION_FAILED, RSEQ_CPU_ID_UNINITIALIZED, Rseq};

// ── glibc weak symbols ──────────────────────────────────────────────────────

// These symbols are exported by glibc >= 2.35 when it auto-registers rseq.
// We declare them as weak so linking succeeds even without glibc.
//
// NOTE: `#[linkage = "extern_weak"]` requires nightly. When building on
// stable without glibc detection, we fall back to self-managed only.
#[cfg(feature = "nightly")]
unsafe extern "C" {
    #[linkage = "extern_weak"]
    static __rseq_offset: *const i32;
    #[linkage = "extern_weak"]
    static __rseq_size: *const u32;
}

/// Check whether glibc has already registered rseq for us.
#[cfg(feature = "nightly")]
fn glibc_rseq_registered() -> bool {
    unsafe {
        // If the weak symbol resolved to non-null, glibc is present.
        let size_ptr: *const *const u32 = &raw const __rseq_size;
        if (*size_ptr).is_null() {
            return false;
        }
        // The symbol itself is the value (not a pointer to a pointer).
        // With extern_weak linkage, a resolved symbol has a non-null address
        // and we read the value at that address.
        let size_val = *(*size_ptr);
        size_val > 0
    }
}

/// Get the rseq pointer from glibc's thread control block.
///
/// # Safety
///
/// Only call this after confirming [`glibc_rseq_registered`] returns true.
#[cfg(feature = "nightly")]
unsafe fn glibc_rseq_ptr() -> *mut Rseq {
    use core::arch::asm;

    let offset: i64;
    unsafe {
        let offset_ptr: *const *const i32 = &raw const __rseq_offset;
        offset = (**offset_ptr) as i64;
    }

    // Read the thread pointer from the `fs` segment base (x86_64 Linux ABI).
    let tp: u64;
    unsafe {
        asm!(
            "mov {tp}, fs:0",
            tp = out(reg) tp,
            options(nostack, preserves_flags, readonly, pure)
        );
    }

    (tp as i64 + offset) as *mut Rseq
}

// ── Self-managed rseq area ───────────────────────────────────────────────────

#[cfg(feature = "nightly")]
#[thread_local]
static mut LOCAL_RSEQ: Rseq = Rseq::new();

#[cfg(feature = "nightly")]
#[thread_local]
static mut THREAD_INITIALIZED: bool = false;

/// Global flag: has the kernel rejected rseq? (ENOSYS → kernel too old.)
static RSEQ_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

// ── Initialization ───────────────────────────────────────────────────────────

/// Possible rseq ownership modes after initialization.
#[cfg(feature = "nightly")]
enum RseqOwner {
    /// glibc owns the rseq area — we just use it.
    Glibc(*mut Rseq),
    /// We own the rseq area and registered it ourselves.
    SelfManaged(*mut Rseq),
    /// rseq is not available (kernel too old or registration failed).
    Unavailable,
}

/// Initialize rseq for the current thread, returning the active pointer.
///
/// This is idempotent — subsequent calls on the same thread return the
/// cached result without re-registering.
///
/// # Safety
///
/// Must be called from a thread context (i.e., not during static init).
#[cfg(feature = "nightly")]
unsafe fn init_thread_rseq() -> RseqOwner {
    unsafe {
        // Fast path: already initialized this thread.
        if THREAD_INITIALIZED {
            if glibc_rseq_registered() {
                return RseqOwner::Glibc(glibc_rseq_ptr());
            } else {
                let ptr = &raw mut LOCAL_RSEQ;
                if (*ptr).cpu_id != RSEQ_CPU_ID_REGISTRATION_FAILED {
                    return RseqOwner::SelfManaged(ptr);
                } else {
                    return RseqOwner::Unavailable;
                }
            }
        }

        // Check global "give up" flag.
        if RSEQ_UNAVAILABLE.load(Ordering::Relaxed) {
            THREAD_INITIALIZED = true;
            return RseqOwner::Unavailable;
        }

        // Try glibc first.
        if glibc_rseq_registered() {
            THREAD_INITIALIZED = true;
            return RseqOwner::Glibc(glibc_rseq_ptr());
        }

        // Self-register.
        let ptr = &raw mut LOCAL_RSEQ;
        match crate::syscall::rseq_register(ptr) {
            Ok(()) => {
                THREAD_INITIALIZED = true;
                RseqOwner::SelfManaged(ptr)
            }
            Err(e) => {
                if e == crate::syscall::ENOSYS {
                    // Kernel doesn't support rseq at all.
                    RSEQ_UNAVAILABLE.store(true, Ordering::Relaxed);
                }
                (*ptr).cpu_id = RSEQ_CPU_ID_REGISTRATION_FAILED;
                THREAD_INITIALIZED = true;
                RseqOwner::Unavailable
            }
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns `true` if rseq is available on this system.
///
/// After the first call to any rseq function on any thread, this reflects
/// whether the kernel accepted registration. Before that, it optimistically
/// returns `true`.
pub fn rseq_available() -> bool {
    !RSEQ_UNAVAILABLE.load(Ordering::Relaxed)
}

/// Get a pointer to the current thread's rseq area.
///
/// Returns `None` if rseq is unavailable (kernel too old, registration
/// failed, or the `nightly` feature is not enabled).
///
/// # Safety
///
/// The returned pointer is only valid on the calling thread and must not
/// be sent to other threads.
pub unsafe fn current_rseq() -> Option<*mut Rseq> {
    #[cfg(feature = "nightly")]
    {
        match unsafe { init_thread_rseq() } {
            RseqOwner::Glibc(ptr) | RseqOwner::SelfManaged(ptr) => Some(ptr),
            RseqOwner::Unavailable => None,
        }
    }
    #[cfg(not(feature = "nightly"))]
    {
        None
    }
}

/// Read the current CPU number from this thread's rseq area.
///
/// Returns `None` if rseq is unavailable.
pub fn current_cpu() -> Option<u32> {
    unsafe {
        let rseq = current_rseq()?;
        let cpu = core::ptr::read_volatile(&(*rseq).cpu_id);
        if cpu == RSEQ_CPU_ID_UNINITIALIZED || cpu == RSEQ_CPU_ID_REGISTRATION_FAILED {
            None
        } else {
            Some(cpu)
        }
    }
}

/// Read the current NUMA node ID from this thread's rseq area.
///
/// Returns `None` if rseq is unavailable. Note that `node_id` requires
/// kernel >= 5.17 and a sufficiently large `rseq_len` at registration.
pub fn current_numa_node() -> Option<u32> {
    unsafe {
        let rseq = current_rseq()?;
        Some(core::ptr::read_volatile(&(*rseq).node_id))
    }
}

/// Read the memory-map concurrency ID from this thread's rseq area.
///
/// Returns `None` if rseq is unavailable. Requires kernel >= 5.17.
pub fn current_mm_cid() -> Option<u32> {
    unsafe {
        let rseq = current_rseq()?;
        Some(core::ptr::read_volatile(&(*rseq).mm_cid))
    }
}

// ── RseqLocal — thread_local!-compatible handle ──────────────────────────────

/// Per-thread rseq handle with cached pointer.
///
/// Designed to be used as a thread-local static. The rseq pointer is
/// lazily resolved on first access and cached for subsequent calls
/// (single null check on the fast path).
///
/// # Usage
///
/// With `#[thread_local]` (nightly):
/// ```ignore
/// #[thread_local]
/// static RSEQ: RseqLocal = RseqLocal::new();
/// ```
///
/// With `std::thread_local!` (stable):
/// ```ignore
/// std::thread_local! {
///     static RSEQ: rseq::RseqLocal = rseq::RseqLocal::new();
/// }
/// RSEQ.with(|r| r.cpu_id());
/// ```
pub struct RseqLocal {
    /// Cached rseq pointer. Null means not yet initialized.
    ptr: core::cell::Cell<*mut Rseq>,
}

// Safety: RseqLocal is only accessed from its owning thread (thread-local).
// The pointer it caches is specific to the current thread's rseq area.
unsafe impl Sync for RseqLocal {}

impl Default for RseqLocal {
    fn default() -> Self {
        Self::new()
    }
}

impl RseqLocal {
    /// Create an uninitialized handle. Cheap — no syscalls until first use.
    pub const fn new() -> Self {
        Self {
            ptr: core::cell::Cell::new(core::ptr::null_mut()),
        }
    }

    /// Get the rseq pointer, lazily initializing on first call.
    ///
    /// Fast path after init: single null-pointer check.
    #[inline(always)]
    fn get_ptr(&self) -> Option<*mut Rseq> {
        let p = self.ptr.get();
        if !p.is_null() {
            return Some(p);
        }
        self.init_slow()
    }

    #[cold]
    fn init_slow(&self) -> Option<*mut Rseq> {
        let p = unsafe { current_rseq()? };
        self.ptr.set(p);
        Some(p)
    }

    /// Get the cached rseq pointer without a null check.
    ///
    /// # Safety
    ///
    /// Caller must ensure [`get_ptr`] or [`rseq_ptr`] has been called at
    /// least once on this thread (i.e., the pointer is already cached).
    #[inline(always)]
    pub unsafe fn get_ptr_unchecked(&self) -> *mut Rseq {
        self.ptr.get()
    }

    /// Get a raw pointer to this thread's rseq area.
    ///
    /// Returns `None` if rseq is unavailable.
    #[inline(always)]
    pub fn rseq_ptr(&self) -> Option<*mut Rseq> {
        self.get_ptr()
    }

    /// Read the current CPU number.
    ///
    /// Returns `None` if rseq is unavailable.
    #[inline(always)]
    pub fn cpu_id(&self) -> Option<u32> {
        let p = self.get_ptr()?;
        let cpu = unsafe { core::ptr::read_volatile(&(*p).cpu_id) };
        if cpu == RSEQ_CPU_ID_UNINITIALIZED || cpu == RSEQ_CPU_ID_REGISTRATION_FAILED {
            None
        } else {
            Some(cpu)
        }
    }

    /// Read the current NUMA node ID. Requires kernel >= 5.17.
    #[inline(always)]
    pub fn numa_node(&self) -> Option<u32> {
        let p = self.get_ptr()?;
        Some(unsafe { core::ptr::read_volatile(&(*p).node_id) })
    }

    /// Read the memory-map concurrency ID. Requires kernel >= 5.17.
    #[inline(always)]
    pub fn mm_cid(&self) -> Option<u32> {
        let p = self.get_ptr()?;
        Some(unsafe { core::ptr::read_volatile(&(*p).mm_cid) })
    }
}

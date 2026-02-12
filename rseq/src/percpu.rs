//! Per-CPU slab: tcmalloc-style per-CPU LIFO caches via rseq.
//!
//! A single contiguous memory region is divided among CPUs. Each CPU
//! gets `2^shift` bytes containing:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ Header[0]  (4 bytes: current u16 | end u16)         │
//! │ Header[1]                                           │
//! │ ...                                                 │
//! │ Header[NUM_CLASSES-1]                               │
//! │ (padding to 8-byte alignment)                       │
//! │ Slot array for class 1: [*mut u8; capacity[1]]      │
//! │ Slot array for class 2: [*mut u8; capacity[2]]      │
//! │ ...                                                 │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! Push and pop are lock-free via rseq critical sections. The only
//! commit operation is a single 16-bit store to `current`.
//!
//! Modelled after Google tcmalloc's `TcmallocSlab` in `percpu_tcmalloc.h`.

use core::arch::asm;
use core::ptr;

use crate::abi::Rseq;

// ── Header layout ────────────────────────────────────────────────────────────

/// Byte offset of `cpu_id` within `struct Rseq`.
const RSEQ_CPU_ID_OFF: u32 = 4;

/// Byte offset of `rseq_cs` within `struct Rseq`.
const RSEQ_CS_OFF: u32 = 8;

/// Per-size-class header within a CPU region.
///
/// Stored as two adjacent `u16` values at `base + class * 4`:
/// - offset 0: `current` — end of the occupied portion of the LIFO stack.
///   Range `[begin..current)` contains valid pointers.
///   `current == begin` means empty, `current == end` means full.
/// - offset 2: `end` — one past the last slot (capacity limit).
///
/// The rseq commit is a single 16-bit store to `current`.
#[repr(C)]
pub struct SlabHeader {
    pub current: u16,
    pub end: u16,
}

// ── PerCpuSlab ───────────────────────────────────────────────────────────────

/// Per-CPU slab allocator with LIFO stacks per size class.
///
/// `NUM_CLASSES` is the total number of size classes (including class 0
/// which is unused). Must match the allocator's size class table.
///
/// The slab does **not** own the backing memory — the caller is
/// responsible for allocating (e.g., via `mmap`) and freeing it.
pub struct PerCpuSlab<const NUM_CLASSES: usize> {
    /// Base pointer to the mmap'd region.
    slabs: *mut u8,
    /// Log2 of per-CPU region size in bytes.
    shift: u32,
    /// Number of CPUs this slab was initialized for.
    num_cpus: u32,
    /// Per-size-class begin offsets in pointer-sized units (8 bytes).
    /// Shared layout across all CPUs.
    begins: [u16; NUM_CLASSES],
}

// Safety: the slab is a shared data structure accessed by multiple threads,
// each touching only their current CPU's region (enforced by rseq).
unsafe impl<const N: usize> Sync for PerCpuSlab<N> {}
unsafe impl<const N: usize> Send for PerCpuSlab<N> {}

impl<const NUM_CLASSES: usize> PerCpuSlab<NUM_CLASSES> {
    /// Create an uninitialized slab. Must call [`init`] before use.
    pub const fn empty() -> Self {
        Self {
            slabs: ptr::null_mut(),
            shift: 0,
            num_cpus: 0,
            begins: [0u16; NUM_CLASSES],
        }
    }

    /// Initialize the slab over a caller-provided memory region.
    ///
    /// - `region`: base pointer, must be at least `num_cpus << shift` bytes.
    ///   Should be page-aligned (e.g., from `mmap`).
    /// - `num_cpus`: number of CPUs to provision.
    /// - `shift`: log2 of per-CPU region size. Each CPU gets `2^shift` bytes.
    ///   Typical values: 12 (4 KiB) to 18 (256 KiB).
    /// - `capacities`: max number of cached pointers per size class.
    ///   `capacities[0]` is ignored (class 0 is unused).
    ///
    /// Returns `false` if the per-CPU layout exceeds `2^shift` bytes.
    ///
    /// # Safety
    ///
    /// - `region` must point to valid, writable memory of at least
    ///   `num_cpus << shift` bytes.
    /// - The memory must remain valid for the lifetime of the slab.
    #[allow(clippy::needless_range_loop)]
    pub unsafe fn init(
        &mut self,
        region: *mut u8,
        num_cpus: u32,
        shift: u32,
        capacities: &[u16; NUM_CLASSES],
    ) -> bool {
        // Compute begin offsets.
        // Headers occupy the first NUM_CLASSES * 4 bytes, then align to 8.
        let header_bytes = NUM_CLASSES * 4;
        let data_start = (header_bytes + 7) & !7; // align to 8 bytes
        let mut offset = data_start / 8; // convert to pointer-sized units

        self.begins[0] = 0;
        for class in 1..NUM_CLASSES {
            self.begins[class] = offset as u16;
            offset += capacities[class] as usize;
        }

        // Check that the per-CPU layout fits.
        let per_cpu_bytes = offset * 8;
        if per_cpu_bytes > (1usize << shift) {
            return false;
        }

        // Write initial headers for each CPU: all classes empty.
        unsafe {
            for cpu in 0..num_cpus {
                let base = region.add((cpu as usize) << shift);
                for class in 0..NUM_CLASSES {
                    let hdr = base.add(class * 4) as *mut SlabHeader;
                    (*hdr).current = self.begins[class];
                    (*hdr).end = self.begins[class] + capacities[class];
                }
            }
        }

        self.slabs = region;
        self.shift = shift;
        self.num_cpus = num_cpus;
        true
    }

    /// Whether the slab has been initialized.
    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        !self.slabs.is_null()
    }

    /// Begin offset for a size class (in pointer-sized units).
    #[inline(always)]
    pub fn begin(&self, class: usize) -> u16 {
        self.begins[class]
    }

    /// Base pointer to the slab region.
    #[inline(always)]
    pub fn slabs_ptr(&self) -> *mut u8 {
        self.slabs
    }

    /// Shift value (log2 of per-CPU region size).
    #[inline(always)]
    pub fn shift(&self) -> u32 {
        self.shift
    }

    /// Number of cached objects for `class` on `cpu`.
    pub fn length(&self, cpu: u32, class: usize) -> u16 {
        unsafe {
            let base = self.slabs.add((cpu as usize) << self.shift);
            let hdr = &*(base.add(class * 4) as *const SlabHeader);
            hdr.current - self.begins[class]
        }
    }

    /// Capacity (max objects) for `class`.
    pub fn capacity(&self, cpu: u32, class: usize) -> u16 {
        unsafe {
            let base = self.slabs.add((cpu as usize) << self.shift);
            let hdr = &*(base.add(class * 4) as *const SlabHeader);
            hdr.end - self.begins[class]
        }
    }

    // ── Push / Pop via rseq ──────────────────────────────────────────

    /// Pop a pointer from `class` on the current CPU.
    ///
    /// Returns `Some(ptr)` on success, `None` if the class is empty or
    /// the rseq critical section was aborted (caller should retry).
    ///
    /// # Safety
    ///
    /// - `rseq` must be a valid, registered rseq pointer for the current thread.
    /// - `class` must be `< NUM_CLASSES` and have been initialized.
    #[inline(never)]
    pub unsafe fn pop(&self, rseq: *mut Rseq, class: usize) -> Option<*mut u8> {
        let class_off = (class * 4) as u64;
        let begin = self.begins[class] as u64;
        let slabs = self.slabs as u64;
        let shift = self.shift;

        let result: u64;
        let success: u64;

        unsafe {
            asm!(
                // rseq_cs descriptor in a relocatable data section.
                ".pushsection __rseq_cs, \"aw\"",
                ".balign 32",
                "77:",
                ".long 0",                     // version
                ".long 0",                     // flags
                ".quad 3f",                    // start_ip
                ".quad (4f - 3f)",             // post_commit_offset
                ".quad 6f",                    // abort_ip
                ".popsection",

                "lea {tmp}, [rip + 77b]",
                "mov qword ptr [{rseq} + {rseq_cs_off}], {tmp}",

                // ── start of critical section ────────────────────────
                "3:",

                // Read cpu_id, compute region base = slabs + (cpu << shift)
                "mov {base:e}, dword ptr [{rseq} + {cpu_id_off}]",
                "shl {base}, cl",
                "add {base}, {slabs}",

                // Load current (16-bit) from header
                "movzx {cur:e}, word ptr [{base} + {class_off}]",

                // Empty check: current == begin
                "cmp {cur}, {begin}",
                "je 7f",

                // new_current = current - 1
                "dec {cur:e}",

                // Load pointer from slot[new_current]
                "mov {result}, qword ptr [{base} + {cur} * 8]",

                // COMMIT: store new current (16-bit write)
                "mov word ptr [{base} + {class_off}], {cur:x}",
                "4:",

                // ── post-commit cleanup ──────────────────────────────
                "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
                "mov {succ}, 1",
                "jmp 5f",

                // ── empty: class has no objects ──────────────────────
                "7:",
                "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
                "xor {succ:e}, {succ:e}",
                "jmp 5f",

                // ── abort handler ────────────────────────────────────
                ".long 0x53053053",
                "6:",
                "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
                "xor {succ:e}, {succ:e}",

                "5:",

                rseq = in(reg) rseq,
                slabs = in(reg) slabs,
                in("rcx") shift as u64,
                class_off = in(reg) class_off,
                begin = in(reg) begin,
                base = out(reg) _,
                cur = out(reg) _,
                result = out(reg) result,
                succ = out(reg) success,
                tmp = out(reg) _,
                rseq_cs_off = const RSEQ_CS_OFF,
                cpu_id_off = const RSEQ_CPU_ID_OFF,
                options(nostack),
            );
        }

        if success != 0 {
            Some(result as *mut u8)
        } else {
            None
        }
    }

    /// Push a pointer to `class` on the current CPU.
    ///
    /// Returns `Some(())` on success, `None` if the class is full or
    /// the rseq critical section was aborted (caller should retry).
    ///
    /// # Safety
    ///
    /// - `rseq` must be a valid, registered rseq pointer for the current thread.
    /// - `class` must be `< NUM_CLASSES` and have been initialized.
    /// - `ptr` must be a valid pointer that was previously allocated.
    #[inline(never)]
    pub unsafe fn push(&self, rseq: *mut Rseq, class: usize, ptr: *mut u8) -> Option<()> {
        let class_off = (class * 4) as u64;
        let slabs = self.slabs as u64;
        let shift = self.shift;

        let success: u64;

        unsafe {
            asm!(
                // rseq_cs descriptor in a relocatable data section.
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

                // ── start of critical section ────────────────────────
                "3:",

                // Read cpu_id, compute region base
                "mov {base:e}, dword ptr [{rseq} + {cpu_id_off}]",
                "shl {base}, cl",
                "add {base}, {slabs}",

                // Load full header (current | end << 16)
                "mov {hdr:e}, dword ptr [{base} + {class_off}]",

                // Extract end (high 16 bits) into tmp
                "mov {end_:e}, {hdr:e}",
                "shr {end_:e}, 16",

                // Extract current (low 16 bits)
                "movzx {hdr:e}, {hdr:x}",

                // Full check: current == end
                "cmp {hdr:e}, {end_:e}",
                "je 7f",

                // Store pointer at slot[current]
                "mov qword ptr [{base} + {hdr} * 8], {ptr}",

                // COMMIT: store current + 1 (16-bit write)
                "inc {hdr:e}",
                "mov word ptr [{base} + {class_off}], {hdr:x}",
                "4:",

                // ── post-commit cleanup ──────────────────────────────
                "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
                "mov {succ}, 1",
                "jmp 5f",

                // ── full: class has no room ──────────────────────────
                "7:",
                "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
                "xor {succ:e}, {succ:e}",
                "jmp 5f",

                // ── abort handler ────────────────────────────────────
                ".long 0x53053053",
                "6:",
                "mov qword ptr [{rseq} + {rseq_cs_off}], 0",
                "xor {succ:e}, {succ:e}",

                "5:",

                rseq = in(reg) rseq,
                slabs = in(reg) slabs,
                in("rcx") shift as u64,
                class_off = in(reg) class_off,
                ptr = in(reg) ptr,
                base = out(reg) _,
                hdr = out(reg) _,
                end_ = out(reg) _,
                succ = out(reg) success,
                tmp = out(reg) _,
                rseq_cs_off = const RSEQ_CS_OFF,
                cpu_id_off = const RSEQ_CPU_ID_OFF,
                options(nostack),
            );
        }

        if success != 0 { Some(()) } else { None }
    }

    // ── Batch operations (non-rseq, caller holds CPU affinity) ───────

    /// Pop up to `count` pointers from `class` on a specific `cpu`.
    ///
    /// Returns the number of pointers written to `out`.
    ///
    /// # Safety
    ///
    /// Caller must ensure exclusive access to this CPU's slab region
    /// (e.g., by disabling preemption or during single-threaded init).
    pub unsafe fn pop_batch(
        &self,
        cpu: u32,
        class: usize,
        out: *mut *mut u8,
        count: usize,
    ) -> usize {
        unsafe {
            let base = self.slabs.add((cpu as usize) << self.shift);
            let hdr = &mut *(base.add(class * 4) as *mut SlabHeader);
            let begin = self.begins[class];

            let avail = (hdr.current - begin) as usize;
            let n = count.min(avail);

            for i in 0..n {
                hdr.current -= 1;
                let slot = base.add(hdr.current as usize * 8) as *const *mut u8;
                out.add(i).write(slot.read());
            }

            n
        }
    }

    /// Push up to `count` pointers to `class` on a specific `cpu`.
    ///
    /// Returns the number of pointers actually pushed.
    ///
    /// # Safety
    ///
    /// Same requirements as [`pop_batch`].
    pub unsafe fn push_batch(
        &self,
        cpu: u32,
        class: usize,
        ptrs: *const *mut u8,
        count: usize,
    ) -> usize {
        unsafe {
            let base = self.slabs.add((cpu as usize) << self.shift);
            let hdr = &mut *(base.add(class * 4) as *mut SlabHeader);

            let room = (hdr.end - hdr.current) as usize;
            let n = count.min(room);

            for i in 0..n {
                let slot = base.add(hdr.current as usize * 8) as *mut *mut u8;
                slot.write(ptrs.add(i).read());
                hdr.current += 1;
            }

            n
        }
    }
}

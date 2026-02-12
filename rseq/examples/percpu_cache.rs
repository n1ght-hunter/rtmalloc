//! Example: per-CPU allocator cache using rseq.
//!
//! Demonstrates how tcmalloc uses PerCpuSlab as a fast-path cache in front
//! of a slower central freelist. This is the core pattern behind per-CPU
//! allocators:
//!
//! ```text
//!   alloc()                          free(ptr)
//!     |                                |
//!     v                                v
//!  [per-CPU slab: pop]           [per-CPU slab: push]
//!     |                                |
//!     | empty?                         | full?
//!     v                                v
//!  [central freelist: lock + grab batch]  [central freelist: lock + drain batch]
//!     |
//!     | empty?
//!     v
//!  [system allocator: allocate new blocks]
//! ```
//!
//! The slab is just a LIFO stack of pointers per CPU per size class.
//! alloc = pop, free = push. No locks, no atomics on the fast path.
//!
//! Run with:
//!   cargo run -p rseq --features nightly --example percpu_cache
//!
//! (Must run on Linux x86_64 with kernel >= 4.18)

use std::alloc::{self, Layout};
use std::sync::Mutex;

use rseq::{PerCpuSlab, RseqLocal};

thread_local! {
    static RSEQ: RseqLocal = const { RseqLocal::new() };
}

/// We model 3 size classes (index 0 is unused by convention):
///   class 1: 64-byte blocks
///   class 2: 128-byte blocks
///   class 3: 256-byte blocks
const NUM_CLASSES: usize = 4;
const CLASS_SIZES: [usize; NUM_CLASSES] = [0, 64, 128, 256];

/// How many pointers each CPU can cache per class.
const SLAB_CAPACITY: u16 = 16;

/// When the slab is empty, fetch this many blocks from central at once.
const BATCH_SIZE: usize = 8;

/// Per-CPU region: 2^12 = 4 KiB (plenty for this demo).
const SHIFT: u32 = 12;

/// A simple Mutex-protected freelist per size class.
/// In a real allocator this would be a more sophisticated structure
/// (e.g., tcmalloc's CentralFreeList with span management).
struct CentralFreeList {
    lists: [Mutex<Vec<*mut u8>>; NUM_CLASSES],
}

// Safety: the pointers in the lists came from the global allocator
// and are not aliased — they're free blocks waiting to be handed out.
unsafe impl Sync for CentralFreeList {}

impl CentralFreeList {
    fn new() -> Self {
        Self {
            lists: std::array::from_fn(|_| Mutex::new(Vec::new())),
        }
    }

    /// Grab up to `count` blocks from central. If central is empty,
    /// allocate fresh blocks from the system allocator.
    fn pop_batch(&self, class: usize, out: &mut Vec<*mut u8>, count: usize) {
        let mut list = self.lists[class].lock().unwrap();

        // Take what central has.
        let from_central = count.min(list.len());
        for _ in 0..from_central {
            out.push(list.pop().unwrap());
        }

        // If central didn't have enough, allocate new blocks.
        let remaining = count - from_central;
        if remaining > 0 {
            let size = CLASS_SIZES[class];
            let layout = Layout::from_size_align(size, 8).unwrap();
            for _ in 0..remaining {
                let ptr = unsafe { alloc::alloc(layout) };
                assert!(!ptr.is_null(), "allocation failed");
                out.push(ptr);
            }
        }
    }

    /// Return a batch of blocks back to central.
    fn push_batch(&self, class: usize, ptrs: &[*mut u8]) {
        let mut list = self.lists[class].lock().unwrap();
        list.extend_from_slice(ptrs);
    }

    /// Free all remaining blocks back to the system.
    #[allow(dead_code, clippy::needless_range_loop)]
    fn cleanup(&self) {
        for class in 1..NUM_CLASSES {
            let mut list = self.lists[class].lock().unwrap();
            let layout = Layout::from_size_align(CLASS_SIZES[class], 8).unwrap();
            for ptr in list.drain(..) {
                unsafe { alloc::dealloc(ptr, layout) };
            }
        }
    }
}

struct PerCpuAllocator {
    slab: PerCpuSlab<NUM_CLASSES>,
    central: CentralFreeList,
}

// Safety: slab is accessed via rseq (per-CPU), central is Mutex-protected.
unsafe impl Sync for PerCpuAllocator {}

impl PerCpuAllocator {
    /// Allocate a block from `class`.
    ///
    /// Fast path: pop from the per-CPU slab (no locks, no atomics).
    /// Slow path: refill from central freelist, then retry.
    fn alloc(&self, class: usize) -> *mut u8 {
        RSEQ.with(|r| {
            let rseq_ptr = r.rseq_ptr().expect("rseq available");

            // Fast path: try popping from the per-CPU cache.
            loop {
                match unsafe { self.slab.pop(rseq_ptr, class) } {
                    Some(ptr) => return ptr,
                    None => {
                        // Could be empty or rseq abort. Check if truly empty
                        // by trying once more — if it's an abort, the retry
                        // will succeed. If it's truly empty, we refill.
                        if let Some(ptr) = unsafe { self.slab.pop(rseq_ptr, class) } {
                            return ptr;
                        }
                        // Slow path: refill the slab from central.
                        self.refill(rseq_ptr, class);
                    }
                }
            }
        })
    }

    /// Free a block back to `class`.
    ///
    /// Fast path: push onto the per-CPU slab.
    /// Slow path: drain excess to central freelist, then retry.
    fn free(&self, class: usize, ptr: *mut u8) {
        RSEQ.with(|r| {
            let rseq_ptr = r.rseq_ptr().expect("rseq available");

            loop {
                match unsafe { self.slab.push(rseq_ptr, class, ptr) } {
                    Some(()) => return,
                    None => {
                        // Retry once for rseq abort.
                        if unsafe { self.slab.push(rseq_ptr, class, ptr) }.is_some() {
                            return;
                        }
                        // Slow path: slab is full, drain some to central.
                        self.drain(rseq_ptr, class);
                    }
                }
            }
        })
    }

    /// Slow path: grab a batch from central and push into the slab.
    fn refill(&self, rseq_ptr: *mut rseq::Rseq, class: usize) {
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        self.central.pop_batch(class, &mut batch, BATCH_SIZE);

        for ptr in batch {
            // Push each into the slab. If push fails (rseq abort), just retry.
            loop {
                if unsafe { self.slab.push(rseq_ptr, class, ptr) }.is_some() {
                    break;
                }
            }
        }
    }

    /// Slow path: pop a batch from the slab and return to central.
    fn drain(&self, rseq_ptr: *mut rseq::Rseq, class: usize) {
        let mut batch = Vec::with_capacity(BATCH_SIZE);

        for _ in 0..BATCH_SIZE {
            // Try twice: first attempt may fail due to rseq abort, second confirms empty.
            let ptr = match unsafe { self.slab.pop(rseq_ptr, class) } {
                Some(p) => Some(p),
                None => unsafe { self.slab.pop(rseq_ptr, class) },
            };
            match ptr {
                Some(p) => batch.push(p),
                None => break,
            }
        }

        if !batch.is_empty() {
            self.central.push_batch(class, &batch);
        }
    }
}

fn main() {
    println!("Per-CPU cache allocator example");
    println!("===============================\n");

    // Check rseq is available.
    let cpu = RSEQ.with(|r| r.cpu_id());
    if cpu.is_none() {
        println!("rseq unavailable (need Linux x86_64, kernel >= 4.18).");
        return;
    }
    println!("rseq active, cpu_id = {}", cpu.unwrap());

    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    println!("{num_cpus} CPUs detected\n");

    // Set up the per-CPU slab.
    let region_size = (num_cpus as usize) << SHIFT;
    let mut region = vec![0u8; region_size];
    let capacities: [u16; NUM_CLASSES] = [0, SLAB_CAPACITY, SLAB_CAPACITY, SLAB_CAPACITY];

    let mut slab = PerCpuSlab::<NUM_CLASSES>::empty();
    let ok = unsafe { slab.init(region.as_mut_ptr(), num_cpus, SHIFT, &capacities) };
    assert!(ok, "slab layout doesn't fit");

    let allocator = PerCpuAllocator {
        slab,
        central: CentralFreeList::new(),
    };

    println!("--- Single-threaded alloc/free (class 1 = 64 bytes) ---\n");

    // Allocate 5 blocks.
    let mut ptrs: Vec<*mut u8> = Vec::new();
    for i in 0..5 {
        let ptr = allocator.alloc(1);
        // Write a tag so we can verify the block is ours.
        unsafe { *(ptr as *mut u64) = 0xCAFE_0000 + i };
        println!("  alloc[{i}] = {ptr:p}");
        ptrs.push(ptr);
    }

    // Free them all.
    println!();
    for (i, ptr) in ptrs.iter().enumerate() {
        let tag = unsafe { *(*ptr as *const u64) };
        println!("  free[{i}]  = {:p}  (tag: {tag:#x})", *ptr);
        allocator.free(1, *ptr);
    }
    ptrs.clear();

    // Allocate again — these should come from the slab (recycled), not
    // the system allocator. You'll see the same addresses reused.
    println!("\n  Re-allocating (should reuse freed blocks):");
    for i in 0..5 {
        let ptr = allocator.alloc(1);
        println!("  alloc[{i}] = {ptr:p}");
        allocator.free(1, ptr);
    }

    println!("\n--- Multi-threaded alloc/free (4 threads x 100 ops) ---\n");

    // Leak the allocator into a &'static so threads can share it.
    let allocator: &'static PerCpuAllocator = Box::leak(Box::new(allocator));
    // Keep the region alive — also leak it.
    let _region = std::mem::ManuallyDrop::new(region);

    let handles: Vec<_> = (0..4)
        .map(|tid| {
            std::thread::spawn(move || {
                let mut local_ptrs: Vec<*mut u8> = Vec::new();

                for i in 0..100 {
                    // Allocate a 64-byte block.
                    let ptr = allocator.alloc(1);

                    // Write thread_id + iteration as a canary.
                    let canary = ((tid as u64) << 32) | (i as u64);
                    unsafe { *(ptr as *mut u64) = canary };
                    local_ptrs.push(ptr);

                    // Every 10 allocs, free the batch.
                    if local_ptrs.len() == 10 {
                        for &p in &local_ptrs {
                            // Verify canary is intact (no corruption from other CPUs).
                            let val = unsafe { *(p as *const u64) };
                            assert_eq!(
                                val >> 32,
                                tid as u64,
                                "corruption detected! expected tid={tid}, got {}",
                                val >> 32
                            );
                            allocator.free(1, p);
                        }
                        local_ptrs.clear();
                    }
                }

                // Free remaining.
                for &p in &local_ptrs {
                    allocator.free(1, p);
                }

                let cpu = RSEQ.with(|r| r.cpu_id().unwrap_or(0));
                println!("  thread {tid} done (last cpu = {cpu})");
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("\nAll canary checks passed — no cross-CPU corruption.");
    println!("\nDone.");
}

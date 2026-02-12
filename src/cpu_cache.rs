//! Per-CPU cache: rseq-based per-CPU LIFO caches replacing thread caches.
//!
//! Uses `rseq::PerCpuSlab` for lock-free, atomic-free push/pop on the fast path.
//! When the slab is empty (alloc) or full (free), batches transfer through the
//! existing TransferCache → CentralFreeList → PageHeap hierarchy.
//!
//! This module is only compiled when `feature = "percpu"` is active.

use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use rseq::{PerCpuSlab, RseqLocal};

use crate::central_free_list::CentralCache;
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class::{self, NUM_SIZE_CLASSES};
use crate::span::FreeObject;
use crate::sync::SpinMutex;
use crate::transfer_cache::TransferCacheArray;

/// Wrapper so we can put PerCpuSlab in a static (it's Sync by rseq design).
struct SlabCell(UnsafeCell<PerCpuSlab<NUM_SIZE_CLASSES>>);
unsafe impl Sync for SlabCell {}

impl SlabCell {
    const fn new() -> Self {
        Self(UnsafeCell::new(PerCpuSlab::empty()))
    }

    /// Get a shared reference. Safe after initialization.
    #[inline(always)]
    fn get(&self) -> &PerCpuSlab<NUM_SIZE_CLASSES> {
        unsafe { &*self.0.get() }
    }

    /// Get a mutable reference. Only call during init (under lock).
    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut(&self) -> &mut PerCpuSlab<NUM_SIZE_CLASSES> {
        unsafe { &mut *self.0.get() }
    }
}

/// Log2 of per-CPU region size: 2^18 = 256 KiB per CPU.
/// 46 classes × 32 slots × 8 bytes = ~11 KiB, well within 256 KiB.
const SHIFT: u32 = 18;

/// `_SC_NPROCESSORS_CONF` on Linux x86_64.
const _SC_NPROCESSORS_CONF: i32 = 83;

unsafe extern "C" {
    fn sysconf(name: i32) -> isize;
}

/// The per-CPU slab. Starts uninitialized (null slabs pointer).
/// After `ensure_init()`, all CPUs have valid headers.
static CPU_SLAB: SlabCell = SlabCell::new();

/// Backing memory pointer. Null = not yet allocated.
/// Non-null = init complete (used as the fast-path check).
static SLAB_REGION: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

/// Protects one-time initialization.
static INIT_LOCK: SpinMutex<()> = SpinMutex::new(());

#[thread_local]
static RSEQ: RseqLocal = RseqLocal::new();

/// Ensure the per-CPU slab is initialized. After the first call, this is
/// just a single atomic load (fast path).
#[inline(always)]
fn ensure_init() {
    if SLAB_REGION.load(Ordering::Acquire).is_null() {
        init_slow();
    }
}

#[cold]
#[inline(never)]
fn init_slow() {
    let _guard = INIT_LOCK.lock();

    // Double-check after acquiring lock.
    if !SLAB_REGION.load(Ordering::Relaxed).is_null() {
        return;
    }

    let num_cpus = unsafe { sysconf(_SC_NPROCESSORS_CONF) };
    let num_cpus = if num_cpus <= 0 { 1 } else { num_cpus as u32 };

    // Allocate backing memory.
    let region_size = (num_cpus as usize) << SHIFT;
    let region = unsafe { crate::platform::page_alloc(region_size) };
    if region.is_null() {
        // Can't allocate — fall through to transfer cache on every call.
        return;
    }

    // Build per-class capacities from batch_size.
    let mut capacities = [0u16; NUM_SIZE_CLASSES];
    for (class, cap) in capacities.iter_mut().enumerate().skip(1) {
        *cap = size_class::class_info(class).batch_size as u16;
    }

    let ok = unsafe {
        CPU_SLAB
            .get_mut()
            .init(region, num_cpus, SHIFT, &capacities)
    };
    if !ok {
        // Layout doesn't fit — shouldn't happen with shift=18.
        unsafe { crate::platform::page_dealloc(region, region_size) };
        return;
    }

    // Publish: all subsequent ensure_init() calls see non-null and skip.
    SLAB_REGION.store(region, Ordering::Release);
}

/// Allocate an object of the given size class via the per-CPU cache.
///
/// Fast path: rseq pop (no locks, no atomics).
/// Slow path: refill from transfer cache, then retry.
///
/// # Safety
///
/// - `class` must be a valid size class (1..NUM_SIZE_CLASSES).
/// - All static references must be valid (they are — module-level statics).
#[inline(always)]
pub unsafe fn alloc(
    class: usize,
    transfer_cache: &TransferCacheArray,
    central: &CentralCache,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) -> *mut u8 {
    ensure_init();

    // If slab init failed, go straight to central.
    if !CPU_SLAB.get().is_initialized() {
        return unsafe { alloc_from_central(class, transfer_cache, central, page_heap, pagemap) };
    }

    let rseq_ptr = match RSEQ.rseq_ptr() {
        Some(p) => p,
        None => {
            // rseq unavailable — fall through to central.
            return unsafe {
                alloc_from_central(class, transfer_cache, central, page_heap, pagemap)
            };
        }
    };

    // Fast path: try popping from the slab.
    unsafe {
        if let Some(ptr) = CPU_SLAB.get().pop(rseq_ptr, class) {
            return ptr;
        }
        // Could be rseq abort — retry once.
        if let Some(ptr) = CPU_SLAB.get().pop(rseq_ptr, class) {
            return ptr;
        }
    }

    // Slow path: slab is empty, refill and retry.
    unsafe {
        refill(class, rseq_ptr, transfer_cache, central, page_heap, pagemap);

        // After refill, try to pop. If still empty (OOM or migration),
        // fall back to central allocation.
        if let Some(ptr) = CPU_SLAB.get().pop(rseq_ptr, class) {
            return ptr;
        }
        if let Some(ptr) = CPU_SLAB.get().pop(rseq_ptr, class) {
            return ptr;
        }
        alloc_from_central(class, transfer_cache, central, page_heap, pagemap)
    }
}

/// Free an object back to the per-CPU cache.
///
/// Fast path: rseq push (no locks, no atomics).
/// Slow path: drain excess to transfer cache, then retry.
///
/// # Safety
///
/// - `ptr` must be a valid freed object of the given `class`.
/// - `class` must be a valid size class.
#[inline(always)]
pub unsafe fn dealloc(
    ptr: *mut u8,
    class: usize,
    transfer_cache: &TransferCacheArray,
    central: &CentralCache,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) {
    ensure_init();

    // If slab init failed, go straight to central.
    if !CPU_SLAB.get().is_initialized() {
        unsafe { dealloc_to_central(ptr, class, transfer_cache, central, page_heap, pagemap) };
        return;
    }

    let rseq_ptr = match RSEQ.rseq_ptr() {
        Some(p) => p,
        None => {
            // rseq unavailable — return directly to central.
            unsafe { dealloc_to_central(ptr, class, transfer_cache, central, page_heap, pagemap) };
            return;
        }
    };

    // Fast path: push onto the slab.
    unsafe {
        if CPU_SLAB.get().push(rseq_ptr, class, ptr).is_some() {
            return;
        }
        // Could be rseq abort — retry once.
        if CPU_SLAB.get().push(rseq_ptr, class, ptr).is_some() {
            return;
        }
    }

    // Slow path: slab is full, drain then retry.
    unsafe {
        drain(class, rseq_ptr, transfer_cache, central, page_heap, pagemap);

        // After drain, try to push. If still full (migration), fall back.
        if CPU_SLAB.get().push(rseq_ptr, class, ptr).is_some() {
            return;
        }
        if CPU_SLAB.get().push(rseq_ptr, class, ptr).is_some() {
            return;
        }
        dealloc_to_central(ptr, class, transfer_cache, central, page_heap, pagemap)
    }
}

/// Refill the per-CPU slab from the transfer cache / central free list.
///
/// Fetches a batch of objects and pushes them into the slab.
#[cold]
unsafe fn refill(
    class: usize,
    rseq_ptr: *mut rseq::Rseq,
    transfer_cache: &TransferCacheArray,
    central: &CentralCache,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) {
    let batch_size = size_class::class_info(class).batch_size;

    let (count, head) =
        unsafe { transfer_cache.remove_range(class, batch_size, central, page_heap, pagemap) };

    if count == 0 || head.is_null() {
        return;
    }

    // Walk the linked list and push each pointer into the slab.
    let mut node = head;
    for (pushed, _) in (0..count).enumerate() {
        if node.is_null() {
            break;
        }
        let next = unsafe { (*node).next };
        // Push into slab. Retry a few times for rseq aborts, then stop.
        let mut ok = false;
        for _ in 0..4 {
            if unsafe { CPU_SLAB.get().push(rseq_ptr, class, node as *mut u8) }.is_some() {
                ok = true;
                break;
            }
        }
        if !ok {
            // Slab full or persistent aborts — return remaining objects
            // to the transfer cache. Re-link the unpushed tail.
            unsafe { (*node).next = next };
            let remaining = count - pushed;
            // Find the tail of the remaining chain.
            let mut tail = node;
            let mut walk = next;
            while !walk.is_null() {
                tail = walk;
                walk = unsafe { (*walk).next };
            }
            unsafe {
                transfer_cache
                    .insert_range(class, node, tail, remaining, central, page_heap, pagemap)
            };
            return;
        }
        node = next;
    }
}

/// Drain excess objects from the per-CPU slab to the transfer cache.
///
/// Pops a batch of pointers and returns them as a linked FreeObject chain.
#[cold]
unsafe fn drain(
    class: usize,
    rseq_ptr: *mut rseq::Rseq,
    transfer_cache: &TransferCacheArray,
    central: &CentralCache,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) {
    let batch_size = size_class::class_info(class).batch_size;

    // Pop pointers from the slab into a linked list.
    let mut head: *mut FreeObject = ptr::null_mut();
    let mut tail: *mut FreeObject = ptr::null_mut();
    let mut count = 0usize;

    for _ in 0..batch_size {
        let ptr = match unsafe { CPU_SLAB.get().pop(rseq_ptr, class) } {
            Some(p) => Some(p),
            // Retry once for abort, then assume empty.
            None => unsafe { CPU_SLAB.get().pop(rseq_ptr, class) },
        };

        match ptr {
            Some(p) => {
                let obj = p as *mut FreeObject;
                unsafe { (*obj).next = head };
                if tail.is_null() {
                    tail = obj;
                }
                head = obj;
                count += 1;
            }
            None => break,
        }
    }

    if count > 0 && !head.is_null() {
        // Null-terminate the tail.
        unsafe { (*tail).next = ptr::null_mut() };
        unsafe {
            transfer_cache.insert_range(class, head, tail, count, central, page_heap, pagemap)
        };
    }
}

/// Allocate directly from the transfer/central cache (rseq not available).
#[cold]
unsafe fn alloc_from_central(
    class: usize,
    transfer_cache: &TransferCacheArray,
    central: &CentralCache,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) -> *mut u8 {
    let (count, head) =
        unsafe { transfer_cache.remove_range(class, 1, central, page_heap, pagemap) };
    if count == 0 || head.is_null() {
        ptr::null_mut()
    } else {
        head as *mut u8
    }
}

/// Free directly to the transfer/central cache (rseq not available).
#[cold]
unsafe fn dealloc_to_central(
    ptr: *mut u8,
    class: usize,
    transfer_cache: &TransferCacheArray,
    central: &CentralCache,
    page_heap: &SpinMutex<PageHeap>,
    pagemap: &PageMap,
) {
    let obj = ptr as *mut FreeObject;
    unsafe { (*obj).next = ptr::null_mut() };
    unsafe { transfer_cache.insert_range(class, obj, obj, 1, central, page_heap, pagemap) };
}

//! Example: rseq per-CPU ID reading and per-CPU slab push/pop.
//!
//! Run with:
//!   cargo run -p rseq --features nightly --example cpu_id

use rseq::{PerCpuSlab, RseqLocal};

thread_local! {
    static RSEQ: RseqLocal = const { RseqLocal::new() };
}

/// Number of size classes for the demo slab.
const NUM_CLASSES: usize = 4;

/// Per-CPU region size: 2^12 = 4 KiB (small, sufficient for demo).
const SHIFT: u32 = 12;

fn main() {
    println!("rseq example");
    println!("============\n");

    let cpu = RSEQ.with(|r| r.cpu_id());
    match cpu {
        Some(cpu) => println!("[main] cpu_id = {cpu}"),
        None => {
            println!("[main] rseq unavailable (kernel too old or not Linux x86_64).");
            return;
        }
    }

    let handles: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                RSEQ.with(|r| match r.cpu_id() {
                    Some(cpu) => println!("[thread {i}] cpu_id = {cpu}"),
                    None => println!("[thread {i}] rseq unavailable"),
                });
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Show cpu_id can change between reads (OS may migrate us).
    print!("\ncpu_id x10:");
    RSEQ.with(|r| {
        for _ in 0..10 {
            match r.cpu_id() {
                Some(cpu) => print!(" {cpu}"),
                None => print!(" ?"),
            }
        }
    });
    println!();

    println!("\nPerCpuSlab demo (tcmalloc-style per-CPU LIFO):");

    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);

    // Allocate backing memory (normally you'd use mmap).
    let region_size = (num_cpus as usize) << SHIFT;
    let mut region = vec![0u8; region_size];

    // Capacities per class: class 0 unused, classes 1-3 get 16 slots each.
    let capacities: [u16; NUM_CLASSES] = [0, 16, 16, 16];

    let mut slab = PerCpuSlab::<NUM_CLASSES>::empty();
    let ok = unsafe { slab.init(region.as_mut_ptr(), num_cpus, SHIFT, &capacities) };
    assert!(ok, "slab layout doesn't fit in 2^{SHIFT} bytes");

    RSEQ.with(|r| {
        let rseq_ptr = r.rseq_ptr().expect("rseq available");

        // Push 5 pointers to class 1.
        let values: Vec<usize> = (100..105).collect();
        for &v in &values {
            loop {
                if unsafe { slab.push(rseq_ptr, 1, v as *mut u8) }.is_some() {
                    break;
                }
            }
        }
        println!("  pushed 5 pointers to class 1");

        // Pop them back (LIFO order).
        print!("  popped:");
        for _ in 0..5 {
            let ptr = loop {
                if let Some(p) = unsafe { slab.pop(rseq_ptr, 1) } {
                    break p;
                }
            };
            print!(" {}", ptr as usize);
        }
        println!();

        // Pop from empty class → None (not abort, just empty).
        let empty = unsafe { slab.pop(rseq_ptr, 1) };
        println!(
            "  pop from empty class 1: {}",
            if empty.is_none() {
                "None (correct)"
            } else {
                "Some (unexpected!)"
            }
        );
    });

    println!("\nDone.");
}

use rstcmalloc::TcMalloc;
use std::collections::HashMap;
use std::time::Instant;

#[global_allocator]
static GLOBAL: TcMalloc = TcMalloc;

fn main() {
    println!("rstcmalloc demo");
    println!("===============\n");

    // Basic allocations
    let boxed = Box::new(42u64);
    println!("Box<u64>:    value = {boxed}");

    let mut v: Vec<i32> = (0..1000).collect();
    println!("Vec<i32>:    len = {}, cap = {}", v.len(), v.capacity());
    v.sort_unstable_by(|a, b| b.cmp(a));
    println!("  sorted[0] = {}, sorted[999] = {}", v[0], v[999]);

    let s: String = (0..100).map(|i| format!("{i} ")).collect();
    println!("String:      len = {}", s.len());

    let mut map = HashMap::new();
    for i in 0..500 {
        map.insert(i, format!("val_{i}"));
    }
    println!("HashMap:     len = {}", map.len());

    // Large allocation (bypasses size classes, goes directly to page heap)
    let big = vec![0u8; 1024 * 1024]; // 1 MiB
    println!("Large alloc: {} bytes, all zero = {}", big.len(), big.iter().all(|&b| b == 0));

    // Multi-threaded workload
    println!("\nMulti-threaded benchmark (8 threads, 100k allocs each):");
    let start = Instant::now();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                let mut vecs: Vec<Vec<u64>> = Vec::new();
                for i in 0u64..100_000 {
                    vecs.push(vec![i; 8]);
                    if vecs.len() > 100 {
                        vecs.drain(..50);
                    }
                }
                vecs.len()
            })
        })
        .collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let elapsed = start.elapsed();
    println!("  completed in {elapsed:?} ({total} live vecs remaining)");

    println!("\nDone.");
}

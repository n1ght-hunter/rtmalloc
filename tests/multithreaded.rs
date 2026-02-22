//! Multi-threaded integration test.

use rtmalloc::RtMalloc;
use std::sync::Arc;

#[global_allocator]
static GLOBAL: RtMalloc = RtMalloc;

#[test]
fn test_multithreaded_alloc() {
    let num_threads = 8;
    let iterations = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            std::thread::spawn(move || {
                let mut vecs: Vec<Vec<u64>> = Vec::new();
                for i in 0..iterations {
                    let v: Vec<u64> = (0..50).map(|x| x + t * iterations + i).collect();
                    vecs.push(v);
                    if vecs.len() > 10 {
                        vecs.remove(0);
                    }
                }
                vecs.len()
            })
        })
        .collect();

    for h in handles {
        let result = h.join().unwrap();
        assert!(result > 0);
    }
}

#[test]
fn test_cross_thread_free() {
    // Allocate on one thread, free on another
    let num_threads = 4;
    let items_per_thread = 500;

    let (tx, rx) = std::sync::mpsc::channel::<Vec<Box<[u8; 64]>>>();

    // Producer threads
    let producers: Vec<_> = (0..num_threads)
        .map(|_| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let items: Vec<Box<[u8; 64]>> = (0..items_per_thread)
                    .map(|i| {
                        let mut arr = [0u8; 64];
                        arr[0] = (i & 0xFF) as u8;
                        Box::new(arr)
                    })
                    .collect();
                tx.send(items).unwrap();
            })
        })
        .collect();

    drop(tx);

    // Consumer: collect and drop all items
    let mut total = 0;
    for items in rx {
        total += items.len();
        drop(items); // Free memory allocated by other threads
    }

    for p in producers {
        p.join().unwrap();
    }

    assert_eq!(total, num_threads * items_per_thread);
}

#[test]
fn test_arc_shared() {
    let data = Arc::new(vec![1u64, 2, 3, 4, 5]);
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let data = Arc::clone(&data);
            std::thread::spawn(move || {
                assert_eq!(data.len(), 5);
                assert_eq!(data[2], 3);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_mixed_sizes_multithreaded() {
    let handles: Vec<_> = (0..4)
        .map(|_| {
            std::thread::spawn(|| {
                let mut allocs: Vec<Box<dyn std::any::Any>> = Vec::new();
                for i in 0..200 {
                    match i % 5 {
                        0 => allocs.push(Box::new([0u8; 8])),
                        1 => allocs.push(Box::new([0u8; 64])),
                        2 => allocs.push(Box::new([0u8; 512])),
                        3 => allocs.push(Box::new([0u8; 4096])),
                        _ => allocs.push(Box::new(vec![0u8; 16384])),
                    }
                    if allocs.len() > 50 {
                        allocs.drain(..25);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

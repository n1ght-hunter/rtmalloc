//! Stress tests with fill-pattern corruption detection.
//!
//! Inspired by mimalloc's test-stress: allocate memory, fill with a known
//! pattern, pass between threads, and verify the pattern before freeing.
//! Any corruption (use-after-free, double-free, buffer overflow) will
//! cause a pattern mismatch and assertion failure.

use rstcmalloc::TcMalloc;
use std::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static GLOBAL: TcMalloc = TcMalloc;

/// Fill a buffer with a deterministic pattern derived from its address and size.
fn fill_pattern(ptr: *mut u8, size: usize) {
    let seed = ptr as usize ^ size;
    for i in 0..size {
        unsafe {
            *ptr.add(i) = ((seed.wrapping_add(i).wrapping_mul(0x9E37_79B9)) & 0xFF) as u8;
        }
    }
}

/// Verify the fill pattern. Returns true if intact.
fn check_pattern(ptr: *mut u8, size: usize) -> bool {
    let seed = ptr as usize ^ size;
    for i in 0..size {
        let expected = ((seed.wrapping_add(i).wrapping_mul(0x9E37_79B9)) & 0xFF) as u8;
        if unsafe { *ptr.add(i) } != expected {
            return false;
        }
    }
    true
}

#[test]
fn stress_fill_pattern_single_thread() {
    let sizes: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 4096, 8192];
    let rounds = 200;

    let mut live: Vec<(*mut u8, Layout)> = Vec::new();

    for round in 0..rounds {
        // Allocate a batch
        for &size in sizes {
            let layout = Layout::from_size_align(size, 8).unwrap();
            let ptr = unsafe { GLOBAL.alloc(layout) };
            assert!(!ptr.is_null(), "alloc failed for size {size}");
            fill_pattern(ptr, size);
            live.push((ptr, layout));
        }

        // Verify all live allocations
        for &(ptr, layout) in &live {
            assert!(
                check_pattern(ptr, layout.size()),
                "corruption detected at round {round}, size {}",
                layout.size()
            );
        }

        // Free half (interleaved to stress free-list ordering)
        let drain_count = live.len() / 2;
        for _ in 0..drain_count {
            let idx = (round * 7 + 3) % live.len();
            let (ptr, layout) = live.swap_remove(idx);
            assert!(
                check_pattern(ptr, layout.size()),
                "corruption before free at round {round}"
            );
            unsafe { GLOBAL.dealloc(ptr, layout) };
        }
    }

    // Final cleanup
    for (ptr, layout) in live {
        assert!(check_pattern(ptr, layout.size()));
        unsafe { GLOBAL.dealloc(ptr, layout) };
    }
}

#[test]
fn stress_fill_pattern_cross_thread() {
    use std::sync::mpsc;

    let npairs = 4;
    let ops = 500;
    let sizes: &[usize] = &[16, 64, 256, 1024];

    let mut producers = Vec::new();
    let mut consumers = Vec::new();

    for pair_id in 0..npairs {
        // Send raw ptr + layout; we know ownership transfers cleanly.
        let (tx, rx) = mpsc::channel::<(usize, Layout)>();

        producers.push(std::thread::spawn(move || {
            for i in 0..ops {
                let size = sizes[(pair_id + i) % sizes.len()];
                let layout = Layout::from_size_align(size, 8).unwrap();
                let ptr = unsafe { GLOBAL.alloc(layout) };
                assert!(!ptr.is_null());
                fill_pattern(ptr, size);
                // Send as usize to satisfy Send
                tx.send((ptr as usize, layout)).unwrap();
            }
        }));

        consumers.push(std::thread::spawn(move || {
            let mut count = 0usize;
            for (addr, layout) in rx {
                let ptr = addr as *mut u8;
                assert!(
                    check_pattern(ptr, layout.size()),
                    "cross-thread corruption at pair {pair_id}, item {count}"
                );
                unsafe { GLOBAL.dealloc(ptr, layout) };
                count += 1;
            }
            count
        }));
    }

    for h in producers {
        h.join().unwrap();
    }

    let total: usize = consumers.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, npairs * ops);
}

#[test]
fn stress_realloc_pattern() {
    let initial_size = 64;
    let layout = Layout::from_size_align(initial_size, 8).unwrap();

    for _ in 0..100 {
        let ptr = unsafe { GLOBAL.alloc(layout) };
        assert!(!ptr.is_null());
        fill_pattern(ptr, initial_size);

        // Grow
        let new_size = 256;
        let new_ptr = unsafe { GLOBAL.realloc(ptr, layout, new_size) };
        assert!(!new_ptr.is_null());
        // Original content should be preserved
        assert!(
            check_pattern(new_ptr, initial_size),
            "realloc corrupted original content during grow"
        );

        // Shrink
        let new_layout = Layout::from_size_align(new_size, 8).unwrap();
        let shrunk_size = 32;
        let shrunk_ptr = unsafe { GLOBAL.realloc(new_ptr, new_layout, shrunk_size) };
        assert!(!shrunk_ptr.is_null());
        // First shrunk_size bytes should still match
        assert!(
            check_pattern(shrunk_ptr, shrunk_size),
            "realloc corrupted content during shrink"
        );

        let shrunk_layout = Layout::from_size_align(shrunk_size, 8).unwrap();
        unsafe { GLOBAL.dealloc(shrunk_ptr, shrunk_layout) };
    }
}

#[test]
fn stress_many_threads_concurrent() {
    // Many threads doing alloc+fill+verify+free simultaneously
    let nthreads = 16;
    let ops_per_thread = 500;

    let handles: Vec<_> = (0..nthreads)
        .map(|tid| {
            std::thread::spawn(move || {
                let mut live: Vec<(*mut u8, Layout)> = Vec::with_capacity(64);

                for i in 0..ops_per_thread {
                    let size = [8, 32, 64, 128, 512, 2048][(tid + i) % 6];
                    let layout = Layout::from_size_align(size, 8).unwrap();
                    let ptr = unsafe { GLOBAL.alloc(layout) };
                    assert!(!ptr.is_null());
                    fill_pattern(ptr, size);
                    live.push((ptr, layout));

                    // Periodically verify and free some
                    if live.len() > 32 {
                        for _ in 0..16 {
                            let (ptr, layout) = live.pop().unwrap();
                            assert!(
                                check_pattern(ptr, layout.size()),
                                "thread {tid} corruption at op {i}"
                            );
                            unsafe { GLOBAL.dealloc(ptr, layout) };
                        }
                    }
                }

                for (ptr, layout) in live {
                    assert!(check_pattern(ptr, layout.size()));
                    unsafe { GLOBAL.dealloc(ptr, layout) };
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

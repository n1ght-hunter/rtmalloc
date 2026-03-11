//! Allocator benchmarks comparing rtmalloc variants against other allocators.
#![allow(unexpected_cfgs)]

use criterion::{Criterion, Throughput};
use rtmalloc_bench::*;
use std::alloc::{GlobalAlloc, Layout};
use std::hint::black_box;

unsafe fn alloc_dealloc(allocator: &dyn GlobalAlloc, layout: Layout) {
    let ptr = unsafe { allocator.alloc(layout) };
    assert!(!ptr.is_null());
    unsafe { allocator.dealloc(ptr, layout) };
}

unsafe fn alloc_n_then_free(allocator: &dyn GlobalAlloc, layout: Layout, n: usize) {
    let mut ptrs = Vec::with_capacity(n);
    for _ in 0..n {
        let ptr = unsafe { allocator.alloc(layout) };
        assert!(!ptr.is_null());
        ptrs.push(ptr);
    }
    for ptr in ptrs.into_iter().rev() {
        unsafe { allocator.dealloc(ptr, layout) };
    }
}

unsafe fn churn(allocator: &dyn GlobalAlloc, layout: Layout, rounds: usize) {
    let mut live: Vec<*mut u8> = Vec::new();
    for _ in 0..rounds {
        for _ in 0..10 {
            let ptr = unsafe { allocator.alloc(layout) };
            assert!(!ptr.is_null());
            live.push(ptr);
        }
        let drain = live.len() / 2;
        for _ in 0..drain {
            let ptr = live.pop().unwrap();
            unsafe { allocator.dealloc(ptr, layout) };
        }
    }
    for ptr in live {
        unsafe { allocator.dealloc(ptr, layout) };
    }
}

fn bench_single_alloc_dealloc(c: &mut Criterion) {
    let sizes: &[usize] = &[8, 64, 256, 1024, 4096, 65536];
    let mut group = c.benchmark_group("single_alloc_dealloc");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(1));
        bench_all_allocators_param!(group, size, |b, alloc| {
            b.iter(|| unsafe { alloc_dealloc(alloc, layout) })
        });
    }
    group.finish();
}

fn bench_batch_alloc_free(c: &mut Criterion) {
    let sizes: &[usize] = &[8, 64, 512, 4096];
    let n = 5000;
    let mut group = c.benchmark_group("batch_5000");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(n as u64));
        bench_all_allocators_param!(group, size, |b, alloc| {
            b.iter(|| unsafe { alloc_n_then_free(alloc, layout, n) })
        });
    }
    group.finish();
}

fn bench_churn(c: &mut Criterion) {
    let sizes: &[usize] = &[32, 256, 2048];
    let rounds = 1000;
    let mut group = c.benchmark_group("churn");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(rounds as u64 * 10));
        bench_all_allocators_param!(group, size, |b, alloc| {
            b.iter(|| unsafe { churn(alloc, layout, rounds) })
        });
    }
    group.finish();
}

fn bench_vec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_growth");
    let final_len: usize = 50_000;
    group.throughput(Throughput::Elements(final_len as u64));

    fn simulate_vec_growth(allocator: &dyn GlobalAlloc, n: usize) {
        let elem = std::mem::size_of::<u64>();
        let mut cap = 1usize;
        let mut layout = Layout::from_size_align(cap * elem, 8).unwrap();
        let mut ptr = unsafe { allocator.alloc(layout) };
        assert!(!ptr.is_null());

        let mut len = 0;
        while len < n {
            len += 1;
            if len > cap {
                let new_cap = cap * 2;
                let new_layout = Layout::from_size_align(new_cap * elem, 8).unwrap();
                let new_ptr = unsafe { allocator.realloc(ptr, layout, new_cap * elem) };
                assert!(!new_ptr.is_null());
                ptr = new_ptr;
                layout = new_layout;
                cap = new_cap;
            }
        }
        unsafe { allocator.dealloc(ptr, layout) };
    }

    bench_all_allocators!(group, "", |b, alloc| {
        b.iter(|| simulate_vec_growth(alloc, black_box(final_len)))
    });

    group.finish();
}

fn bench_multithreaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("multithread_4t");
    let ops_per_thread = 20_000usize;
    let nthreads = 4;
    group.throughput(Throughput::Elements((ops_per_thread * nthreads) as u64));

    fn mt_workload<A: GlobalAlloc + Sync>(allocator: &'static A, nthreads: usize, ops: usize) {
        let layout = Layout::from_size_align(64, 8).unwrap();
        let handles: Vec<_> = (0..nthreads)
            .map(|_| {
                std::thread::spawn(move || {
                    let mut ptrs: Vec<*mut u8> = Vec::with_capacity(100);
                    for _ in 0..ops {
                        let ptr = unsafe { allocator.alloc(layout) };
                        assert!(!ptr.is_null());
                        ptrs.push(ptr);
                        if ptrs.len() > 50 {
                            for _ in 0..25 {
                                let p = ptrs.pop().unwrap();
                                unsafe { allocator.dealloc(p, layout) };
                            }
                        }
                    }
                    for p in ptrs {
                        unsafe { allocator.dealloc(p, layout) };
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    bench_all_allocators_static!(group, "", |b, alloc| {
        b.iter(|| mt_workload(alloc, nthreads, ops_per_thread))
    });

    group.finish();
}

fn bench_cross_thread_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_thread_free");
    let ops = 10_000usize;
    let nthreads = 4;
    group.throughput(Throughput::Elements((ops * nthreads) as u64));

    fn cross_thread_workload<A: GlobalAlloc + Sync>(
        allocator: &'static A,
        nthreads: usize,
        ops: usize,
    ) {
        use std::sync::mpsc;
        let layout = Layout::from_size_align(64, 8).unwrap();

        let mut producer_handles = Vec::new();
        let mut consumer_handles = Vec::new();

        for _ in 0..nthreads {
            let (tx, rx) = mpsc::channel::<SendPtr>();

            producer_handles.push(std::thread::spawn(move || {
                for _ in 0..ops {
                    let ptr = unsafe { allocator.alloc(layout) };
                    assert!(!ptr.is_null());
                    tx.send(SendPtr(ptr)).unwrap();
                }
            }));

            consumer_handles.push(std::thread::spawn(move || {
                for SendPtr(ptr) in rx {
                    unsafe { allocator.dealloc(ptr, layout) };
                }
            }));
        }

        for h in producer_handles {
            h.join().unwrap();
        }
        for h in consumer_handles {
            h.join().unwrap();
        }
    }

    bench_all_allocators_static!(group, "", |b, alloc| {
        b.iter(|| cross_thread_workload(alloc, nthreads, ops))
    });

    group.finish();
}

fn bench_thread_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("thread_scalability");
    let ops_per_thread = 10_000usize;

    fn scale_workload<A: GlobalAlloc + Sync>(allocator: &'static A, nthreads: usize, ops: usize) {
        let layout = Layout::from_size_align(64, 8).unwrap();
        let handles: Vec<_> = (0..nthreads)
            .map(|_| {
                std::thread::spawn(move || {
                    let mut ptrs: Vec<*mut u8> = Vec::with_capacity(64);
                    for _ in 0..ops {
                        let ptr = unsafe { allocator.alloc(layout) };
                        assert!(!ptr.is_null());
                        ptrs.push(ptr);
                        if ptrs.len() > 32 {
                            for _ in 0..16 {
                                let p = ptrs.pop().unwrap();
                                unsafe { allocator.dealloc(p, layout) };
                            }
                        }
                    }
                    for p in ptrs {
                        unsafe { allocator.dealloc(p, layout) };
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    for &nthreads in &[1usize, 2, 4, 8] {
        group.throughput(Throughput::Elements((ops_per_thread * nthreads) as u64));
        bench_all_allocators_static!(group, nthreads, |b, alloc| {
            b.iter(|| scale_workload(alloc, nthreads, ops_per_thread))
        });
    }

    group.finish();
}

fn bench_mixed_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_sizes");
    let n = 10_000usize;
    group.throughput(Throughput::Elements(n as u64));

    fn mixed_workload(allocator: &dyn GlobalAlloc, n: usize) {
        let mut rng_state: u64 = 0xDEAD_BEEF_CAFE_BABE;
        let mut next_u64 = || -> u64 {
            rng_state = rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = rng_state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };

        let base_sizes: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024];
        let mut ptrs: Vec<(*mut u8, Layout)> = Vec::with_capacity(n);

        for _ in 0..n {
            let r = next_u64();
            let base = base_sizes[(r as usize) % base_sizes.len()];
            let size = if r % 1000 == 0 {
                base * 1000
            } else if r % 100 == 0 {
                base * 100
            } else {
                base
            };
            let layout = Layout::from_size_align(size, 8).unwrap();
            let ptr = unsafe { allocator.alloc(layout) };
            assert!(!ptr.is_null());
            ptrs.push((ptr, layout));

            if ptrs.len() > 10 && r % 3 == 0 {
                let idx = (next_u64() as usize) % ptrs.len();
                let (p, l) = ptrs.swap_remove(idx);
                unsafe { allocator.dealloc(p, l) };
            }
        }

        for (p, l) in ptrs {
            unsafe { allocator.dealloc(p, l) };
        }
    }

    bench_all_allocators!(group, "", |b, alloc| {
        b.iter(|| mixed_workload(alloc, black_box(n)))
    });

    group.finish();
}

fn bench_producer_consumer(c: &mut Criterion) {
    let mut group = c.benchmark_group("producer_consumer");
    let ops_per_producer = 10_000usize;
    let npairs = 4;
    group.throughput(Throughput::Elements((ops_per_producer * npairs) as u64));

    fn pc_workload<A: GlobalAlloc + Sync>(allocator: &'static A, npairs: usize, ops: usize) {
        use std::sync::mpsc;

        let sizes: &[usize] = &[16, 64, 256, 1024];
        let mut producers = Vec::new();
        let mut consumers = Vec::new();

        for pair_id in 0..npairs {
            let (tx, rx) = mpsc::channel::<(SendPtr, Layout)>();

            producers.push(std::thread::spawn(move || {
                for i in 0..ops {
                    let size = sizes[(pair_id + i) % sizes.len()];
                    let layout = Layout::from_size_align(size, 8).unwrap();
                    let ptr = unsafe { allocator.alloc(layout) };
                    assert!(!ptr.is_null());
                    tx.send((SendPtr(ptr), layout)).unwrap();
                }
            }));

            consumers.push(std::thread::spawn(move || {
                for (SendPtr(ptr), layout) in rx {
                    unsafe { allocator.dealloc(ptr, layout) };
                }
            }));
        }

        for h in producers {
            h.join().unwrap();
        }
        for h in consumers {
            h.join().unwrap();
        }
    }

    bench_all_allocators_static!(group, "", |b, alloc| {
        b.iter(|| pc_workload(alloc, npairs, ops_per_producer))
    });

    group.finish();
}

fn bench_cache_scratch(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_scratch");
    let iters_per_thread = 1_000_000usize;

    fn cache_scratch_workload<A: GlobalAlloc + Sync>(
        allocator: &'static A,
        nthreads: usize,
        iters: usize,
    ) {
        use std::sync::Arc;
        use std::sync::Barrier;

        let layout = Layout::from_size_align(64, 8).unwrap();
        let barrier = Arc::new(Barrier::new(nthreads));

        let handles: Vec<_> = (0..nthreads)
            .map(|_| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let ptr = unsafe { allocator.alloc(layout) };
                    assert!(!ptr.is_null());
                    let p = ptr as *mut u64;

                    barrier.wait();

                    for _ in 0..iters {
                        unsafe { p.write_volatile(p.read_volatile().wrapping_add(1)) };
                    }

                    unsafe { allocator.dealloc(ptr, layout) };
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }

    for &nthreads in &[1usize, 2, 4, 8] {
        group.throughput(Throughput::Elements((iters_per_thread * nthreads) as u64));
        bench_all_allocators_static!(group, nthreads, |b, alloc| {
            b.iter(|| cache_scratch_workload(alloc, nthreads, iters_per_thread))
        });
    }

    group.finish();
}

fn bench_large_alloc(c: &mut Criterion) {
    let sizes: &[usize] = &[256 * 1024, 1024 * 1024, 4 * 1024 * 1024, 16 * 1024 * 1024];
    let mut group = c.benchmark_group("large_alloc");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        let label = if size >= 1024 * 1024 {
            format!("{}MB", size / (1024 * 1024))
        } else {
            format!("{}KB", size / 1024)
        };
        group.throughput(Throughput::Elements(1));
        bench_all_allocators_param!(group, &label, |b, alloc| {
            b.iter(|| unsafe { alloc_dealloc(alloc, layout) })
        });
    }
    group.finish();
}

fn bench_aligned_alloc(c: &mut Criterion) {
    let aligns: &[(usize, &str)] = &[(16, "16_sse"), (64, "64_cacheline"), (4096, "4096_page")];
    let size = 256usize;
    let mut group = c.benchmark_group("aligned_alloc");
    group.throughput(Throughput::Elements(1));

    for &(align, label) in aligns {
        let layout = Layout::from_size_align(size, align).unwrap();
        bench_all_allocators!(group, format!("/{label}"), |b, alloc| {
            b.iter(|| unsafe { alloc_dealloc(alloc, layout) })
        });
    }

    group.finish();
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let sample_size: usize = env_or("BENCH_SAMPLE_SIZE", 10);
    let warmup_secs: u64 = env_or("BENCH_WARMUP_SECS", 1);
    let measure_secs: u64 = env_or("BENCH_MEASURE_SECS", 2);

    let mut criterion = {
        #[cfg(codspeed)]
        let mut c = Criterion::new_instrumented();
        #[cfg(not(codspeed))]
        let mut c = Criterion::default();
        c = c
            .sample_size(sample_size)
            .warm_up_time(std::time::Duration::from_secs(warmup_secs))
            .measurement_time(std::time::Duration::from_secs(measure_secs))
            .configure_from_args();
        c
    };

    bench_single_alloc_dealloc(&mut criterion);
    bench_batch_alloc_free(&mut criterion);
    bench_churn(&mut criterion);
    bench_vec_push(&mut criterion);
    bench_multithreaded(&mut criterion);
    bench_cross_thread_free(&mut criterion);
    bench_thread_scalability(&mut criterion);
    bench_mixed_sizes(&mut criterion);
    bench_producer_consumer(&mut criterion);
    bench_cache_scratch(&mut criterion);
    bench_large_alloc(&mut criterion);
    bench_aligned_alloc(&mut criterion);

    summary::recolor_svgs();
    summary::print_summary();
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

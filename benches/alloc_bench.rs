//! Allocator benchmarks comparing rstcmalloc vs system allocator vs mimalloc vs tcmalloc2.
//!
//! Since #[global_allocator] is process-wide and cannot be switched at runtime,
//! each allocator is tested via its raw GlobalAlloc interface directly.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::{alloc::{GlobalAlloc, Layout, System}, hint::black_box};

use mimalloc::MiMalloc;
use rstcmalloc::TcMalloc;


static TCMALLOC: TcMalloc = TcMalloc;
static MIMALLOC: MiMalloc = MiMalloc;
static GOOGLE_TC: GoogleTcMalloc = GoogleTcMalloc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Allocate + deallocate a single object of `size` bytes.
unsafe fn alloc_dealloc(allocator: &dyn GlobalAlloc, layout: Layout) {
    let ptr = unsafe { allocator.alloc(layout) };
    assert!(!ptr.is_null());
    unsafe { allocator.dealloc(ptr, layout) };
}

/// Allocate N objects, then free them all (LIFO order).
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

/// Interleaved alloc/free pattern: allocate a batch, free half, allocate more.
unsafe fn churn(allocator: &dyn GlobalAlloc, layout: Layout, rounds: usize) {
    let mut live: Vec<*mut u8> = Vec::new();
    for _ in 0..rounds {
        // Allocate batch
        for _ in 0..10 {
            let ptr = unsafe { allocator.alloc(layout) };
            assert!(!ptr.is_null());
            live.push(ptr);
        }
        // Free half
        let drain = live.len() / 2;
        for _ in 0..drain {
            let ptr = live.pop().unwrap();
            unsafe { allocator.dealloc(ptr, layout) };
        }
    }
    // Cleanup
    for ptr in live {
        unsafe { allocator.dealloc(ptr, layout) };
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_single_alloc_dealloc(c: &mut Criterion) {
    let sizes: &[usize] = &[8, 64, 256, 1024, 4096, 65536];
    let mut group = c.benchmark_group("single_alloc_dealloc");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&System, layout) })
        });
        group.bench_with_input(BenchmarkId::new("rstcmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&TCMALLOC, layout) })
        });
        group.bench_with_input(BenchmarkId::new("mimalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&MIMALLOC, layout) })
        });
        group.bench_with_input(BenchmarkId::new("google_tc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&GOOGLE_TC, layout) })
        });
    }
    group.finish();
}

fn bench_batch_alloc_free(c: &mut Criterion) {
    let sizes: &[usize] = &[8, 64, 512, 4096];
    let n = 1000;
    let mut group = c.benchmark_group("batch_1000_alloc_then_free");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&System, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("rstcmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&TCMALLOC, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("mimalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&MIMALLOC, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("google_tc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&GOOGLE_TC, layout, n) })
        });
    }
    group.finish();
}

fn bench_churn(c: &mut Criterion) {
    let sizes: &[usize] = &[32, 256, 2048];
    let rounds = 200;
    let mut group = c.benchmark_group("churn_200_rounds");

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(rounds as u64 * 10));

        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&System, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("rstcmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&TCMALLOC, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("mimalloc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&MIMALLOC, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("google_tc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&GOOGLE_TC, layout, rounds) })
        });
    }
    group.finish();
}

fn bench_vec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulated_vec_growth");
    let final_len: usize = 10_000;
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

    group.bench_function("system", |b| {
        b.iter(|| simulate_vec_growth(&System, black_box(final_len)))
    });
    group.bench_function("rstcmalloc", |b| {
        b.iter(|| simulate_vec_growth(&TCMALLOC, black_box(final_len)))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| simulate_vec_growth(&MIMALLOC, black_box(final_len)))
    });
    group.bench_function("google_tc", |b| {
        b.iter(|| simulate_vec_growth(&GOOGLE_TC, black_box(final_len)))
    });

    group.finish();
}

fn bench_multithreaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("multithreaded_4_threads");
    let ops_per_thread = 5000usize;
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

    static SYS: System = System;

    group.bench_function("system", |b| {
        b.iter(|| mt_workload(&SYS, nthreads, ops_per_thread))
    });
    group.bench_function("rstcmalloc", |b| {
        b.iter(|| mt_workload(&TCMALLOC, nthreads, ops_per_thread))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| mt_workload(&MIMALLOC, nthreads, ops_per_thread))
    });
    group.bench_function("google_tc", |b| {
        b.iter(|| mt_workload(&GOOGLE_TC, nthreads, ops_per_thread))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_alloc_dealloc,
    bench_batch_alloc_free,
    bench_churn,
    bench_vec_push,
    bench_multithreaded,
);
criterion_main!(benches);

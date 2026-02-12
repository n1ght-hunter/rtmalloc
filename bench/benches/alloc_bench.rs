//! Allocator benchmarks comparing rstcmalloc vs system allocator vs mimalloc vs google tcmalloc.
//!
//! Since #[global_allocator] is process-wide and cannot be switched at runtime,
//! each allocator is tested via its raw GlobalAlloc interface directly.
//!
//! rstcmalloc is linked as a staticlib (built with --profile fast by build.rs).
//! After criterion finishes, a colored comparison table is printed.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;

use mimalloc::MiMalloc;
use rpmalloc::RpMalloc;
use snmalloc_rs::SnMalloc;
#[cfg(has_jemalloc)]
use tikv_jemallocator::Jemalloc;

// ---------------------------------------------------------------------------
// rstcmalloc FFI (statically linked, built by build.rs with --profile fast)
// ---------------------------------------------------------------------------

mod rstcmalloc_ffi {
    use std::alloc::{GlobalAlloc, Layout};

    unsafe extern "C" {
        // Nightly variant (#[thread_local] thread cache)
        fn rstcmalloc_nightly_alloc(size: usize, align: usize) -> *mut u8;
        fn rstcmalloc_nightly_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rstcmalloc_nightly_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;

        // Std variant (std::thread_local! thread cache)
        fn rstcmalloc_std_alloc(size: usize, align: usize) -> *mut u8;
        fn rstcmalloc_std_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rstcmalloc_std_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;

        // Nostd variant (central cache only, no thread cache)
        fn rstcmalloc_nostd_alloc(size: usize, align: usize) -> *mut u8;
        fn rstcmalloc_nostd_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rstcmalloc_nostd_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;
    }

    // Per-CPU variant (rseq, Linux x86_64 only)
    #[cfg(has_rstcmalloc_percpu)]
    unsafe extern "C" {
        fn rstcmalloc_percpu_alloc(size: usize, align: usize) -> *mut u8;
        fn rstcmalloc_percpu_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rstcmalloc_percpu_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;
    }

    macro_rules! impl_ffi_alloc {
        ($name:ident, $alloc:ident, $dealloc:ident, $realloc:ident) => {
            pub struct $name;

            unsafe impl GlobalAlloc for $name {
                unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                    unsafe { $alloc(layout.size(), layout.align()) }
                }
                unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                    unsafe { $dealloc(ptr, layout.size(), layout.align()) }
                }
                unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
                    unsafe { $realloc(ptr, layout.size(), layout.align(), new_size) }
                }
            }

            unsafe impl Send for $name {}
            unsafe impl Sync for $name {}
        };
    }

    impl_ffi_alloc!(
        RstcmallocNightly,
        rstcmalloc_nightly_alloc,
        rstcmalloc_nightly_dealloc,
        rstcmalloc_nightly_realloc
    );
    impl_ffi_alloc!(
        RstcmallocStd,
        rstcmalloc_std_alloc,
        rstcmalloc_std_dealloc,
        rstcmalloc_std_realloc
    );
    impl_ffi_alloc!(
        RstcmallocNostd,
        rstcmalloc_nostd_alloc,
        rstcmalloc_nostd_dealloc,
        rstcmalloc_nostd_realloc
    );
    #[cfg(has_rstcmalloc_percpu)]
    impl_ffi_alloc!(
        RstcmallocPercpu,
        rstcmalloc_percpu_alloc,
        rstcmalloc_percpu_dealloc,
        rstcmalloc_percpu_realloc
    );
}

#[cfg(has_rstcmalloc_percpu)]
use rstcmalloc_ffi::RstcmallocPercpu;
use rstcmalloc_ffi::{RstcmallocNightly, RstcmallocNostd, RstcmallocStd};

// ---------------------------------------------------------------------------
// Google tcmalloc FFI (statically linked when available)
// ---------------------------------------------------------------------------

#[cfg(has_google_tcmalloc)]
mod google_tc {
    use std::alloc::{GlobalAlloc, Layout};

    #[allow(clippy::duplicated_attributes)]
    #[link(name = "tcmalloc_minimal", kind = "static")]
    #[link(name = "common", kind = "static")]
    #[link(name = "low_level_alloc", kind = "static")]
    unsafe extern "C" {
        fn tc_malloc(size: usize) -> *mut u8;
        fn tc_free(ptr: *mut u8);
        fn tc_realloc(ptr: *mut u8, size: usize) -> *mut u8;
        fn tc_memalign(align: usize, size: usize) -> *mut u8;
    }

    pub struct GoogleTcMalloc;

    unsafe impl GlobalAlloc for GoogleTcMalloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if layout.align() <= 8 {
                unsafe { tc_malloc(layout.size()) }
            } else {
                unsafe { tc_memalign(layout.align(), layout.size()) }
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            unsafe { tc_free(ptr) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
            unsafe { tc_realloc(ptr, new_size) }
        }
    }

    unsafe impl Sync for GoogleTcMalloc {}
    unsafe impl Send for GoogleTcMalloc {}
}

#[cfg(has_google_tcmalloc)]
use google_tc::GoogleTcMalloc;

static TCMALLOC_NIGHTLY: RstcmallocNightly = RstcmallocNightly;
static TCMALLOC_STD: RstcmallocStd = RstcmallocStd;
static TCMALLOC_NOSTD: RstcmallocNostd = RstcmallocNostd;
#[cfg(has_rstcmalloc_percpu)]
static TCMALLOC_PERCPU: RstcmallocPercpu = RstcmallocPercpu;
static MIMALLOC: MiMalloc = MiMalloc;
static SNMALLOC: SnMalloc = SnMalloc;
static RPMALLOC: RpMalloc = RpMalloc;
#[cfg(has_jemalloc)]
static JEMALLOC: Jemalloc = Jemalloc;
#[cfg(has_google_tcmalloc)]
static GOOGLE_TC: GoogleTcMalloc = GoogleTcMalloc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrapper to send raw pointers across threads in benchmarks.
/// Safety: the benchmarks ensure each pointer is only used by one thread at a time.
struct SendPtr(*mut u8);
unsafe impl Send for SendPtr {}

// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_single_alloc_dealloc(c: &mut Criterion) {
    let sizes: &[usize] = &[8, 64, 256, 1024, 4096, 65536];
    let mut group = c.benchmark_group("single_alloc_dealloc");
    group.sample_size(50);

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&System, layout) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_nightly", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&TCMALLOC_NIGHTLY, layout) })
        });
        #[cfg(has_rstcmalloc_percpu)]
        group.bench_with_input(BenchmarkId::new("rstc_percpu", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&TCMALLOC_PERCPU, layout) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_std", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&TCMALLOC_STD, layout) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_nostd", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&TCMALLOC_NOSTD, layout) })
        });
        group.bench_with_input(BenchmarkId::new("mimalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&MIMALLOC, layout) })
        });
        #[cfg(has_google_tcmalloc)]
        group.bench_with_input(BenchmarkId::new("google_tc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&GOOGLE_TC, layout) })
        });
        group.bench_with_input(BenchmarkId::new("snmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&SNMALLOC, layout) })
        });
        group.bench_with_input(BenchmarkId::new("rpmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&RPMALLOC, layout) })
        });
        #[cfg(has_jemalloc)]
        group.bench_with_input(BenchmarkId::new("jemalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_dealloc(&JEMALLOC, layout) })
        });
    }
    group.finish();
}

fn bench_batch_alloc_free(c: &mut Criterion) {
    let sizes: &[usize] = &[8, 64, 512, 4096];
    let n = 1000;
    let mut group = c.benchmark_group("batch_1000");
    group.sample_size(30);

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&System, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_nightly", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&TCMALLOC_NIGHTLY, layout, n) })
        });
        #[cfg(has_rstcmalloc_percpu)]
        group.bench_with_input(BenchmarkId::new("rstc_percpu", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&TCMALLOC_PERCPU, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_std", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&TCMALLOC_STD, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_nostd", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&TCMALLOC_NOSTD, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("mimalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&MIMALLOC, layout, n) })
        });
        #[cfg(has_google_tcmalloc)]
        group.bench_with_input(BenchmarkId::new("google_tc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&GOOGLE_TC, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("snmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&SNMALLOC, layout, n) })
        });
        group.bench_with_input(BenchmarkId::new("rpmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&RPMALLOC, layout, n) })
        });
        #[cfg(has_jemalloc)]
        group.bench_with_input(BenchmarkId::new("jemalloc", size), &size, |b, _| {
            b.iter(|| unsafe { alloc_n_then_free(&JEMALLOC, layout, n) })
        });
    }
    group.finish();
}

fn bench_churn(c: &mut Criterion) {
    let sizes: &[usize] = &[32, 256, 2048];
    let rounds = 200;
    let mut group = c.benchmark_group("churn");
    group.sample_size(30);

    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        group.throughput(Throughput::Elements(rounds as u64 * 10));

        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&System, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_nightly", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&TCMALLOC_NIGHTLY, layout, rounds) })
        });
        #[cfg(has_rstcmalloc_percpu)]
        group.bench_with_input(BenchmarkId::new("rstc_percpu", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&TCMALLOC_PERCPU, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_std", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&TCMALLOC_STD, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("rstc_nostd", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&TCMALLOC_NOSTD, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("mimalloc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&MIMALLOC, layout, rounds) })
        });
        #[cfg(has_google_tcmalloc)]
        group.bench_with_input(BenchmarkId::new("google_tc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&GOOGLE_TC, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("snmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&SNMALLOC, layout, rounds) })
        });
        group.bench_with_input(BenchmarkId::new("rpmalloc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&RPMALLOC, layout, rounds) })
        });
        #[cfg(has_jemalloc)]
        group.bench_with_input(BenchmarkId::new("jemalloc", size), &size, |b, _| {
            b.iter(|| unsafe { churn(&JEMALLOC, layout, rounds) })
        });
    }
    group.finish();
}

fn bench_vec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_growth");
    let final_len: usize = 10_000;
    group.throughput(Throughput::Elements(final_len as u64));
    group.sample_size(50);

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
    group.bench_function("rstc_nightly", |b| {
        b.iter(|| simulate_vec_growth(&TCMALLOC_NIGHTLY, black_box(final_len)))
    });
    #[cfg(has_rstcmalloc_percpu)]
    group.bench_function("rstc_percpu", |b| {
        b.iter(|| simulate_vec_growth(&TCMALLOC_PERCPU, black_box(final_len)))
    });
    group.bench_function("rstc_std", |b| {
        b.iter(|| simulate_vec_growth(&TCMALLOC_STD, black_box(final_len)))
    });
    group.bench_function("rstc_nostd", |b| {
        b.iter(|| simulate_vec_growth(&TCMALLOC_NOSTD, black_box(final_len)))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| simulate_vec_growth(&MIMALLOC, black_box(final_len)))
    });
    #[cfg(has_google_tcmalloc)]
    group.bench_function("google_tc", |b| {
        b.iter(|| simulate_vec_growth(&GOOGLE_TC, black_box(final_len)))
    });
    group.bench_function("snmalloc", |b| {
        b.iter(|| simulate_vec_growth(&SNMALLOC, black_box(final_len)))
    });
    group.bench_function("rpmalloc", |b| {
        b.iter(|| simulate_vec_growth(&RPMALLOC, black_box(final_len)))
    });
    #[cfg(has_jemalloc)]
    group.bench_function("jemalloc", |b| {
        b.iter(|| simulate_vec_growth(&JEMALLOC, black_box(final_len)))
    });

    group.finish();
}

fn bench_multithreaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("multithread_4t");
    let ops_per_thread = 5000usize;
    let nthreads = 4;
    group.throughput(Throughput::Elements((ops_per_thread * nthreads) as u64));
    group.sample_size(20);

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
    group.bench_function("rstc_nightly", |b| {
        b.iter(|| mt_workload(&TCMALLOC_NIGHTLY, nthreads, ops_per_thread))
    });
    #[cfg(has_rstcmalloc_percpu)]
    group.bench_function("rstc_percpu", |b| {
        b.iter(|| mt_workload(&TCMALLOC_PERCPU, nthreads, ops_per_thread))
    });
    group.bench_function("rstc_std", |b| {
        b.iter(|| mt_workload(&TCMALLOC_STD, nthreads, ops_per_thread))
    });
    group.bench_function("rstc_nostd", |b| {
        b.iter(|| mt_workload(&TCMALLOC_NOSTD, nthreads, ops_per_thread))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| mt_workload(&MIMALLOC, nthreads, ops_per_thread))
    });
    #[cfg(has_google_tcmalloc)]
    group.bench_function("google_tc", |b| {
        b.iter(|| mt_workload(&GOOGLE_TC, nthreads, ops_per_thread))
    });
    group.bench_function("snmalloc", |b| {
        b.iter(|| mt_workload(&SNMALLOC, nthreads, ops_per_thread))
    });
    group.bench_function("rpmalloc", |b| {
        b.iter(|| mt_workload(&RPMALLOC, nthreads, ops_per_thread))
    });
    #[cfg(has_jemalloc)]
    group.bench_function("jemalloc", |b| {
        b.iter(|| mt_workload(&JEMALLOC, nthreads, ops_per_thread))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Cross-thread free (Larson pattern): alloc on thread A, free on thread B
// ---------------------------------------------------------------------------

fn bench_cross_thread_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_thread_free");
    let ops = 2000usize;
    let nthreads = 4;
    group.throughput(Throughput::Elements((ops * nthreads) as u64));
    group.sample_size(20);

    /// Allocate objects on producer threads, send them to consumer threads for
    /// deallocation. This is the core pattern that stresses thread caches —
    /// the freeing thread didn't allocate the object, so it must return it
    /// through the central path (or transfer cache).
    fn cross_thread_workload<A: GlobalAlloc + Sync>(
        allocator: &'static A,
        nthreads: usize,
        ops: usize,
    ) {
        use std::sync::mpsc;
        let layout = Layout::from_size_align(64, 8).unwrap();

        // Each producer has a paired consumer.
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
        // Producers are done and dropped their tx, consumers will drain and exit.
        for h in consumer_handles {
            h.join().unwrap();
        }
    }

    static SYS2: System = System;

    group.bench_function("system", |b| {
        b.iter(|| cross_thread_workload(&SYS2, nthreads, ops))
    });
    group.bench_function("rstc_nightly", |b| {
        b.iter(|| cross_thread_workload(&TCMALLOC_NIGHTLY, nthreads, ops))
    });
    #[cfg(has_rstcmalloc_percpu)]
    group.bench_function("rstc_percpu", |b| {
        b.iter(|| cross_thread_workload(&TCMALLOC_PERCPU, nthreads, ops))
    });
    group.bench_function("rstc_std", |b| {
        b.iter(|| cross_thread_workload(&TCMALLOC_STD, nthreads, ops))
    });
    group.bench_function("rstc_nostd", |b| {
        b.iter(|| cross_thread_workload(&TCMALLOC_NOSTD, nthreads, ops))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| cross_thread_workload(&MIMALLOC, nthreads, ops))
    });
    #[cfg(has_google_tcmalloc)]
    group.bench_function("google_tc", |b| {
        b.iter(|| cross_thread_workload(&GOOGLE_TC, nthreads, ops))
    });
    group.bench_function("snmalloc", |b| {
        b.iter(|| cross_thread_workload(&SNMALLOC, nthreads, ops))
    });
    group.bench_function("rpmalloc", |b| {
        b.iter(|| cross_thread_workload(&RPMALLOC, nthreads, ops))
    });
    #[cfg(has_jemalloc)]
    group.bench_function("jemalloc", |b| {
        b.iter(|| cross_thread_workload(&JEMALLOC, nthreads, ops))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Thread scalability: same workload at 1, 2, 4, 8 threads
// ---------------------------------------------------------------------------

fn bench_thread_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("thread_scalability");
    let ops_per_thread = 3000usize;
    group.sample_size(15);

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

    static SYS3: System = System;

    for &nthreads in &[1usize, 2, 4, 8] {
        group.throughput(Throughput::Elements((ops_per_thread * nthreads) as u64));

        group.bench_with_input(BenchmarkId::new("system", nthreads), &nthreads, |b, &nt| {
            b.iter(|| scale_workload(&SYS3, nt, ops_per_thread))
        });
        group.bench_with_input(
            BenchmarkId::new("rstc_nightly", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&TCMALLOC_NIGHTLY, nt, ops_per_thread)),
        );
        #[cfg(has_rstcmalloc_percpu)]
        group.bench_with_input(
            BenchmarkId::new("rstc_percpu", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&TCMALLOC_PERCPU, nt, ops_per_thread)),
        );
        group.bench_with_input(
            BenchmarkId::new("rstc_std", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&TCMALLOC_STD, nt, ops_per_thread)),
        );
        group.bench_with_input(
            BenchmarkId::new("rstc_nostd", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&TCMALLOC_NOSTD, nt, ops_per_thread)),
        );
        group.bench_with_input(
            BenchmarkId::new("mimalloc", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&MIMALLOC, nt, ops_per_thread)),
        );
        #[cfg(has_google_tcmalloc)]
        group.bench_with_input(
            BenchmarkId::new("google_tc", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&GOOGLE_TC, nt, ops_per_thread)),
        );
        group.bench_with_input(
            BenchmarkId::new("snmalloc", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&SNMALLOC, nt, ops_per_thread)),
        );
        group.bench_with_input(
            BenchmarkId::new("rpmalloc", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&RPMALLOC, nt, ops_per_thread)),
        );
        #[cfg(has_jemalloc)]
        group.bench_with_input(
            BenchmarkId::new("jemalloc", nthreads),
            &nthreads,
            |b, &nt| b.iter(|| scale_workload(&JEMALLOC, nt, ops_per_thread)),
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Mixed sizes: realistic size distribution (many small, few large)
// ---------------------------------------------------------------------------

fn bench_mixed_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_sizes");
    let n = 2000usize;
    group.throughput(Throughput::Elements(n as u64));
    group.sample_size(30);

    /// Mimalloc-style size distribution: linearly distributed in powers-of-2
    /// buckets, with 1% large objects (100x base) and 0.1% huge objects (1000x).
    /// Uses a deterministic PRNG for reproducibility.
    fn mixed_workload(allocator: &dyn GlobalAlloc, n: usize) {
        // Splitmix64 PRNG (deterministic, fast, good enough for size distribution)
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
            // 1% large, 0.1% huge
            let size = if r % 1000 == 0 {
                base * 1000 // huge
            } else if r % 100 == 0 {
                base * 100 // large
            } else {
                base
            };
            let layout = Layout::from_size_align(size, 8).unwrap();
            let ptr = unsafe { allocator.alloc(layout) };
            assert!(!ptr.is_null());
            ptrs.push((ptr, layout));

            // Free ~30% of live objects to maintain churn
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

    group.bench_function("system", |b| {
        b.iter(|| mixed_workload(&System, black_box(n)))
    });
    group.bench_function("rstc_nightly", |b| {
        b.iter(|| mixed_workload(&TCMALLOC_NIGHTLY, black_box(n)))
    });
    #[cfg(has_rstcmalloc_percpu)]
    group.bench_function("rstc_percpu", |b| {
        b.iter(|| mixed_workload(&TCMALLOC_PERCPU, black_box(n)))
    });
    group.bench_function("rstc_std", |b| {
        b.iter(|| mixed_workload(&TCMALLOC_STD, black_box(n)))
    });
    group.bench_function("rstc_nostd", |b| {
        b.iter(|| mixed_workload(&TCMALLOC_NOSTD, black_box(n)))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| mixed_workload(&MIMALLOC, black_box(n)))
    });
    #[cfg(has_google_tcmalloc)]
    group.bench_function("google_tc", |b| {
        b.iter(|| mixed_workload(&GOOGLE_TC, black_box(n)))
    });
    group.bench_function("snmalloc", |b| {
        b.iter(|| mixed_workload(&SNMALLOC, black_box(n)))
    });
    group.bench_function("rpmalloc", |b| {
        b.iter(|| mixed_workload(&RPMALLOC, black_box(n)))
    });
    #[cfg(has_jemalloc)]
    group.bench_function("jemalloc", |b| {
        b.iter(|| mixed_workload(&JEMALLOC, black_box(n)))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Producer-consumer: N threads allocate only, N threads free only
// ---------------------------------------------------------------------------

fn bench_producer_consumer(c: &mut Criterion) {
    let mut group = c.benchmark_group("producer_consumer");
    let ops_per_producer = 2000usize;
    let npairs = 4;
    group.throughput(Throughput::Elements((ops_per_producer * npairs) as u64));
    group.sample_size(15);

    /// Asymmetric workload: producer threads only allocate and send pointers
    /// through a channel; consumer threads only receive and free. This is the
    /// worst case for thread-local caches because every free goes through the
    /// slow path (the freeing thread never allocated from its own cache).
    fn pc_workload<A: GlobalAlloc + Sync>(allocator: &'static A, npairs: usize, ops: usize) {
        use std::sync::mpsc;

        // Use mixed sizes to stress multiple size classes simultaneously.
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

    static SYS4: System = System;

    group.bench_function("system", |b| {
        b.iter(|| pc_workload(&SYS4, npairs, ops_per_producer))
    });
    group.bench_function("rstc_nightly", |b| {
        b.iter(|| pc_workload(&TCMALLOC_NIGHTLY, npairs, ops_per_producer))
    });
    #[cfg(has_rstcmalloc_percpu)]
    group.bench_function("rstc_percpu", |b| {
        b.iter(|| pc_workload(&TCMALLOC_PERCPU, npairs, ops_per_producer))
    });
    group.bench_function("rstc_std", |b| {
        b.iter(|| pc_workload(&TCMALLOC_STD, npairs, ops_per_producer))
    });
    group.bench_function("rstc_nostd", |b| {
        b.iter(|| pc_workload(&TCMALLOC_NOSTD, npairs, ops_per_producer))
    });
    group.bench_function("mimalloc", |b| {
        b.iter(|| pc_workload(&MIMALLOC, npairs, ops_per_producer))
    });
    #[cfg(has_google_tcmalloc)]
    group.bench_function("google_tc", |b| {
        b.iter(|| pc_workload(&GOOGLE_TC, npairs, ops_per_producer))
    });
    group.bench_function("snmalloc", |b| {
        b.iter(|| pc_workload(&SNMALLOC, npairs, ops_per_producer))
    });
    group.bench_function("rpmalloc", |b| {
        b.iter(|| pc_workload(&RPMALLOC, npairs, ops_per_producer))
    });
    #[cfg(has_jemalloc)]
    group.bench_function("jemalloc", |b| {
        b.iter(|| pc_workload(&JEMALLOC, npairs, ops_per_producer))
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
    bench_cross_thread_free,
    bench_thread_scalability,
    bench_mixed_sizes,
    bench_producer_consumer,
);

// ---------------------------------------------------------------------------
// Colored summary table — reads criterion's saved estimates after benches run
// ---------------------------------------------------------------------------

mod summary {
    use std::collections::BTreeMap;
    use std::path::Path;

    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";
    const DIM: &str = "\x1b[2m";
    const WHITE: &str = "\x1b[37m";
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const YELLOW: &str = "\x1b[33m";
    const BG_GREEN: &str = "\x1b[42m\x1b[30m";

    const MAGENTA: &str = "\x1b[35m";
    const RED: &str = "\x1b[31m";
    const BRIGHT_GREEN: &str = "\x1b[92m";
    const BRIGHT_BLUE: &str = "\x1b[94m";
    const BRIGHT_CYAN: &str = "\x1b[96m";
    const BRIGHT_YELLOW: &str = "\x1b[93m";

    const KNOWN: &[&str] = &[
        "system",
        "rstc_nightly",
        "rstc_percpu",
        "rstc_std",
        "rstc_nostd",
        "mimalloc",
        "google_tc",
        "jemalloc",
        "snmalloc",
        "rpmalloc",
    ];

    fn color_for(name: &str) -> &'static str {
        match name {
            "system" => WHITE,
            "rstc_nightly" => GREEN,
            "rstc_percpu" => BRIGHT_GREEN,
            "rstc_std" => MAGENTA,
            "rstc_nostd" => RED,
            "mimalloc" => CYAN,
            "google_tc" => YELLOW,
            "jemalloc" => BRIGHT_BLUE,
            "snmalloc" => BRIGHT_CYAN,
            "rpmalloc" => BRIGHT_YELLOW,
            _ => WHITE,
        }
    }

    fn format_time(ns: f64) -> String {
        if ns >= 1_000_000.0 {
            format!("{:>8.2} ms", ns / 1_000_000.0)
        } else if ns >= 1_000.0 {
            format!("{:>8.2} us", ns / 1_000.0)
        } else {
            format!("{:>8.1} ns", ns)
        }
    }

    /// Read the point estimate (median ns) from criterion's saved JSON.
    fn read_estimate(path: &Path) -> Option<f64> {
        let data = std::fs::read_to_string(path.join("new").join("estimates.json")).ok()?;
        // Simple JSON parsing — find "median" -> "point_estimate"
        let median_pos = data.find("\"median\"")?;
        let after_median = &data[median_pos..];
        let pe_pos = after_median.find("\"point_estimate\"")?;
        let after_pe = &after_median[pe_pos + "\"point_estimate\"".len()..];
        let colon = after_pe.find(':')?;
        let after_colon = after_pe[colon + 1..].trim_start();
        let end = after_colon.find([',', '}'])?;
        after_colon[..end].trim().parse::<f64>().ok()
    }

    /// Scan criterion output dir and print colored summary.
    ///
    /// Criterion saves estimates as:
    ///   target/criterion/<group>/<allocator>/<param>/new/estimates.json   (with param)
    ///   target/criterion/<group>/<allocator>/new/estimates.json           (without param)
    pub fn print_summary() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join("criterion");
        if !base.exists() {
            return;
        }

        // Collect: group -> param -> allocator -> ns
        let mut groups: BTreeMap<String, BTreeMap<String, Vec<(String, f64)>>> = BTreeMap::new();

        let Ok(group_dirs) = std::fs::read_dir(&base) else {
            return;
        };
        for group_entry in group_dirs.flatten() {
            let group_name = group_entry.file_name().to_string_lossy().to_string();
            if group_name == "report" || !group_entry.path().is_dir() {
                continue;
            }

            let Ok(alloc_dirs) = std::fs::read_dir(group_entry.path()) else {
                continue;
            };
            for alloc_entry in alloc_dirs.flatten() {
                let alloc_name = alloc_entry.file_name().to_string_lossy().to_string();
                if alloc_name == "report" || !alloc_entry.path().is_dir() {
                    continue;
                }

                // Check if this dir has a "new/" subdir directly (no param)
                if alloc_entry
                    .path()
                    .join("new")
                    .join("estimates.json")
                    .exists()
                {
                    if let Some(ns) = read_estimate(&alloc_entry.path()) {
                        groups
                            .entry(group_name.clone())
                            .or_default()
                            .entry(String::new())
                            .or_default()
                            .push((alloc_name.clone(), ns));
                    }
                    continue;
                }

                // Otherwise, iterate param subdirs: <alloc>/<param>/new/estimates.json
                let Ok(param_dirs) = std::fs::read_dir(alloc_entry.path()) else {
                    continue;
                };
                for param_entry in param_dirs.flatten() {
                    let param_name = param_entry.file_name().to_string_lossy().to_string();
                    if param_name == "report" || !param_entry.path().is_dir() {
                        continue;
                    }

                    if let Some(ns) = read_estimate(&param_entry.path()) {
                        groups
                            .entry(group_name.clone())
                            .or_default()
                            .entry(param_name)
                            .or_default()
                            .push((alloc_name.clone(), ns));
                    }
                }
            }
        }

        if groups.is_empty() {
            return;
        }

        let bar_width = 30;

        println!();
        println!("  {BOLD}========== Benchmark Summary =========={RESET}");
        println!();
        print!("  Legend: ");
        print!("{WHITE}system{RESET}  ");
        print!("{GREEN}rstc_nightly{RESET}  ");
        print!("{BRIGHT_GREEN}rstc_percpu{RESET}  ");
        print!("{MAGENTA}rstc_std{RESET}  ");
        print!("{RED}rstc_nostd{RESET}  ");
        print!("{CYAN}mimalloc{RESET}  ");
        print!("{YELLOW}google_tc{RESET}  ");
        print!("{BRIGHT_BLUE}jemalloc{RESET}  ");
        print!("{BRIGHT_CYAN}snmalloc{RESET}  ");
        print!("{BRIGHT_YELLOW}rpmalloc{RESET}");
        println!();

        for (group, params) in &groups {
            println!();
            println!("  {BOLD}{group}{RESET}");

            for (param, results) in params {
                // Filter to known allocators and sort fastest first
                let mut results: Vec<_> = results
                    .iter()
                    .filter(|(name, _)| KNOWN.contains(&name.as_str()))
                    .collect();
                results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                if results.is_empty() {
                    continue;
                }

                let label = if param.is_empty() {
                    String::new()
                } else {
                    format!("  size={param}")
                };
                println!("  {DIM}---{label}{RESET}");

                let best = results
                    .iter()
                    .map(|(_, ns)| *ns)
                    .fold(f64::INFINITY, f64::min);
                let worst = results.iter().map(|(_, ns)| *ns).fold(0.0f64, f64::max);

                for (alloc, ns) in results {
                    let color = color_for(alloc);
                    let time = format_time(*ns);
                    let ratio = if worst > 0.0 { ns / worst } else { 1.0 };
                    let bar_len = ((ratio * bar_width as f64) as usize).max(1);
                    let bar = "\u{2588}".repeat(bar_len);
                    let pad = " ".repeat(bar_width - bar_len);

                    let tag = if (*ns - best).abs() < 0.01 {
                        format!(" {BG_GREEN} BEST {RESET}")
                    } else {
                        let slower = *ns / best;
                        format!(" {DIM}{slower:.2}x{RESET}")
                    };

                    println!("  {color}{alloc:>12}{RESET}  {time}  {color}{bar}{RESET}{pad}{tag}");
                }
            }
        }
        println!();
    }

    /// Hex colors for SVG plots.
    fn svg_color_for(name: &str) -> &'static str {
        match name {
            "system" => "#888888",       // gray
            "rstc_nightly" => "#2ca02c", // green
            "rstc_percpu" => "#98df8a",  // light green
            "rstc_std" => "#9467bd",     // purple
            "rstc_nostd" => "#d62728",   // red
            "mimalloc" => "#17becf",     // cyan
            "google_tc" => "#ff7f0e",    // orange
            "jemalloc" => "#1f77b4",     // blue
            "snmalloc" => "#e377c2",     // pink
            "rpmalloc" => "#bcbd22",     // olive
            _ => "#1f78b4",              // default blue
        }
    }

    /// Recolor criterion's violin SVGs so each allocator gets a distinct color.
    ///
    /// Violin SVGs have text labels like "group/allocator" at known y positions,
    /// followed by polygon pairs at those same y positions. We parse the labels
    /// to find allocator names, then replace fill colors on their polygons.
    pub fn recolor_svgs() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join("criterion");

        if !base.exists() {
            return;
        }

        // Find all violin.svg files
        fn visit(dir: &Path, svgs: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, svgs);
                } else if path.file_name().is_some_and(|n| n == "violin.svg") {
                    svgs.push(path);
                }
            }
        }

        let mut svgs = Vec::new();
        visit(&base, &mut svgs);

        for svg_path in &svgs {
            let Ok(content) = std::fs::read_to_string(svg_path) else {
                continue;
            };

            // Parse: find text elements that reference allocator names and their y positions.
            // Text elements look like: <text x="96" y="148" ...>group/allocator</text>
            // Each allocator has 2 polygons at that y center.
            //
            // Strategy: extract (allocator_name, y_value) pairs from labels,
            // then for each polygon, check which y-band it belongs to and recolor.

            let mut label_y: Vec<(String, f64)> = Vec::new();

            // Find labels: text elements containing known allocator names
            let mut pos = 0;
            while let Some(start) = content[pos..].find("<text ") {
                let abs_start = pos + start;
                let Some(end) = content[abs_start..].find("</text>") else {
                    break;
                };
                let tag = &content[abs_start..abs_start + end + 7];

                // Extract y attribute
                if let Some(y_start) = tag.find(" y=\"") {
                    let y_str = &tag[y_start + 4..];
                    if let Some(y_end) = y_str.find('"')
                        && let Ok(y) = y_str[..y_end].parse::<f64>()
                    {
                        // Extract text content (trim whitespace from multi-line SVG)
                        if let Some(gt) = tag.find('>') {
                            let text = tag[gt + 1..tag.len() - 7].trim();
                            // Labels: "group/alloc" or "group/alloc/param"
                            let parts: Vec<&str> = text.split('/').collect();
                            if parts.len() >= 2 {
                                let alloc_part = parts[1];
                                if KNOWN.contains(&alloc_part) {
                                    label_y.push((alloc_part.to_string(), y));
                                }
                            }
                        }
                    }
                }

                pos = abs_start + end + 7;
            }

            if label_y.is_empty() {
                continue;
            }

            // Now recolor polygons. Each polygon has a y-center that matches a label y.
            // Replace fill="#1F78B4" with the allocator's color based on y proximity.
            let mut result = String::with_capacity(content.len());
            let mut remaining = content.as_str();

            while let Some(poly_start) = remaining.find("<polygon ") {
                result.push_str(&remaining[..poly_start]);
                let poly_tag_end = remaining[poly_start..]
                    .find("/>")
                    .unwrap_or(remaining.len() - poly_start);
                let poly_tag = &remaining[poly_start..poly_start + poly_tag_end + 2];

                // Extract first y coordinate from points to determine which allocator
                let recolored = if let Some(pts_start) = poly_tag.find("points=\"") {
                    let pts = &poly_tag[pts_start + 8..];
                    // First point is like "656,148 ..."
                    let first_y = pts
                        .split_whitespace()
                        .next()
                        .and_then(|p| p.split(',').nth(1))
                        .and_then(|y| y.parse::<f64>().ok());

                    if let Some(y) = first_y {
                        // Find closest label
                        let closest = label_y
                            .iter()
                            .min_by(|a, b| (a.1 - y).abs().partial_cmp(&(b.1 - y).abs()).unwrap());

                        if let Some((alloc, _)) = closest {
                            let new_color = svg_color_for(alloc);
                            poly_tag
                                .replace("fill=\"#1F78B4\"", &format!("fill=\"{new_color}\""))
                                .replace("fill=\"#1f78b4\"", &format!("fill=\"{new_color}\""))
                        } else {
                            poly_tag.to_string()
                        }
                    } else {
                        poly_tag.to_string()
                    }
                } else {
                    poly_tag.to_string()
                };

                result.push_str(&recolored);
                remaining = &remaining[poly_start + poly_tag_end + 2..];
            }
            result.push_str(remaining);

            let _ = std::fs::write(svg_path, result);
        }
    }
}

// ---------------------------------------------------------------------------
// Custom main: run criterion, then print colored summary
// ---------------------------------------------------------------------------

fn main() {
    // Run criterion benchmarks (respects CLI args like --bench, filters, etc.)
    let mut criterion = Criterion::default().configure_from_args();
    bench_single_alloc_dealloc(&mut criterion);
    bench_batch_alloc_free(&mut criterion);
    bench_churn(&mut criterion);
    bench_vec_push(&mut criterion);
    bench_multithreaded(&mut criterion);
    bench_cross_thread_free(&mut criterion);
    bench_thread_scalability(&mut criterion);
    bench_mixed_sizes(&mut criterion);
    bench_producer_consumer(&mut criterion);

    // Recolor SVG plots so each allocator has a distinct color
    summary::recolor_svgs();

    // Print colored comparison table before criterion's final_summary (which may exit)
    summary::print_summary();
    use std::io::Write;
    let _ = std::io::stdout().flush();

    criterion.final_summary();
}

//! Shared allocator infrastructure for rtmalloc benchmarks.
//!
//! Provides FFI bindings, allocator statics, helpers, and the summary/SVG
//! recolor module used by the benchmark harness.
#![allow(unexpected_cfgs)]

use std::alloc::System;

#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;
#[cfg(feature = "rpmalloc")]
use rpmalloc::RpMalloc;
#[cfg(feature = "snmalloc")]
use snmalloc_rs::SnMalloc;
#[cfg(all(has_jemalloc, feature = "jemalloc"))]
use tikv_jemallocator::Jemalloc;

// ---------------------------------------------------------------------------
// rtmalloc FFI (statically linked, built by build.rs with --profile fast)
// ---------------------------------------------------------------------------

mod rtmalloc_ffi {
    use std::alloc::{GlobalAlloc, Layout};

    unsafe extern "C" {
        // Nightly variant (#[thread_local] thread cache)
        fn rtmalloc_nightly_alloc(size: usize, align: usize) -> *mut u8;
        fn rtmalloc_nightly_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rtmalloc_nightly_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;

        // Std variant (std::thread_local! thread cache)
        fn rtmalloc_std_alloc(size: usize, align: usize) -> *mut u8;
        fn rtmalloc_std_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rtmalloc_std_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;

        // Nostd variant (central cache only, no thread cache)
        fn rtmalloc_nostd_alloc(size: usize, align: usize) -> *mut u8;
        fn rtmalloc_nostd_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rtmalloc_nostd_realloc(
            ptr: *mut u8,
            size: usize,
            align: usize,
            new_size: usize,
        ) -> *mut u8;
    }

    // Per-CPU variant (rseq, Linux x86_64 only)
    #[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
    unsafe extern "C" {
        fn rtmalloc_percpu_alloc(size: usize, align: usize) -> *mut u8;
        fn rtmalloc_percpu_dealloc(ptr: *mut u8, size: usize, align: usize);
        fn rtmalloc_percpu_realloc(
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
        RtmallocNightly,
        rtmalloc_nightly_alloc,
        rtmalloc_nightly_dealloc,
        rtmalloc_nightly_realloc
    );
    impl_ffi_alloc!(
        RtmallocStd,
        rtmalloc_std_alloc,
        rtmalloc_std_dealloc,
        rtmalloc_std_realloc
    );
    impl_ffi_alloc!(
        RtmallocNostd,
        rtmalloc_nostd_alloc,
        rtmalloc_nostd_dealloc,
        rtmalloc_nostd_realloc
    );
    #[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
    impl_ffi_alloc!(
        RtmallocPercpu,
        rtmalloc_percpu_alloc,
        rtmalloc_percpu_dealloc,
        rtmalloc_percpu_realloc
    );
}

#[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
use rtmalloc_ffi::RtmallocPercpu;
use rtmalloc_ffi::{RtmallocNightly, RtmallocNostd, RtmallocStd};

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

// ---------------------------------------------------------------------------
// Allocator statics
// ---------------------------------------------------------------------------

#[cfg(feature = "system")]
pub static SYSTEM: System = System;
pub static RTMALLOC_NIGHTLY: RtmallocNightly = RtmallocNightly;
pub static RTMALLOC_STD: RtmallocStd = RtmallocStd;
pub static RTMALLOC_NOSTD: RtmallocNostd = RtmallocNostd;
#[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
pub static RTMALLOC_PERCPU: RtmallocPercpu = RtmallocPercpu;
#[cfg(feature = "mimalloc")]
pub static MIMALLOC: MiMalloc = MiMalloc;
#[cfg(feature = "snmalloc")]
pub static SNMALLOC: SnMalloc = SnMalloc;
#[cfg(feature = "rpmalloc")]
pub static RPMALLOC: RpMalloc = RpMalloc;
#[cfg(all(has_jemalloc, feature = "jemalloc"))]
pub static JEMALLOC: Jemalloc = Jemalloc;
#[cfg(has_google_tcmalloc)]
pub static GOOGLE_TC: GoogleTcMalloc = GoogleTcMalloc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrapper to send raw pointers across threads in benchmarks.
/// Safety: the benchmarks ensure each pointer is only used by one thread at a time.
pub struct SendPtr(pub *mut u8);
unsafe impl Send for SendPtr {}

// ---------------------------------------------------------------------------
// Macro to reduce repetitive per-allocator benchmark registration
// ---------------------------------------------------------------------------

/// Register a benchmark closure for every available allocator.
///
/// The closure receives `(b: &mut criterion::Bencher, alloc: &dyn GlobalAlloc)`.
#[macro_export]
macro_rules! bench_all_allocators {
    ($group:expr, $suffix:expr, |$b:ident, $alloc:ident| $body:expr) => {{
        #[cfg(feature = "system")]
        $group.bench_function(
            format!("system{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::SYSTEM;
                $body
            },
        );
        $group.bench_function(
            format!("rt_nightly{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::RTMALLOC_NIGHTLY;
                $body
            },
        );
        #[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
        $group.bench_function(
            format!("rt_percpu{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::RTMALLOC_PERCPU;
                $body
            },
        );
        $group.bench_function(
            format!("rt_std{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::RTMALLOC_STD;
                $body
            },
        );
        $group.bench_function(
            format!("rt_nostd{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::RTMALLOC_NOSTD;
                $body
            },
        );
        #[cfg(feature = "mimalloc")]
        $group.bench_function(
            format!("mimalloc{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::MIMALLOC;
                $body
            },
        );
        #[cfg(has_google_tcmalloc)]
        $group.bench_function(
            format!("google_tc{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::GOOGLE_TC;
                $body
            },
        );
        #[cfg(feature = "snmalloc")]
        $group.bench_function(
            format!("snmalloc{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::SNMALLOC;
                $body
            },
        );
        #[cfg(feature = "rpmalloc")]
        $group.bench_function(
            format!("rpmalloc{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::RPMALLOC;
                $body
            },
        );
        #[cfg(all(has_jemalloc, feature = "jemalloc"))]
        $group.bench_function(
            format!("jemalloc{}", $suffix),
            |$b: &mut criterion::Bencher| {
                let $alloc = &$crate::JEMALLOC;
                $body
            },
        );
    }};
}

/// Register a parameterised benchmark for every available allocator.
///
/// The closure receives `(b: &mut criterion::Bencher, alloc: &dyn GlobalAlloc)`.
#[macro_export]
macro_rules! bench_all_allocators_param {
    ($group:expr, $param:expr, |$b:ident, $alloc:ident| $body:expr) => {{
        #[cfg(feature = "system")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("system", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::SYSTEM;
                $body
            },
        );
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_nightly", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_NIGHTLY;
                $body
            },
        );
        #[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_percpu", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_PERCPU;
                $body
            },
        );
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_std", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_STD;
                $body
            },
        );
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_nostd", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_NOSTD;
                $body
            },
        );
        #[cfg(feature = "mimalloc")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("mimalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::MIMALLOC;
                $body
            },
        );
        #[cfg(has_google_tcmalloc)]
        $group.bench_with_input(
            criterion::BenchmarkId::new("google_tc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::GOOGLE_TC;
                $body
            },
        );
        #[cfg(feature = "snmalloc")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("snmalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::SNMALLOC;
                $body
            },
        );
        #[cfg(feature = "rpmalloc")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("rpmalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RPMALLOC;
                $body
            },
        );
        #[cfg(all(has_jemalloc, feature = "jemalloc"))]
        $group.bench_with_input(
            criterion::BenchmarkId::new("jemalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::JEMALLOC;
                $body
            },
        );
    }};
}

/// Like `bench_all_allocators_param!` but the closure receives the static allocator
/// reference with its concrete type (for `Sync + 'static` bounds needed by thread spawning).
///
/// The closure body can use `alloc` which is `&'static impl GlobalAlloc + Sync`.
#[macro_export]
macro_rules! bench_all_allocators_static {
    ($group:expr, $param:expr, |$b:ident, $alloc:ident| $body:expr) => {{
        #[cfg(feature = "system")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("system", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::SYSTEM;
                $body
            },
        );
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_nightly", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_NIGHTLY;
                $body
            },
        );
        #[cfg(all(has_rtmalloc_percpu, not(feature = "callgrind")))]
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_percpu", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_PERCPU;
                $body
            },
        );
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_std", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_STD;
                $body
            },
        );
        $group.bench_with_input(
            criterion::BenchmarkId::new("rt_nostd", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RTMALLOC_NOSTD;
                $body
            },
        );
        #[cfg(feature = "mimalloc")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("mimalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::MIMALLOC;
                $body
            },
        );
        #[cfg(has_google_tcmalloc)]
        $group.bench_with_input(
            criterion::BenchmarkId::new("google_tc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::GOOGLE_TC;
                $body
            },
        );
        #[cfg(feature = "snmalloc")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("snmalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::SNMALLOC;
                $body
            },
        );
        #[cfg(feature = "rpmalloc")]
        $group.bench_with_input(
            criterion::BenchmarkId::new("rpmalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::RPMALLOC;
                $body
            },
        );
        #[cfg(all(has_jemalloc, feature = "jemalloc"))]
        $group.bench_with_input(
            criterion::BenchmarkId::new("jemalloc", $param),
            &$param,
            |$b: &mut criterion::Bencher, _| {
                let $alloc = &$crate::JEMALLOC;
                $body
            },
        );
    }};
}

// ---------------------------------------------------------------------------
// Summary module
// ---------------------------------------------------------------------------

pub mod summary {
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
        "rt_nightly",
        "rt_percpu",
        "rt_std",
        "rt_nostd",
        "mimalloc",
        "google_tc",
        "jemalloc",
        "snmalloc",
        "rpmalloc",
    ];

    fn color_for(name: &str) -> &'static str {
        match name {
            "system" => WHITE,
            "rt_nightly" => GREEN,
            "rt_percpu" => BRIGHT_GREEN,
            "rt_std" => MAGENTA,
            "rt_nostd" => RED,
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
        print!("{GREEN}rt_nightly{RESET}  ");
        print!("{BRIGHT_GREEN}rt_percpu{RESET}  ");
        print!("{MAGENTA}rt_std{RESET}  ");
        print!("{RED}rt_nostd{RESET}  ");
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
            "system" => "#888888",     // gray
            "rt_nightly" => "#2ca02c", // green
            "rt_percpu" => "#98df8a",  // light green
            "rt_std" => "#9467bd",     // purple
            "rt_nostd" => "#d62728",   // red
            "mimalloc" => "#17becf",   // cyan
            "google_tc" => "#ff7f0e",  // orange
            "jemalloc" => "#1f77b4",   // blue
            "snmalloc" => "#e377c2",   // pink
            "rpmalloc" => "#bcbd22",   // olive
            _ => "#1f78b4",            // default blue
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

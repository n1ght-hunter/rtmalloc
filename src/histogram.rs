//! Allocation size histogram.
//!
//! Records the distribution of allocation sizes in 8-byte buckets up to
//! [`MAX_TRACKED`] bytes. Use [`print_report`] to display results and
//! [`optimal_layout`] to derive custom size class configurations.

extern crate std;

use core::sync::atomic::{AtomicU64, Ordering};
use std::format;
use std::println;
use std::string::String;
use std::vec::Vec;

/// Maximum allocation size tracked in a bucket (inclusive).
pub const MAX_TRACKED: usize = 4096;
/// Width of each bucket in bytes.
pub const BUCKET_SIZE: usize = 8;
/// Number of buckets: sizes 1–8 → bucket 0, 9–16 → bucket 1, …, 4089–4096 → bucket 511.
pub const NUM_BUCKETS: usize = MAX_TRACKED / BUCKET_SIZE; // 512

struct BucketArray([AtomicU64; NUM_BUCKETS]);
// SAFETY: AtomicU64 is Sync.
unsafe impl Sync for BucketArray {}

static BUCKETS: BucketArray = {
    const ZERO: AtomicU64 = AtomicU64::new(0);
    BucketArray([ZERO; NUM_BUCKETS])
};
static OVERFLOW: AtomicU64 = AtomicU64::new(0);

/// Record one allocation of `size` bytes.
///
/// Called from the `hist_record!` macro. Safe to call from the allocator
/// hot path — only does an atomic increment, no allocation.
#[inline]
pub fn record(size: usize) {
    if size == 0 {
        return;
    }
    if size > MAX_TRACKED {
        OVERFLOW.fetch_add(1, Ordering::Relaxed);
    } else {
        let idx = (size - 1) / BUCKET_SIZE;
        BUCKETS.0[idx].fetch_add(1, Ordering::Relaxed);
    }
}

/// A point-in-time snapshot of the histogram counters.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// `counts[i]` = number of allocations whose size falls in `(i*8, (i+1)*8]`.
    /// Class upper bound for bucket `i` is `(i+1) * BUCKET_SIZE`.
    pub counts: [u64; NUM_BUCKETS],
    /// Allocations with size > [`MAX_TRACKED`].
    pub overflow: u64,
}

/// Load all counters and return a [`Snapshot`].
pub fn snapshot() -> Snapshot {
    let mut counts = [0u64; NUM_BUCKETS];
    for (i, bucket) in BUCKETS.0.iter().enumerate() {
        counts[i] = bucket.load(Ordering::Relaxed);
    }
    Snapshot {
        counts,
        overflow: OVERFLOW.load(Ordering::Relaxed),
    }
}

/// Return the smallest set of size class upper bounds (in bytes, sorted ascending)
/// whose combined allocation count is at least `coverage` fraction of all tracked
/// allocations (overflow excluded).
///
/// Algorithm: sort buckets by count descending, greedily take sizes until the
/// cumulative count / total >= `coverage`, then sort the result ascending.
///
/// `coverage` should be in `0.0..=1.0`. Values >= 1.0 return all non-empty sizes.
pub fn suggest_classes(snap: &Snapshot, coverage: f64) -> Vec<usize> {
    let total: u64 = snap.counts.iter().sum();
    if total == 0 {
        return Vec::new();
    }
    let target = ((total as f64) * coverage.clamp(0.0, 1.0)) as u64;

    let mut pairs: Vec<(usize, u64)> = snap
        .counts
        .iter()
        .enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(i, c)| ((i + 1) * BUCKET_SIZE, *c))
        .collect();
    pairs.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    let mut sizes = Vec::new();
    let mut covered = 0u64;
    for (size, count) in pairs {
        sizes.push(size);
        covered += count;
        if covered >= target {
            break;
        }
    }
    sizes.sort_unstable();
    sizes
}

/// The result of [`optimal_layout`].
pub struct ClassLayout {
    /// Size class upper bounds in bytes, sorted ascending.
    ///
    /// An allocation of size `s` should use the smallest class `c` where `c >= s`.
    pub classes: Vec<usize>,
    /// Average wasted bytes per allocation under this class layout.
    pub avg_waste_bytes: f64,
    /// Total internal fragmentation as a fraction of total allocated bytes (0.0–1.0).
    pub fragmentation_ratio: f64,
}

/// Compute an optimal set of size class boundaries for the observed distribution.
///
/// Starts with one class per observed size (zero waste), then greedily merges
/// adjacent class ranges in order of cheapest additional waste, stopping when:
/// - `classes.len() <= max_classes`, AND
/// - no remaining merge keeps every class's waste ratio below `max_waste_pct`.
///
/// `max_waste_pct` is checked per-merge: if performing a merge would push the
/// merged class's waste ratio (avg_waste / class_size) above `max_waste_pct`,
/// that merge is skipped. If ALL remaining merges violate this, merging stops
/// even if `max_classes` is not yet reached.
///
/// Waste is estimated conservatively: for a bucket of width [`BUCKET_SIZE`],
/// the assumed allocation size is the bucket's lower bound + 1 byte (worst case).
pub fn optimal_layout(snap: &Snapshot, max_classes: usize, max_waste_pct: f64) -> ClassLayout {
    // Collect non-empty (class_size, count, waste) triples, sorted by class size.
    let mut ranges: Vec<(usize, u64, u64)> = snap
        .counts
        .iter()
        .enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(i, c)| {
            let c = *c;
            let class_size = (i + 1) * BUCKET_SIZE;
            // Conservative: assume alloc size = lower bound of bucket + 1.
            let assumed_alloc_size = i * BUCKET_SIZE + 1;
            let waste_per_alloc = class_size - assumed_alloc_size;
            (class_size, c, c * waste_per_alloc as u64)
        })
        .collect();

    if ranges.is_empty() {
        return ClassLayout {
            classes: Vec::new(),
            avg_waste_bytes: 0.0,
            fragmentation_ratio: 0.0,
        };
    }

    // Greedy merge loop.
    loop {
        if ranges.len() <= max_classes {
            break;
        }

        // Find the adjacent pair whose merge adds the least waste.
        let best = (0..ranges.len() - 1).min_by_key(|&i| {
            ranges[i].1 * (ranges[i + 1].0 - ranges[i].0) as u64
        });

        let i = match best {
            Some(i) => i,
            None => break,
        };

        // Check waste ratio constraint for the merged range.
        let (right_class, right_count, right_waste) = ranges[i + 1];
        let (_, left_count, left_waste) = ranges[i];
        let added_waste = left_count * (right_class - ranges[i].0) as u64;
        let merged_waste = left_waste + added_waste + right_waste;
        let merged_count = left_count + right_count;
        let merged_waste_ratio =
            merged_waste as f64 / (merged_count as f64 * right_class as f64);

        if merged_waste_ratio > max_waste_pct {
            break;
        }

        ranges[i] = (right_class, merged_count, merged_waste);
        ranges.remove(i + 1);
    }

    // Compute summary stats.
    let total_count: u64 = ranges.iter().map(|(_, c, _)| *c).sum();
    let total_waste: u64 = ranges.iter().map(|(_, _, w)| *w).sum();
    let total_alloc_bytes: u64 = ranges
        .iter()
        .map(|&(sz, c, _)| (sz as u64) * c)
        .sum();

    let avg_waste_bytes = if total_count > 0 {
        total_waste as f64 / total_count as f64
    } else {
        0.0
    };
    let fragmentation_ratio = if total_alloc_bytes > 0 {
        total_waste as f64 / total_alloc_bytes as f64
    } else {
        0.0
    };

    ClassLayout {
        classes: ranges.iter().map(|(sz, _, _)| *sz).collect(),
        avg_waste_bytes,
        fragmentation_ratio,
    }
}

impl ClassLayout {
    /// Format this layout as a TOML config string suitable for `RTMALLOC_CLASSES`.
    ///
    /// Uses the simple `classes = [...]` format so build.rs auto-tunes
    /// pages and batch_size for each class.
    pub fn to_toml(&self) -> String {
        let sizes: Vec<String> = self.classes.iter().map(|s| format!("{}", s)).collect();
        format!("classes = [{}]\n", sizes.join(", "))
    }
}

/// Take a snapshot, compute an optimal layout, and return it as a TOML string
/// ready to be written to a file and used with `RTMALLOC_CLASSES`.
///
/// `max_classes` and `max_waste_pct` are forwarded to [`optimal_layout`].
pub fn export_toml(max_classes: usize, max_waste_pct: f64) -> String {
    let snap = snapshot();
    let layout = optimal_layout(&snap, max_classes, max_waste_pct);
    layout.to_toml()
}

/// Print a human-readable histogram report to stdout.
///
/// Shows all non-zero buckets with count, percentage, and cumulative percentage.
/// Appends the output of `optimal_layout(&snap, 64, 0.125)` at the end.
pub fn print_report() {
    let snap = snapshot();
    let total: u64 = snap.counts.iter().sum::<u64>() + snap.overflow;

    println!(
        "\nAllocation size histogram (8-byte buckets, max tracked: {} bytes)",
        MAX_TRACKED
    );
    println!(
        "Total tracked: {}   Overflow (>{} bytes): {} ({:.2}%)\n",
        total,
        MAX_TRACKED,
        snap.overflow,
        if total > 0 {
            snap.overflow as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    );

    if total == 0 {
        println!("  (no allocations recorded)");
        return;
    }

    println!(
        "  {:>6}   {:>12}   {:>7}   {:>10}",
        "Size", "Count", "%", "Cumulative"
    );
    println!(
        "  {:->6}   {:->12}   {:->7}   {:->10}",
        "", "", "", ""
    );

    let mut cumulative = 0u64;
    for (i, &count) in snap.counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let size = (i + 1) * BUCKET_SIZE;
        cumulative += count;
        println!(
            "  {:>6}   {:>12}   {:>6.2}%   {:>9.2}%",
            size,
            count,
            count as f64 / total as f64 * 100.0,
            cumulative as f64 / total as f64 * 100.0,
        );
    }

    let layout = optimal_layout(&snap, 64, 0.125);
    println!("\nSuggested class layout (max 64 classes, max waste 12.5%):");
    if layout.classes.is_empty() {
        println!("  (insufficient data)");
    } else {
        println!("  {:?}", layout.classes);
        println!(
            "  Avg waste: {:.1} bytes/alloc   Fragmentation: {:.2}%",
            layout.avg_waste_bytes,
            layout.fragmentation_ratio * 100.0
        );
        println!("\nTOML config (save to a file, build with RTMALLOC_CLASSES=<path>):");
        println!("{}", layout.to_toml());
    }
}

//! Integration tests for the alloc-histogram feature.
//!
//! Run with: cargo test --features alloc-histogram,nightly --test histogram

#![cfg(feature = "alloc-histogram")]
#![feature(allocator_api)]

use rtmalloc::RtMalloc;
use rtmalloc::histogram::{self, MAX_TRACKED, NUM_BUCKETS};

#[test]
fn test_snapshot_accessible() {
    let snap = histogram::snapshot();
    let _ = snap.counts;
    let _ = snap.overflow;
}

#[test]
fn test_record_small_lands_in_correct_bucket() {
    let before = histogram::snapshot();
    histogram::record(8);
    histogram::record(16);
    let after = histogram::snapshot();
    assert!(
        after.counts[0] > before.counts[0],
        "bucket 0 (sizes 1-8) should increment"
    );
    assert!(
        after.counts[1] > before.counts[1],
        "bucket 1 (sizes 9-16) should increment"
    );
}

#[test]
fn test_record_overflow() {
    let before = histogram::snapshot();
    histogram::record(MAX_TRACKED + 1);
    let after = histogram::snapshot();
    assert!(after.overflow > before.overflow);
}

#[test]
fn test_record_zero_is_noop() {
    let before = histogram::snapshot();
    histogram::record(0);
    let after = histogram::snapshot();
    assert_eq!(
        before.counts.iter().sum::<u64>() + before.overflow,
        after.counts.iter().sum::<u64>() + after.overflow,
    );
}

#[test]
fn test_bucket_boundary_sizes() {
    let before = histogram::snapshot();
    histogram::record(1);
    histogram::record(9);
    histogram::record(MAX_TRACKED);
    let after = histogram::snapshot();
    assert!(after.counts[0] > before.counts[0]);
    assert!(after.counts[1] > before.counts[1]);
    assert!(after.counts[NUM_BUCKETS - 1] > before.counts[NUM_BUCKETS - 1]);
}

// --- suggest_classes ---

#[test]
fn test_suggest_classes_empty() {
    let snap = histogram::Snapshot {
        counts: [0; NUM_BUCKETS],
        overflow: 0,
    };
    let classes = histogram::suggest_classes(&snap, 0.99);
    assert!(classes.is_empty());
}

#[test]
fn test_suggest_classes_single_dominant_size() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[1] = 1000;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let classes = histogram::suggest_classes(&snap, 0.99);
    assert_eq!(classes, vec![16]);
}

#[test]
fn test_suggest_classes_covers_target_fraction() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[0] = 600;
    counts[1] = 300;
    counts[2] = 100;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };

    let classes_90 = histogram::suggest_classes(&snap, 0.90);
    assert!(classes_90.contains(&8));
    assert!(classes_90.contains(&16));
    assert!(!classes_90.contains(&24));

    let classes_100 = histogram::suggest_classes(&snap, 1.0);
    assert_eq!(classes_100.len(), 3);
}

#[test]
fn test_suggest_classes_is_sorted_ascending() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[3] = 500;
    counts[0] = 300;
    counts[7] = 200;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let classes = histogram::suggest_classes(&snap, 1.0);
    for w in classes.windows(2) {
        assert!(w[0] < w[1], "classes must be sorted ascending");
    }
}

// --- optimal_layout ---

#[test]
fn test_optimal_layout_empty() {
    let snap = histogram::Snapshot {
        counts: [0; NUM_BUCKETS],
        overflow: 0,
    };
    let layout = histogram::optimal_layout(&snap, 64, 0.125);
    assert!(layout.classes.is_empty());
    assert_eq!(layout.avg_waste_bytes, 0.0);
    assert_eq!(layout.fragmentation_ratio, 0.0);
}

#[test]
fn test_optimal_layout_single_size() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[1] = 1000;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let layout = histogram::optimal_layout(&snap, 64, 0.125);
    assert_eq!(layout.classes, vec![16]);
}

#[test]
fn test_optimal_layout_respects_max_classes() {
    let mut counts = [0u64; NUM_BUCKETS];
    for i in 0..10 {
        counts[i] = 100;
    }
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let layout = histogram::optimal_layout(&snap, 5, 1.0);
    assert!(
        layout.classes.len() <= 5,
        "got {} classes, expected <= 5",
        layout.classes.len()
    );
}

#[test]
fn test_optimal_layout_respects_max_waste_pct() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[0] = 1000;
    counts[NUM_BUCKETS - 1] = 1000;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let layout = histogram::optimal_layout(&snap, 1, 0.125);
    assert_eq!(
        layout.classes.len(),
        2,
        "waste guard should prevent merging 8 and 4096"
    );
}

#[test]
fn test_optimal_layout_classes_sorted_ascending() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[0] = 500;
    counts[2] = 300;
    counts[5] = 200;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let layout = histogram::optimal_layout(&snap, 64, 0.125);
    for w in layout.classes.windows(2) {
        assert!(w[0] < w[1]);
    }
}

#[test]
fn test_optimal_layout_stats_consistent() {
    let mut counts = [0u64; NUM_BUCKETS];
    counts[0] = 400;
    counts[1] = 600;
    let snap = histogram::Snapshot {
        counts,
        overflow: 0,
    };
    let layout = histogram::optimal_layout(&snap, 64, 0.125);
    assert!(layout.avg_waste_bytes >= 0.0);
    assert!(layout.fragmentation_ratio >= 0.0);
    assert!(layout.fragmentation_ratio <= 1.0);
    assert_eq!(layout.classes, vec![8, 16]);
}

// --- print_report ---

#[test]
fn test_print_report_does_not_panic() {
    histogram::record(8);
    histogram::record(16);
    histogram::record(16);
    histogram::record(5000);
    histogram::print_report();
}

// --- real allocations ---

#[test]
fn test_real_allocations_are_recorded() {
    let before = histogram::snapshot();
    let _a = Box::new_in([0u8; 8], RtMalloc);
    let _b = Box::new_in([0u8; 16], RtMalloc);
    let _c = Box::new_in([0u8; 32], RtMalloc);
    let after = histogram::snapshot();
    let delta: u64 = after
        .counts
        .iter()
        .zip(before.counts.iter())
        .map(|(a, b)| a - b)
        .sum::<u64>()
        + (after.overflow - before.overflow);
    assert!(
        delta >= 3,
        "expected at least 3 recorded allocations, got {}",
        delta
    );
}

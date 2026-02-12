//! Size class table and lookup functions for tcmalloc.
//!
//! Objects are bucketed into size classes to reduce fragmentation and enable
//! free list management. The table covers sizes from 8 bytes up to 256 KiB.

/// Information about a single size class.
#[derive(Clone, Copy)]
pub struct SizeClassInfo {
    /// Allocation size for this class (bytes). All allocations in this class
    /// are rounded up to this size.
    pub size: usize,
    /// Number of pages per span for this class.
    pub pages: usize,
    /// Number of objects to transfer between thread cache and central cache at once.
    pub batch_size: usize,
}

impl SizeClassInfo {
    pub const fn objects_per_span(&self) -> usize {
        (self.pages * PAGE_SIZE) / self.size
    }
}

use crate::PAGE_SIZE;

/// Number of defined size classes (index 0 is unused/sentinel).
pub const NUM_SIZE_CLASSES: usize = 46;

/// Maximum allocation size that goes through size classes.
/// Anything larger is a "large" allocation handled directly by the page heap.
pub const MAX_SMALL_SIZE: usize = 262144; // 256 KiB

/// The size class table. Index 0 is a sentinel (unused).
/// Classes 1..=45 cover sizes from 8 bytes to 256 KiB.
pub static SIZE_CLASSES: [SizeClassInfo; NUM_SIZE_CLASSES] = [
    // Class 0: sentinel (unused)
    SizeClassInfo {
        size: 0,
        pages: 0,
        batch_size: 0,
    },
    // Class 1-8: 8-byte increments (8 to 64)
    SizeClassInfo {
        size: 8,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 16,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 24,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 32,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 40,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 48,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 56,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 64,
        pages: 1,
        batch_size: 32,
    },
    // Class 9-12: 16-byte increments (80 to 128)
    SizeClassInfo {
        size: 80,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 96,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 112,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 128,
        pages: 1,
        batch_size: 32,
    },
    // Class 13-16: 32-byte increments (160 to 256)
    SizeClassInfo {
        size: 160,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 192,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 224,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 256,
        pages: 1,
        batch_size: 32,
    },
    // Class 17-20: 64-byte increments (320 to 512)
    // batch = min(65536/size, 32) per gperftools formula
    SizeClassInfo {
        size: 320,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 384,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 448,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 512,
        pages: 1,
        batch_size: 32,
    },
    // Class 21-24: 128-byte increments (640 to 1024)
    SizeClassInfo {
        size: 640,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 768,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 896,
        pages: 1,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 1024,
        pages: 1,
        batch_size: 32,
    },
    // Class 25-28: 256-byte increments (1280 to 2048)
    // gperftools: pages=2, batch=32 for all of these
    SizeClassInfo {
        size: 1280,
        pages: 2,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 1536,
        pages: 2,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 1792,
        pages: 2,
        batch_size: 32,
    },
    SizeClassInfo {
        size: 2048,
        pages: 2,
        batch_size: 32,
    },
    // Class 29-32: 512-byte increments (2560 to 4096)
    // batch = min(65536/size, 32); pages sized for >=8 obj/span
    // (gperftools uses fewer pages but has transfer cache; we compensate)
    SizeClassInfo {
        size: 2560,
        pages: 4,
        batch_size: 25,
    },
    SizeClassInfo {
        size: 3072,
        pages: 4,
        batch_size: 21,
    },
    SizeClassInfo {
        size: 3584,
        pages: 4,
        batch_size: 18,
    },
    SizeClassInfo {
        size: 4096,
        pages: 4,
        batch_size: 16,
    },
    // Class 33-36: 1024-byte increments (5120 to 8192)
    SizeClassInfo {
        size: 5120,
        pages: 5,
        batch_size: 12,
    },
    SizeClassInfo {
        size: 6144,
        pages: 6,
        batch_size: 10,
    },
    SizeClassInfo {
        size: 7168,
        pages: 7,
        batch_size: 9,
    },
    SizeClassInfo {
        size: 8192,
        pages: 8,
        batch_size: 8,
    },
    // Class 37-40: larger sizes
    SizeClassInfo {
        size: 10240,
        pages: 10,
        batch_size: 6,
    },
    SizeClassInfo {
        size: 12288,
        pages: 12,
        batch_size: 5,
    },
    SizeClassInfo {
        size: 16384,
        pages: 16,
        batch_size: 4,
    },
    SizeClassInfo {
        size: 20480,
        pages: 20,
        batch_size: 3,
    },
    // Class 41-45: large size classes
    SizeClassInfo {
        size: 32768,
        pages: 16,
        batch_size: 2,
    },
    SizeClassInfo {
        size: 40960,
        pages: 20,
        batch_size: 2,
    },
    SizeClassInfo {
        size: 65536,
        pages: 32,
        batch_size: 2,
    },
    SizeClassInfo {
        size: 131072,
        pages: 32,
        batch_size: 2,
    },
    SizeClassInfo {
        size: 262144,
        pages: 64,
        batch_size: 2,
    },
];

/// Lookup table for small sizes (<= 1024 bytes).
/// Index = (size + 7) / 8, value = size class index.
/// Covers sizes 0..=1024 in 8-byte steps (129 entries).
const SMALL_LOOKUP_LEN: usize = 129; // ceil(1024/8) + 1

static SMALL_LOOKUP: [u8; SMALL_LOOKUP_LEN] = const {
    let mut table = [0u8; SMALL_LOOKUP_LEN];
    let mut i = 0;
    while i < SMALL_LOOKUP_LEN {
        let size = if i == 0 { 0 } else { i * 8 };
        // Find the smallest size class that fits this size
        let mut cls = 1u8;
        while (cls as usize) < NUM_SIZE_CLASSES {
            if SIZE_CLASSES[cls as usize].size >= size {
                break;
            }
            cls += 1;
        }
        if (cls as usize) >= NUM_SIZE_CLASSES {
            cls = (NUM_SIZE_CLASSES - 1) as u8;
        }
        table[i] = cls;
        i += 1;
    }
    table
};

/// Map an allocation size to its size class index.
/// Returns 0 if size is 0 (callers should handle this).
/// Returns a class in 1..NUM_SIZE_CLASSES-1 for valid sizes.
/// For sizes > MAX_SMALL_SIZE, returns 0 (indicating large allocation).
#[inline]
pub fn size_to_class(size: usize) -> usize {
    if size == 0 {
        return 1; // Minimum allocation is 8 bytes
    }
    if size > MAX_SMALL_SIZE {
        return 0; // Large allocation
    }
    if size <= 1024 {
        let idx = size.div_ceil(8);
        return SMALL_LOOKUP[idx] as usize;
    }
    // For sizes > 1024, do a linear scan of the upper classes.
    // There are only ~20 classes above 1024, so this is fast enough.
    let mut cls = 25; // First class with size > 1024
    while cls < NUM_SIZE_CLASSES {
        if SIZE_CLASSES[cls].size >= size {
            return cls;
        }
        cls += 1;
    }
    0 // Too large for size classes
}

/// Get the allocation size for a given size class.
#[inline]
pub fn class_to_size(cls: usize) -> usize {
    SIZE_CLASSES[cls].size
}

/// Get the size class info for a given class index.
#[inline]
pub fn class_info(cls: usize) -> &'static SizeClassInfo {
    &SIZE_CLASSES[cls]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_to_class_zero() {
        let cls = size_to_class(0);
        assert_eq!(cls, 1);
        assert_eq!(class_to_size(cls), 8);
    }

    #[test]
    fn test_size_to_class_exact() {
        assert_eq!(class_to_size(size_to_class(8)), 8);
        assert_eq!(class_to_size(size_to_class(16)), 16);
        assert_eq!(class_to_size(size_to_class(64)), 64);
        assert_eq!(class_to_size(size_to_class(128)), 128);
        assert_eq!(class_to_size(size_to_class(256)), 256);
        assert_eq!(class_to_size(size_to_class(512)), 512);
        assert_eq!(class_to_size(size_to_class(1024)), 1024);
        assert_eq!(class_to_size(size_to_class(2048)), 2048);
        assert_eq!(class_to_size(size_to_class(4096)), 4096);
        assert_eq!(class_to_size(size_to_class(8192)), 8192);
        assert_eq!(class_to_size(size_to_class(262144)), 262144);
    }

    #[test]
    fn test_size_to_class_rounds_up() {
        assert_eq!(class_to_size(size_to_class(1)), 8);
        assert_eq!(class_to_size(size_to_class(7)), 8);
        assert_eq!(class_to_size(size_to_class(9)), 16);
        assert_eq!(class_to_size(size_to_class(15)), 16);
        assert_eq!(class_to_size(size_to_class(17)), 24);
        assert_eq!(class_to_size(size_to_class(65)), 80);
        assert_eq!(class_to_size(size_to_class(129)), 160);
        assert_eq!(class_to_size(size_to_class(257)), 320);
        assert_eq!(class_to_size(size_to_class(1025)), 1280);
    }

    #[test]
    fn test_size_to_class_large() {
        assert_eq!(size_to_class(262145), 0);
        assert_eq!(size_to_class(1_000_000), 0);
    }

    #[test]
    fn test_round_trip_all_classes() {
        for cls in 1..NUM_SIZE_CLASSES {
            let size = class_to_size(cls);
            assert!(size > 0, "class {} has zero size", cls);
            let found = size_to_class(size);
            assert_eq!(
                found, cls,
                "round-trip failed for class {} (size {})",
                cls, size
            );
        }
    }

    #[test]
    fn test_classes_monotonically_increasing() {
        for i in 2..NUM_SIZE_CLASSES {
            assert!(
                SIZE_CLASSES[i].size > SIZE_CLASSES[i - 1].size,
                "class {} size {} not greater than class {} size {}",
                i,
                SIZE_CLASSES[i].size,
                i - 1,
                SIZE_CLASSES[i - 1].size
            );
        }
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_all_sizes_8_aligned() {
        for cls in 1..NUM_SIZE_CLASSES {
            assert_eq!(
                SIZE_CLASSES[cls].size % 8,
                0,
                "class {} size {} not 8-aligned",
                cls,
                SIZE_CLASSES[cls].size
            );
        }
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_objects_per_span() {
        for cls in 1..NUM_SIZE_CLASSES {
            let info = &SIZE_CLASSES[cls];
            let objs = info.objects_per_span();
            assert!(objs >= 1, "class {} has 0 objects per span", cls);
            // Verify objects fit in span
            assert!(objs * info.size <= info.pages * PAGE_SIZE);
        }
    }
}

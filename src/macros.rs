/// Increment a stats counter by 1.
///
/// Compiles to nothing when the `stats` feature is disabled.
#[macro_export]
macro_rules! stat_inc {
    ($counter:ident) => {
        #[cfg(feature = "stats")]
        {
            $crate::stats::STATS
                .$counter
                .fetch_add(1, ::core::sync::atomic::Ordering::Relaxed);
        }
    };
}

/// Add a value to a stats counter.
///
/// Compiles to nothing (including the value expression) when the `stats`
/// feature is disabled.
#[macro_export]
macro_rules! stat_add {
    ($counter:ident, $val:expr) => {
        #[cfg(feature = "stats")]
        {
            $crate::stats::STATS
                .$counter
                .fetch_add($val as u64, ::core::sync::atomic::Ordering::Relaxed);
        }
    };
}

/// Record an allocation size in the histogram.
///
/// Compiles to nothing when the `alloc-histogram` feature is disabled.
#[macro_export]
macro_rules! hist_record {
    ($size:expr) => {
        #[cfg(feature = "alloc-histogram")]
        {
            $crate::histogram::record($size);
        }
    };
}

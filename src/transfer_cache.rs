//! Transfer Cache: per-size-class batch cache between thread caches and central free lists.
//!
//! Stores pre-built linked lists of exactly `batch_size` objects. Thread caches
//! transfer full batches to/from here in O(1). This avoids the per-object span
//! lookups in the central free list for the common case where one thread frees
//! a batch and another allocates it.

use crate::central_free_list::{
    self, CentralCache,
};
use crate::page_heap::PageHeap;
use crate::pagemap::PageMap;
use crate::size_class::{self, NUM_SIZE_CLASSES};
use crate::span::FreeObject;
use crate::sync::SpinMutex;
use core::ptr;

/// Maximum number of cached batches per size class.
const MAX_SLOTS: usize = 64;

#[derive(Clone, Copy)]
struct TransferCacheSlot {
    head: *mut FreeObject,
    tail: *mut FreeObject,
}

/// Per-size-class transfer cache (LIFO stack of batches).
struct TransferCacheInner {
    slots: [TransferCacheSlot; MAX_SLOTS],
    used: usize,
}

// SAFETY: Only accessed through external SpinMutex synchronization.
unsafe impl Send for TransferCacheInner {}

impl TransferCacheInner {
    const fn new() -> Self {
        Self {
            slots: [TransferCacheSlot {
                head: ptr::null_mut(),
                tail: ptr::null_mut(),
            }; MAX_SLOTS],
            used: 0,
        }
    }

    /// Pop a batch. Returns (head, tail) or None.
    fn pop(&mut self) -> Option<(*mut FreeObject, *mut FreeObject)> {
        if self.used == 0 {
            return None;
        }
        self.used -= 1;
        let slot = self.slots[self.used];
        Some((slot.head, slot.tail))
    }

    /// Push a batch. Returns true if successful, false if full.
    fn push(&mut self, head: *mut FreeObject, tail: *mut FreeObject) -> bool {
        if self.used >= MAX_SLOTS {
            return false;
        }
        self.slots[self.used] = TransferCacheSlot { head, tail };
        self.used += 1;
        true
    }
}

/// Array of transfer caches, one per size class.
/// Each is individually locked (separate from central free list locks).
pub struct TransferCacheArray {
    caches: [SpinMutex<TransferCacheInner>; NUM_SIZE_CLASSES],
}

impl TransferCacheArray {
    pub const fn new() -> Self {
        Self {
            caches: [const { SpinMutex::new(TransferCacheInner::new()) }; NUM_SIZE_CLASSES],
        }
    }

    /// Remove a batch of objects for the given size class.
    /// Tries transfer cache first (O(1)), falls through to central free list on miss.
    pub unsafe fn remove_range(
        &self,
        size_class: usize,
        count: usize,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) -> (usize, *mut FreeObject) {
        let batch_size = size_class::class_info(size_class).batch_size;

        // Try transfer cache (O(1) if hit)
        {
            let mut tc = self.caches[size_class].lock();
            if let Some((head, _tail)) = tc.pop() {
                return (batch_size, head);
            }
        }
        // Transfer cache lock released before central lock -- no deadlock possible

        // Fall through to central free list (with lock dropping for page heap calls)
        unsafe {
            central_free_list::remove_range_dropping_lock(
                central.get(size_class), size_class, count, page_heap, pagemap,
            )
        }
    }

    /// Insert a batch of objects for the given size class.
    /// If count == batch_size, tries transfer cache first (O(1)).
    /// Falls through to central free list if cache is full or count != batch_size.
    pub unsafe fn insert_range(
        &self,
        size_class: usize,
        head: *mut FreeObject,
        tail: *mut FreeObject,
        count: usize,
        central: &CentralCache,
        page_heap: &SpinMutex<PageHeap>,
        pagemap: &PageMap,
    ) {
        let batch_size = size_class::class_info(size_class).batch_size;

        // Only cache exact-batch-size transfers
        if count == batch_size {
            let mut tc = self.caches[size_class].lock();
            if tc.push(head, tail) {
                return;
            }
            // Transfer cache full -- fall through
        }
        // Transfer cache lock released before central lock

        // Fall through to central free list (with lock dropping for span dealloc)
        unsafe {
            central_free_list::insert_range_dropping_lock(
                central.get(size_class), head, count, page_heap, pagemap,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_heap::PageHeap;
    use crate::pagemap::PageMap;

    fn make_test_env() -> (
        &'static PageMap,
        SpinMutex<PageHeap>,
        CentralCache,
        TransferCacheArray,
    ) {
        let pm = Box::leak(Box::new(PageMap::new()));
        let heap = SpinMutex::new(PageHeap::new(pm));
        let central = CentralCache::new();
        let tc = TransferCacheArray::new();
        (pm, heap, central, tc)
    }

    #[test]
    fn test_transfer_cache_remove_populates() {
        let (pm, heap, central, tc) = make_test_env();
        unsafe {
            let (count, head) = tc.remove_range(1, 32, &central, &heap, pm);
            assert!(count > 0);
            assert!(!head.is_null());
        }
    }

    #[test]
    fn test_transfer_cache_roundtrip() {
        let (pm, heap, central, tc) = make_test_env();
        unsafe {
            // Get a batch from central (through transfer cache)
            let batch_size = size_class::class_info(1).batch_size;
            let (count, head) = tc.remove_range(1, batch_size, &central, &heap, pm);
            assert_eq!(count, batch_size);

            // Find the tail
            let mut tail = head;
            for _ in 1..count {
                let next = (*tail).next;
                if next.is_null() {
                    break;
                }
                tail = next;
            }

            // Insert back (should go into transfer cache since count == batch_size)
            tc.insert_range(1, head, tail, count, &central, &heap, pm);

            // Remove again -- should come from transfer cache (O(1))
            let (count2, head2) = tc.remove_range(1, batch_size, &central, &heap, pm);
            assert_eq!(count2, batch_size);
            assert_eq!(head2, head); // Same batch returned (LIFO)
        }
    }

    #[test]
    fn test_transfer_cache_overflow() {
        let (pm, heap, central, tc) = make_test_env();
        unsafe {
            let batch_size = size_class::class_info(4).batch_size;

            // Fill 64 slots + central fallthrough
            for _ in 0..MAX_SLOTS + 1 {
                let (count, head) = tc.remove_range(4, batch_size, &central, &heap, pm);
                assert!(count > 0);

                let mut tail = head;
                for _ in 1..count {
                    let next = (*tail).next;
                    if next.is_null() {
                        break;
                    }
                    tail = next;
                }

                tc.insert_range(4, head, tail, count, &central, &heap, pm);
            }

            // Should still be able to remove (from transfer cache or central)
            let (count, head) = tc.remove_range(4, batch_size, &central, &heap, pm);
            assert!(count > 0);
            assert!(!head.is_null());
        }
    }
}

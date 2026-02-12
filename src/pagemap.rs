//! 3-level radix tree mapping page IDs to Span pointers.
//!
//! For 48-bit virtual addresses with 13-bit page shift, we have 35 bits of
//! page ID. Split as: root 12 bits, mid 12 bits, leaf 11 bits.
//!
//! The root is statically allocated (32 KiB). Mid and leaf nodes are lazily
//! allocated from the OS. Reads are lock-free (AtomicPtr with Acquire).
//! Writes must happen under external synchronization (the page heap lock).

use crate::PAGE_SIZE;
use crate::platform;
use crate::span::Span;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

const ROOT_BITS: usize = 12;
const MID_BITS: usize = 12;
const LEAF_BITS: usize = 11;

const ROOT_LEN: usize = 1 << ROOT_BITS; // 4096
const MID_LEN: usize = 1 << MID_BITS; // 4096
const LEAF_LEN: usize = 1 << LEAF_BITS; // 2048

const MID_SHIFT: usize = LEAF_BITS; // 11
const ROOT_SHIFT: usize = LEAF_BITS + MID_BITS; // 23

const MID_MASK: usize = (1 << MID_BITS) - 1;
const LEAF_MASK: usize = (1 << LEAF_BITS) - 1;

#[repr(C)]
struct MidNode {
    children: [AtomicPtr<LeafNode>; MID_LEN],
}

#[repr(C)]
struct LeafNode {
    spans: [AtomicPtr<Span>; LEAF_LEN],
}

/// 3-level radix tree for page_id -> *mut Span lookup.
pub struct PageMap {
    root: [AtomicPtr<MidNode>; ROOT_LEN],
}

// AtomicPtr is Send+Sync, and we only expose safe operations
unsafe impl Send for PageMap {}
unsafe impl Sync for PageMap {}

/// Helper to create a const-initialized array of null AtomicPtrs.
/// We use a macro since const generics with AtomicPtr arrays require this.
macro_rules! null_atomic_array {
    ($len:expr, $T:ty) => {{
        // SAFETY: AtomicPtr<T>::new(null_mut()) is just a null pointer,
        // which has the same bit pattern as zeroed memory.
        unsafe { core::mem::transmute::<[usize; $len], [AtomicPtr<$T>; $len]>([0usize; $len]) }
    }};
}

impl PageMap {
    /// Create a new empty page map. All root entries are null.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            root: null_atomic_array!(ROOT_LEN, MidNode),
        }
    }

    /// Look up the span for a given page ID. Returns null if not set.
    /// This is lock-free.
    #[inline]
    pub fn get(&self, page_id: usize) -> *mut Span {
        let root_idx = page_id >> ROOT_SHIFT;
        let mid_idx = (page_id >> MID_SHIFT) & MID_MASK;
        let leaf_idx = page_id & LEAF_MASK;

        if root_idx >= ROOT_LEN {
            return ptr::null_mut();
        }

        let mid = self.root[root_idx].load(Ordering::Acquire);
        if mid.is_null() {
            return ptr::null_mut();
        }

        let leaf = unsafe { (*mid).children[mid_idx].load(Ordering::Acquire) };
        if leaf.is_null() {
            return ptr::null_mut();
        }

        unsafe { (*leaf).spans[leaf_idx].load(Ordering::Acquire) }
    }

    /// Set the span for a given page ID.
    ///
    /// # Safety
    /// Must be called under external synchronization (the page heap lock).
    /// The span pointer must be valid or null.
    pub unsafe fn set(&self, page_id: usize, span: *mut Span) {
        let root_idx = page_id >> ROOT_SHIFT;
        let mid_idx = (page_id >> MID_SHIFT) & MID_MASK;
        let leaf_idx = page_id & LEAF_MASK;

        assert!(root_idx < ROOT_LEN, "page_id out of range for page map");

        // Ensure mid node exists
        let mut mid = self.root[root_idx].load(Ordering::Acquire);
        if mid.is_null() {
            mid = unsafe { Self::alloc_mid_node() };
            assert!(!mid.is_null(), "failed to allocate mid node for page map");
            // Store with Release so readers see the initialized node
            self.root[root_idx].store(mid, Ordering::Release);
        }

        // Ensure leaf node exists
        let mut leaf = unsafe { (*mid).children[mid_idx].load(Ordering::Acquire) };
        if leaf.is_null() {
            leaf = unsafe { Self::alloc_leaf_node() };
            assert!(!leaf.is_null(), "failed to allocate leaf node for page map");
            unsafe { (*mid).children[mid_idx].store(leaf, Ordering::Release) };
        }

        unsafe { (*leaf).spans[leaf_idx].store(span, Ordering::Release) };
    }

    /// Register a span for all pages it covers.
    ///
    /// # Safety
    /// Must be called under external synchronization.
    pub unsafe fn register_span(&self, span: *mut Span) {
        let start = unsafe { (*span).start_page };
        let count = unsafe { (*span).num_pages };
        for page_id in start..start + count {
            unsafe { self.set(page_id, span) };
        }
    }

    /// Unregister a span (set all its pages to null).
    ///
    /// # Safety
    /// Must be called under external synchronization.
    pub unsafe fn unregister_span(&self, span: *mut Span) {
        let start = unsafe { (*span).start_page };
        let count = unsafe { (*span).num_pages };
        for page_id in start..start + count {
            unsafe { self.set(page_id, ptr::null_mut()) };
        }
    }

    unsafe fn alloc_mid_node() -> *mut MidNode {
        let size = core::mem::size_of::<MidNode>();
        // Round up to page size
        let alloc_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let ptr = unsafe { platform::page_alloc(alloc_size) };
        // page_alloc returns zeroed memory, which is valid for AtomicPtr (all null)
        ptr.cast::<MidNode>()
    }

    unsafe fn alloc_leaf_node() -> *mut LeafNode {
        let size = core::mem::size_of::<LeafNode>();
        let alloc_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let ptr = unsafe { platform::page_alloc(alloc_size) };
        ptr.cast::<LeafNode>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::{self, SpanState};

    #[test]
    fn test_pagemap_get_empty() {
        let map = PageMap::new();
        assert!(map.get(0).is_null());
        assert!(map.get(1000).is_null());
        assert!(map.get(123456).is_null());
    }

    #[test]
    fn test_pagemap_set_get() {
        let map = PageMap::new();
        let s = span::alloc_span();
        assert!(!s.is_null());

        unsafe {
            (*s).start_page = 42;
            (*s).num_pages = 1;

            map.set(42, s);
            assert_eq!(map.get(42), s);
            assert!(map.get(41).is_null());
            assert!(map.get(43).is_null());

            // Clear it
            map.set(42, ptr::null_mut());
            assert!(map.get(42).is_null());

            span::dealloc_span(s);
        }
    }

    #[test]
    fn test_pagemap_register_span() {
        let map = PageMap::new();
        let s = span::alloc_span();
        assert!(!s.is_null());

        unsafe {
            (*s).start_page = 100;
            (*s).num_pages = 5;
            (*s).state = SpanState::InUse;

            map.register_span(s);

            for page in 100..105 {
                assert_eq!(map.get(page), s);
            }
            assert!(map.get(99).is_null());
            assert!(map.get(105).is_null());

            map.unregister_span(s);
            for page in 100..105 {
                assert!(map.get(page).is_null());
            }

            span::dealloc_span(s);
        }
    }

    #[test]
    fn test_pagemap_high_address() {
        let map = PageMap::new();
        let s = span::alloc_span();
        assert!(!s.is_null());

        unsafe {
            // Use a high page ID that exercises all three levels
            let page_id = (1 << 20) + (1 << 15) + 42;
            (*s).start_page = page_id;
            (*s).num_pages = 1;

            map.set(page_id, s);
            assert_eq!(map.get(page_id), s);
            assert!(map.get(page_id - 1).is_null());
            assert!(map.get(page_id + 1).is_null());

            span::dealloc_span(s);
        }
    }
}

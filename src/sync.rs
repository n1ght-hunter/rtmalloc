//! Lightweight synchronization primitives for use in the allocator.
//!
//! We cannot use `std::sync::Mutex` because it allocates. Instead we provide
//! a simple test-and-set spinlock and a `SpinMutex<T>` wrapper.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple test-and-set spinlock.
pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn lock(&self) {
        // Fast path: try to acquire immediately
        if self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
        // Slow path: spin with backoff
        self.lock_slow();
    }

    #[cold]
    fn lock_slow(&self) {
        loop {
            // Spin while locked (read-only, doesn't invalidate cache line)
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
            // Try to acquire
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
    }

    #[inline]
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
}

// SpinLock is safe to share between threads
unsafe impl Send for SpinLock {}
unsafe impl Sync for SpinLock {}

/// A mutex that uses a spinlock for synchronization.
/// Does not allocate and can be used in a `static`.
pub struct SpinMutex<T> {
    lock: SpinLock,
    data: UnsafeCell<T>,
}

impl<T> SpinMutex<T> {
    pub const fn new(val: T) -> Self {
        Self {
            lock: SpinLock::new(),
            data: UnsafeCell::new(val),
        }
    }

    #[inline]
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        self.lock.lock();
        SpinMutexGuard { mutex: self }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T>> {
        if self.lock.try_lock() {
            Some(SpinMutexGuard { mutex: self })
        } else {
            None
        }
    }
}

unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

/// RAII guard for `SpinMutex`. Unlocks on drop.
pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<T> Deref for SpinMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for SpinMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.lock.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use std::sync::Arc;

    #[test]
    fn test_spinlock_basic() {
        let lock = SpinLock::new();
        lock.lock();
        lock.unlock();
    }

    #[test]
    fn test_spinlock_try() {
        let lock = SpinLock::new();
        assert!(lock.try_lock());
        assert!(!lock.try_lock());
        lock.unlock();
        assert!(lock.try_lock());
        lock.unlock();
    }

    #[test]
    fn test_spinmutex_basic() {
        let mutex = SpinMutex::new(42u64);
        {
            let guard = mutex.lock();
            assert_eq!(*guard, 42);
        }
        {
            let mut guard = mutex.lock();
            *guard = 100;
        }
        {
            let guard = mutex.lock();
            assert_eq!(*guard, 100);
        }
    }

    #[test]
    fn test_spinmutex_concurrent() {
        let mutex = Arc::new(SpinMutex::new(0u64));
        let num_threads = 8;
        let iterations = 10_000;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let m = Arc::clone(&mutex);
                std::thread::spawn(move || {
                    for _ in 0..iterations {
                        let mut guard = m.lock();
                        *guard += 1;
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let guard = mutex.lock();
        assert_eq!(*guard, num_threads * iterations);
    }
}

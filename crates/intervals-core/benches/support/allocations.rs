//! Allocation measurement is a separate untimed run. Includes thread allocations,
//! excludes inputs and verification; bytes requested, not OS resident memory.
//!
//! Meaningful counters require one measurement at a time, no unrelated allocation
//! activity, and retaining preexisting allocations until measurement ends. Join all
//! measured workers before returning. These counters do not track allocation
//! identities, so freeing preexisting allocations can invalidate the statistics.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

pub struct Allocator;
static ENABLED: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static COUNT: AtomicUsize = AtomicUsize::new(0);

fn allocated(size: usize) {
    if ENABLED.load(Relaxed) {
        COUNT.fetch_add(1, Relaxed);
        // Match the atomic's wrapping arithmetic: allocator hooks must not unwind,
        // even if a preexisting allocation was freed during measurement.
        let current = LIVE.fetch_add(size, Relaxed).wrapping_add(size);
        PEAK.fetch_max(current, Relaxed);
    }
}
fn freed(size: usize) {
    if ENABLED.load(Relaxed) {
        LIVE.fetch_sub(size, Relaxed);
    }
}

// SAFETY: every operation forwards the identical layout/pointer to System;
// instrumentation only uses non-allocating, non-panicking atomic arithmetic.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies a valid, nonzero allocation layout.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        freed(layout.size());
        // SAFETY: allocations originate from System, and the caller supplies the
        // live pointer and its original layout. Ownership is forwarded once.
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies a valid, nonzero allocation layout.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: the caller supplies a live System allocation, its original
        // layout, and a valid nonzero new size. System preserves it on failure.
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() {
            freed(layout.size());
            allocated(size);
        }
        next
    }
}

struct DisableOnDrop;

impl Drop for DisableOnDrop {
    fn drop(&mut self) {
        ENABLED.store(false, Relaxed);
    }
}

pub fn measure<T>(run: impl FnOnce() -> T) -> (T, usize, usize) {
    LIVE.store(0, Relaxed);
    PEAK.store(0, Relaxed);
    COUNT.store(0, Relaxed);
    ENABLED.store(true, Relaxed);
    let guard = DisableOnDrop;
    let result = run();
    drop(guard);
    (result, PEAK.load(Relaxed), COUNT.load(Relaxed))
}

#[cfg(test)]
mod tests {
    // Keep these checks in one test because measurement state is process-wide.
    // Call the safe accounting helpers directly; never register this allocator
    // globally when testing arithmetic that could unwind in the broken version.
    #[test]
    fn accounting_wraps_without_unwinding_and_disables_after_panic() {
        use super::*;

        let (result, peak, count) = measure(|| {
            allocated(16);
            freed(8);
            allocated(4);
            42
        });
        assert_eq!((result, peak, count), (42, 16, 2));
        assert!(!ENABLED.load(Relaxed));

        let (_, peak, count) = measure(|| {
            freed(16);
            allocated(16);
        });
        assert_eq!((peak, count), (0, 1));
        assert_eq!(LIVE.load(Relaxed), 0);

        let result = std::panic::catch_unwind(|| {
            measure(|| panic!("measured closure failed"));
        });
        assert!(result.is_err());
        assert!(!ENABLED.load(Relaxed));
    }
}

//! Allocation measurement is a separate untimed run. Includes thread allocations,
//! excludes inputs and verification; bytes requested, not OS resident memory.
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
        let current = LIVE.fetch_add(size, Relaxed) + size;
        PEAK.fetch_max(current, Relaxed);
    }
}
fn freed(size: usize) {
    if ENABLED.load(Relaxed) {
        LIVE.fetch_sub(size, Relaxed);
    }
}

// SAFETY: every operation forwards the identical layout/pointer to System;
// instrumentation only observes successful allocations through atomic counters.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        freed(layout.size());
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() {
            freed(layout.size());
            allocated(size);
        }
        next
    }
}

pub fn measure<T>(run: impl FnOnce() -> T) -> (T, usize, usize) {
    LIVE.store(0, Relaxed);
    PEAK.store(0, Relaxed);
    COUNT.store(0, Relaxed);
    ENABLED.store(true, Relaxed);
    let result = run();
    ENABLED.store(false, Relaxed);
    (result, PEAK.load(Relaxed), COUNT.load(Relaxed))
}

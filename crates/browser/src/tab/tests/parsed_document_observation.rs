//! Per-thread observation allocation guard. Other runtime threads are excluded.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {static WORK:Cell<(bool,[usize;3])>=const{Cell::new((false,[0;3]))};}
thread_local! {static REFUSE_NEXT:Cell<bool>=const{Cell::new(false)};}
thread_local! {static REFUSE_AFTER:Cell<Option<usize>>=const{Cell::new(None)};}
// Arm immediately before a qualified fallible inventory reservation, avoiding
// accidental refusal in inherited AH/CSS or fixture construction allocations.
pub(crate) fn arm_inventory_allocation_refusal() {
    REFUSE_NEXT.with(|v| assert!(!v.replace(true)));
}
pub(crate) fn inventory_refusal_consumed() -> bool {
    REFUSE_NEXT.with(|v| !v.get())
}
fn refuse() -> bool {
    if REFUSE_NEXT.try_with(|v| v.replace(false)).unwrap_or(false) {
        return true;
    }
    REFUSE_AFTER
        .try_with(|v| match v.get() {
            Some(0) => {
                v.set(None);
                true
            }
            Some(remaining) => {
                v.set(Some(remaining - 1));
                false
            }
            None => false,
        })
        .unwrap_or(false)
}
struct ObservationAllocator;
fn record(bytes: usize, reallocation: bool) {
    let _ = WORK.try_with(|w| {
        let (enabled, mut counts) = w.get();
        if enabled {
            counts[usize::from(reallocation) * 2] += 1;
            counts[1] += bytes;
            w.set((true, counts));
        }
    });
}
unsafe impl GlobalAlloc for ObservationAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if refuse() {
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            record(layout.size(), false);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if refuse() {
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            record(layout.size(), false);
        }
        p
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // A refused realloc leaves the original allocation owned by its caller.
        if refuse() {
            return std::ptr::null_mut();
        }
        let q = unsafe { System.realloc(p, layout, size) };
        if !q.is_null() {
            record(size.saturating_sub(layout.size()), true);
        }
        q
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: ObservationAllocator = ObservationAllocator;
struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        WORK.with(|w| {
            let (_, c) = w.get();
            w.set((false, c));
        });
    }
}
pub(crate) fn allocation_work<T>(f: impl FnOnce() -> T) -> (T, [usize; 3]) {
    WORK.with(|w| {
        assert!(!w.get().0);
        w.set((true, [0; 3]));
    });
    let scope = Scope;
    let value = f();
    let counts = WORK.with(|w| w.get().1);
    drop(scope);
    (value, counts)
}

pub(super) fn refuse_next_allocation<T>(f: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REFUSE_NEXT.with(|v| v.set(false));
        }
    }
    REFUSE_NEXT.with(|v| assert!(!v.replace(true)));
    let reset = Reset;
    let value = f();
    let unconsumed = REFUSE_NEXT.with(|v| v.replace(false));
    drop(reset);
    assert!(!unconsumed, "fallible reservation must consume refusal");
    value
}

/// Refuse one later allocation at the same production reservation boundary.
pub(super) fn refuse_allocation_after<T>(
    successful_allocations: usize,
    f: impl FnOnce() -> T,
) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REFUSE_AFTER.with(|v| v.set(None));
        }
    }
    REFUSE_AFTER.with(|v| assert!(v.replace(Some(successful_allocations)).is_none()));
    let reset = Reset;
    let value = f();
    let pending = REFUSE_AFTER.with(|v| v.take());
    drop(reset);
    assert!(
        pending.is_none(),
        "targeted fallible reservation must consume refusal"
    );
    value
}

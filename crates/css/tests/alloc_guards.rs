#![cfg(feature = "count-alloc")]

use css::{
    ParseOptions, compute_document_styles, parse_stylesheet_with_options, perf_fixtures,
    resolve_document_styles,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

// Lightweight allocation counters for opt-in regression guards. These measure
// allocation/reallocation events and allocation growth while enabled, not live
// heap usage.
struct CountingAlloc;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOC_MEASURE_LOCK: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && ENABLED.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() && ENABLED.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() && ENABLED.load(Ordering::Relaxed) {
            REALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            let old_size = layout.size();
            if new_size > old_size {
                ALLOC_BYTES.fetch_add(new_size - old_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[derive(Clone, Copy, Debug)]
struct AllocCounts {
    allocs: usize,
    bytes: usize,
    reallocs: usize,
}

struct AllocGuard;

impl AllocGuard {
    fn new() -> Self {
        ALLOC_COUNT.store(0, Ordering::Relaxed);
        ALLOC_BYTES.store(0, Ordering::Relaxed);
        REALLOC_COUNT.store(0, Ordering::Relaxed);
        ENABLED.store(true, Ordering::Relaxed);
        Self
    }
}

impl Drop for AllocGuard {
    fn drop(&mut self) {
        ENABLED.store(false, Ordering::Relaxed);
    }
}

fn alloc_counts() -> AllocCounts {
    AllocCounts {
        allocs: ALLOC_COUNT.load(Ordering::Relaxed),
        bytes: ALLOC_BYTES.load(Ordering::Relaxed),
        reallocs: REALLOC_COUNT.load(Ordering::Relaxed),
    }
}

fn measure<T>(warm: impl FnOnce(), run: impl FnOnce() -> T) -> (T, AllocCounts) {
    let _lock = ALLOC_MEASURE_LOCK
        .lock()
        .expect("allocation measurement lock poisoned");

    warm();

    let guard = AllocGuard::new();
    let output = run();
    let counts = alloc_counts();
    drop(guard);

    (output, counts)
}

#[test]
fn parse_representative_stylesheet_allocation_is_bounded() {
    const RULES: usize = 256;

    let css = perf_fixtures::representative_stylesheet(RULES);
    let (parsed, counts) = measure(
        || {
            let _ = parse_stylesheet_with_options(&css, &ParseOptions::stylesheet());
        },
        || parse_stylesheet_with_options(&css, &ParseOptions::stylesheet()),
    );

    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.stats.rules_emitted, RULES);

    let max_bytes = css.len().saturating_mul(256);
    let max_allocs = RULES.saturating_mul(96);
    let max_reallocs = RULES.saturating_mul(16);
    assert!(
        counts.bytes <= max_bytes,
        "CSS parse allocation bytes exceeded guard: bytes={} max={} input={}",
        counts.bytes,
        max_bytes,
        css.len()
    );
    assert!(
        counts.allocs <= max_allocs,
        "CSS parse allocation events exceeded guard: allocs={} max={}",
        counts.allocs,
        max_allocs
    );
    assert!(
        counts.reallocs <= max_reallocs,
        "CSS parse realloc events exceeded guard: reallocs={} max={}",
        counts.reallocs,
        max_reallocs
    );
}

#[test]
fn style_resolution_allocation_is_bounded_for_representative_page() {
    const RULES: usize = 128;
    const BLOCKS: usize = 256;

    let css = perf_fixtures::representative_stylesheet(RULES);
    let sheets = vec![parse_stylesheet_with_options(
        &css,
        &ParseOptions::stylesheet(),
    )];
    let dom = perf_fixtures::representative_dom(BLOCKS);

    let (computed, counts) = measure(
        || {
            let _ = resolve_document_styles(&dom, &sheets).expect("warm style resolution");
            let _ = compute_document_styles(&dom, &sheets).expect("warm computed style");
        },
        || compute_document_styles(&dom, &sheets).expect("computed style should work"),
    );

    let entries = perf_fixtures::representative_element_count(BLOCKS);
    assert_eq!(computed.entries().len(), entries);

    let max_bytes = entries.saturating_mul(24_000);
    let max_allocs = entries.saturating_mul(80);
    let max_reallocs = entries.saturating_mul(16);
    assert!(
        counts.bytes <= max_bytes,
        "style resolution allocation bytes exceeded guard: bytes={} max={} entries={}",
        counts.bytes,
        max_bytes,
        entries
    );
    assert!(
        counts.allocs <= max_allocs,
        "style resolution allocation events exceeded guard: allocs={} max={} entries={}",
        counts.allocs,
        max_allocs,
        entries
    );
    assert!(
        counts.reallocs <= max_reallocs,
        "style resolution realloc events exceeded guard: reallocs={} max={} entries={}",
        counts.reallocs,
        max_reallocs,
        entries
    );
}

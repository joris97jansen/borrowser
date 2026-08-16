#![cfg(feature = "count-alloc")]

use css::{
    ParseOptions, Rule, SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment,
    compute_document_styles, parse_stylesheet_with_options, perf_fixtures, resolve_document_styles,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

// Lightweight allocation counters for opt-in regression guards. These measure
// allocation/reallocation events and allocation growth while enabled, not live
// heap usage.
struct CountingAlloc;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_MEASURE_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    // The test harness may prepare a different allocation test concurrently.
    // Count only work performed by the thread that owns the serialized
    // measurement region, not unrelated allocations from neighboring tests.
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
}

fn count_allocations_on_current_thread() -> bool {
    COUNT_ALLOCATIONS.try_with(Cell::get).unwrap_or(false)
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && count_allocations_on_current_thread() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() && count_allocations_on_current_thread() {
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
        if !new_ptr.is_null() && count_allocations_on_current_thread() {
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
        COUNT_ALLOCATIONS.with(|enabled| {
            assert!(!enabled.replace(true), "nested allocation measurement");
        });
        Self
    }
}

impl Drop for AllocGuard {
    fn drop(&mut self) {
        COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
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
fn selector_comparison_hot_path_is_allocation_free() {
    const MATCHES: usize = 4_096;
    const SELECTOR: &str = concat!(
        "DIV#mixed-é.mixed-é",
        "[DISABLED]",
        "[ACCEPT=\"mixed-é\"]",
        "[REL~=\"mixed-é\"]",
        "[LANG|=\"mixed-é\"]",
        "[TYPE^=\"mixed-é\"]",
        "[TARGET$=\"mixed-é\"]",
        "[MEDIA*=\"mixed-é\"]",
        "[data-kind=\"Exact-é\"]",
    );

    // Every owned artifact is constructed before allocation measurement. A
    // missing doctype intentionally gives this parser-created HTML document
    // full Quirks mode for the ID/class comparison path.
    let parsed = html::parse_document(
        concat!(
            "<html><body><div ",
            "id=\"MiXeD-é\" class=\"MiXeD-é\" ",
            "disabled ",
            "accept=\"MiXeD-é\" rel=\"left MiXeD-é right\" ",
            "lang=\"MiXeD-é-tail\" type=\"MiXeD-é-tail\" ",
            "target=\"head-MiXeD-é\" media=\"head-MiXeD-é-tail\" ",
            "data-kind=\"Exact-é\"></div></body></html>",
        ),
        html::HtmlParseOptions::default(),
    )
    .expect("host-language allocation fixture should parse");
    assert_eq!(parsed.document_mode, html::DocumentMode::Quirks);

    let index = SelectorDomIndex::try_from_document(&parsed.document)
        .expect("host-language allocation fixture should project");
    let context = SelectorMatchingContext::new(
        &index,
        SelectorMatchingEnvironment::new(parsed.document_mode),
    );
    let element = index
        .elements()
        .find(|element| context.element_local_name(*element) == "div")
        .expect("host-language allocation fixture should contain its target div");

    let stylesheet = parse_stylesheet_with_options(
        &format!("{SELECTOR} {{ color: red; }}"),
        &ParseOptions::stylesheet(),
    );
    assert!(stylesheet.diagnostics.is_empty());
    let Rule::Style(rule) = &stylesheet.stylesheet.rules[0] else {
        panic!("host-language allocation selector should parse as a style rule");
    };
    let selector = rule
        .selectors
        .parsed()
        .expect("host-language allocation selector should be supported")
        .selectors()[0]
        .head();

    let (matched, counts) = measure(
        || {
            assert!(context.matches_compound_selector(element, selector));
        },
        || {
            let mut matched = 0usize;
            for _ in 0..MATCHES {
                let result = std::hint::black_box(&context).matches_compound_selector(
                    std::hint::black_box(element),
                    std::hint::black_box(selector),
                );
                matched += usize::from(std::hint::black_box(result));
            }
            std::hint::black_box(matched)
        },
    );

    assert_eq!(matched, MATCHES, "every measured comparison should match");
    assert_eq!(counts.allocs, 0, "selector comparisons allocated");
    assert_eq!(counts.bytes, 0, "selector comparisons allocated bytes");
    assert_eq!(counts.reallocs, 0, "selector comparisons reallocated");
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
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);

    let (computed, counts) = measure(
        || {
            let _ =
                resolve_document_styles(&dom, environment, &sheets).expect("warm style resolution");
            let _ =
                compute_document_styles(&dom, environment, &sheets).expect("warm computed style");
        },
        || compute_document_styles(&dom, environment, &sheets).expect("computed style should work"),
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

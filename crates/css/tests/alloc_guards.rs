#![cfg(feature = "count-alloc")]

use css::{
    ChangedStyleNodeFacts, DomStyleAttributeMutation, DomStyleChangeFacts, ParseOptions, Rule,
    RuleCollection, SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment,
    StyleChangeFacts, StyleDependencyArtifact, StyleInvalidationDecision, StyleInvalidationInput,
    StyleResolutionLimits, StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder,
    StylesheetSourceId, af5_match_rule_inputs_for_allocation_guard,
    af6_resolve_winners_for_allocation_guard, classify_style_invalidation_with_dependencies,
    compute_document_styles, parse_stylesheet_with_options, perf_fixtures, property_registry,
    resolve_document_styles,
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

fn af9_dependency_artifact(
    source: &str,
    environment: SelectorMatchingEnvironment,
) -> StyleDependencyArtifact {
    let sheet = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
    let input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &sheet,
        StylesheetConditionInput::None,
    );
    let limits = StyleResolutionLimits::default();
    let collection = RuleCollection::try_new(&[input], &limits)
        .expect("AF9 allocation-guard collection should build");
    StyleDependencyArtifact::from_rule_collection(&collection, environment, &limits)
}

fn af9_attribute_change(node_id: html::internal::Id) -> StyleChangeFacts {
    StyleChangeFacts::dom_publication(
        DomStyleChangeFacts::builder()
            .attributes(ChangedStyleNodeFacts::changed([node_id]))
            .build(),
    )
}

fn measure_af9_attribute_classification<'a>(
    change: &'a StyleChangeFacts,
    artifact: &'a StyleDependencyArtifact,
    environment: SelectorMatchingEnvironment,
    mutations: &'a [DomStyleAttributeMutation<'a>],
) -> (StyleInvalidationDecision, AllocCounts) {
    measure(
        || {
            let decision = classify_style_invalidation_with_dependencies(
                StyleInvalidationInput::new(change, Some(artifact), environment)
                    .with_attribute_mutations(Some(mutations)),
            );
            std::hint::black_box(decision);
        },
        || {
            std::hint::black_box(classify_style_invalidation_with_dependencies(
                StyleInvalidationInput::new(change, Some(artifact), environment)
                    .with_attribute_mutations(Some(mutations)),
            ))
        },
    )
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

    eprintln!(
        "AF5 representative style resolution: bytes={} allocs={} reallocs={} entries={}",
        counts.bytes, counts.allocs, counts.reallocs, entries
    );

    // The accepted AF5 boundary and this corrected dependency/order pass both
    // measured 28,095,354 bytes for 1,025 entries on the review platform.
    // Keep less than six percent headroom around that deliberate provenance
    // growth instead of carrying the provisional 30,000-byte budget.
    let max_bytes = entries.saturating_mul(29_000);
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

#[test]
fn af5_rule_collection_arena_is_built_once_independent_of_element_count() {
    let css = (0..64)
        .map(|index| format!(".r{index} {{ color: red; width: {index}px; }}"))
        .collect::<String>();
    let sheet = parse_stylesheet_with_options(&css, &ParseOptions::stylesheet());
    let input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &sheet,
        StylesheetConditionInput::None,
    );
    let limits = StyleResolutionLimits::default();
    let (collection, counts) = measure(
        || {
            RuleCollection::try_new(&[input], &limits).expect("warm collection build");
        },
        || RuleCollection::try_new(&[input], &limits).expect("measured collection build"),
    );
    eprintln!(
        "AF5 collection arena: bytes={} allocs={} reallocs={} rules=64 declarations=128",
        counts.bytes, counts.allocs, counts.reallocs
    );
    assert!(
        counts.bytes <= 52_000,
        "collection arena bytes={}",
        counts.bytes
    );
    assert!(
        counts.allocs <= 420,
        "collection arena allocs={}",
        counts.allocs
    );
    assert!(
        counts.reallocs <= 8,
        "collection arena reallocs={}",
        counts.reallocs
    );

    let one = html::parse_document(
        "<!doctype html><html><body><div class=r1></div></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("one-element document parses");
    let many_body = (0..128).map(|_| "<div class=r1></div>").collect::<String>();
    let many = html::parse_document(
        &format!("<!doctype html><html><body>{many_body}</body></html>"),
        html::HtmlParseOptions::default(),
    )
    .expect("many-element document parses");
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);
    let (one_counts, _) = af5_match_rule_inputs_for_allocation_guard(
        &one.document,
        environment,
        &collection,
        &limits,
    )
    .expect("one-element matching succeeds");
    let (many_counts, _) = af5_match_rule_inputs_for_allocation_guard(
        &many.document,
        environment,
        &collection,
        &limits,
    )
    .expect("many-element matching succeeds");
    assert_eq!(one_counts, 1);
    assert_eq!(many_counts, 128);
}

#[test]
fn af6_transient_workspace_is_registry_sized_and_reused_across_elements() {
    let sheet = parse_stylesheet_with_options(
        "div { color: red; width: 10px; } .skip { display: grid; future: value; --x: y; }",
        &ParseOptions::stylesheet(),
    );
    let input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &sheet,
        StylesheetConditionInput::None,
    );
    let limits = StyleResolutionLimits::default();
    let collection = RuleCollection::try_new(&[input], &limits).unwrap();
    let body = (0..256)
        .map(|index| {
            format!(
                "<div class='{}'></div>",
                if index % 2 == 0 { "skip" } else { "" }
            )
        })
        .collect::<String>();
    let dom = html::parse_document(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        html::HtmlParseOptions::default(),
    )
    .unwrap()
    .document;
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);

    let (stats, counts) = measure(
        || {
            af6_resolve_winners_for_allocation_guard(&dom, environment, &collection, &limits)
                .unwrap();
        },
        || {
            af6_resolve_winners_for_allocation_guard(&dom, environment, &collection, &limits)
                .unwrap()
        },
    );
    assert!(stats.elements >= 256);
    assert_eq!(stats.initial_capacity, property_registry().entries().len());
    assert_eq!(stats.high_water_capacity, stats.initial_capacity);
    assert_eq!(stats.capacity_growths, 0);
    eprintln!(
        "AF6 transient cascade: bytes={} allocs={} reallocs={} elements={} workspace-capacity={}",
        counts.bytes, counts.allocs, counts.reallocs, stats.elements, stats.initial_capacity
    );
    assert!(
        counts.reallocs <= stats.elements.saturating_mul(8),
        "AF6 transient winner evaluation reallocated excessively: {counts:?}"
    );
}

#[test]
fn af5_matched_rule_inputs_borrow_declarations_without_vector_copy() {
    let low_sheet =
        parse_stylesheet_with_options("div { color: red; }", &ParseOptions::stylesheet());
    let high_declarations = (0..64)
        .map(|index| format!("color: rgb-{index};"))
        .collect::<String>();
    let high_sheet = parse_stylesheet_with_options(
        &format!("div {{ {high_declarations} }}"),
        &ParseOptions::stylesheet(),
    );
    let limits = StyleResolutionLimits::default();
    let low_input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &low_sheet,
        StylesheetConditionInput::None,
    );
    let high_input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(1),
        StylesheetOrder::new(0),
        &high_sheet,
        StylesheetConditionInput::None,
    );
    let low = RuleCollection::try_new(&[low_input], &limits).expect("low collection builds");
    let high = RuleCollection::try_new(&[high_input], &limits).expect("high collection builds");
    let body = (0..128).map(|_| "<div></div>").collect::<String>();
    let dom = html::parse_document(
        &format!("<!doctype html><html><body>{body}</body></html>"),
        html::HtmlParseOptions::default(),
    )
    .expect("allocation fixture parses");
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);

    let ((low_rules, low_declarations), low_counts) = measure(
        || {
            af5_match_rule_inputs_for_allocation_guard(&dom.document, environment, &low, &limits)
                .expect("warm low matching");
        },
        || {
            af5_match_rule_inputs_for_allocation_guard(&dom.document, environment, &low, &limits)
                .expect("low matching")
        },
    );
    let ((high_rules, high_declarations), high_counts) = measure(
        || {
            af5_match_rule_inputs_for_allocation_guard(&dom.document, environment, &high, &limits)
                .expect("warm high matching");
        },
        || {
            af5_match_rule_inputs_for_allocation_guard(&dom.document, environment, &high, &limits)
                .expect("high matching")
        },
    );
    assert_eq!(low_rules, high_rules);
    assert_eq!(low_declarations, 128);
    assert_eq!(high_declarations, 128 * 64);
    eprintln!(
        "AF5 borrowed matching: low-bytes={} high-bytes={} low-allocs={} high-allocs={}",
        low_counts.bytes, high_counts.bytes, low_counts.allocs, high_counts.allocs
    );
    assert!(
        high_counts.bytes <= low_counts.bytes.saturating_add(256),
        "borrowed declaration count changed matched-input allocation: low={} high={}",
        low_counts.bytes,
        high_counts.bytes
    );
    assert!(
        high_counts.allocs <= low_counts.allocs.saturating_add(2),
        "borrowed declaration count changed matched-input allocations: low={} high={}",
        low_counts.allocs,
        high_counts.allocs
    );
}

#[test]
fn af9_repeated_irrelevant_class_candidates_do_not_allocate_per_token() {
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);
    let artifact = af9_dependency_artifact(".target { color: red; }", environment);
    let node_id = html::internal::Id(7);
    let change = af9_attribute_change(node_id);
    let before = Vec::new();
    let small_after = vec![html::internal::unqualified_attribute("class", "irrelevant")];
    let large_after = vec![html::internal::unqualified_attribute(
        "class",
        "irrelevant ".repeat(100_000),
    )];
    let small_mutations = [DomStyleAttributeMutation::new(
        node_id,
        html::ElementNamespace::Html,
        Some(&before),
        &small_after,
    )];
    let large_mutations = [DomStyleAttributeMutation::new(
        node_id,
        html::ElementNamespace::Html,
        Some(&before),
        &large_after,
    )];

    let (small_decision, small_counts) =
        measure_af9_attribute_classification(&change, &artifact, environment, &small_mutations);
    let (large_decision, large_counts) =
        measure_af9_attribute_classification(&change, &artifact, environment, &large_mutations);

    eprintln!(
        "AF9 repeated class candidates: small={small_counts:?} large={large_counts:?} tokens=100000"
    );
    assert!(small_decision.into_plan().is_none());
    assert!(large_decision.into_plan().is_none());
    assert_eq!(small_counts.allocs, 0, "small classification allocated");
    assert_eq!(
        small_counts.bytes, 0,
        "small classification allocated bytes"
    );
    assert_eq!(small_counts.reallocs, 0, "small classification reallocated");
    assert_eq!(
        large_counts.allocs, small_counts.allocs,
        "100,000 raw tokens changed classification allocation count"
    );
    assert_eq!(
        large_counts.bytes, small_counts.bytes,
        "100,000 raw tokens changed classification allocation bytes"
    );
    assert_eq!(
        large_counts.reallocs, small_counts.reallocs,
        "100,000 raw tokens changed classification reallocation count"
    );
}

#[test]
fn af9_unchanged_large_class_on_attribute_transition_has_constant_allocations() {
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);
    let artifact = af9_dependency_artifact(".target, [title=new] { color: red; }", environment);
    let node_id = html::internal::Id(8);
    let change = af9_attribute_change(node_id);
    let small_before = vec![
        html::internal::unqualified_attribute("class", "irrelevant"),
        html::internal::unqualified_attribute("title", "old"),
    ];
    let small_after = vec![
        html::internal::unqualified_attribute("class", "irrelevant"),
        html::internal::unqualified_attribute("title", "new"),
    ];
    let large_class = "irrelevant ".repeat(100_000);
    let large_before = vec![
        html::internal::unqualified_attribute("class", large_class.clone()),
        html::internal::unqualified_attribute("title", "old"),
    ];
    let large_after = vec![
        html::internal::unqualified_attribute("class", large_class),
        html::internal::unqualified_attribute("title", "new"),
    ];
    let small_mutations = [DomStyleAttributeMutation::new(
        node_id,
        html::ElementNamespace::Html,
        Some(&small_before),
        &small_after,
    )];
    let large_mutations = [DomStyleAttributeMutation::new(
        node_id,
        html::ElementNamespace::Html,
        Some(&large_before),
        &large_after,
    )];

    let (small_decision, small_counts) =
        measure_af9_attribute_classification(&change, &artifact, environment, &small_mutations);
    let (large_decision, large_counts) =
        measure_af9_attribute_classification(&change, &artifact, environment, &large_mutations);

    eprintln!(
        "AF9 unchanged class attribute transition: small={small_counts:?} large={large_counts:?} tokens=100000"
    );
    assert!(small_decision.into_plan().is_some());
    assert!(large_decision.into_plan().is_some());
    assert_eq!(
        large_counts.allocs, small_counts.allocs,
        "an unchanged 100,000-token class changed allocation count"
    );
    assert_eq!(
        large_counts.bytes, small_counts.bytes,
        "an unchanged 100,000-token class changed allocation bytes"
    );
    assert_eq!(
        large_counts.reallocs, small_counts.reallocs,
        "an unchanged 100,000-token class changed reallocation count"
    );
    assert!(
        large_counts.allocs <= 1,
        "only the one-node suffix plan may allocate: {large_counts:?}"
    );
    assert_eq!(large_counts.reallocs, 0);
}

#[test]
fn af9_quirks_class_lookup_does_not_allocate_folded_candidates() {
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::Quirks);
    let artifact = af9_dependency_artifact(".target { color: red; }", environment);
    let node_id = html::internal::Id(9);
    let change = af9_attribute_change(node_id);
    let before = vec![html::internal::unqualified_attribute(
        "class",
        "TaRgEt unrelated-before",
    )];
    let after = vec![html::internal::unqualified_attribute(
        "class",
        "TARGET unrelated-after",
    )];
    let mutations = [DomStyleAttributeMutation::new(
        node_id,
        html::ElementNamespace::Html,
        Some(&before),
        &after,
    )];

    let (decision, counts) =
        measure_af9_attribute_classification(&change, &artifact, environment, &mutations);

    eprintln!("AF9 Quirks class candidate lookup: {counts:?}");
    assert!(decision.into_plan().is_none());
    assert_eq!(counts.allocs, 0, "Quirks candidate folding allocated");
    assert_eq!(counts.bytes, 0, "Quirks candidate folding allocated bytes");
    assert_eq!(counts.reallocs, 0, "Quirks candidate folding reallocated");
}

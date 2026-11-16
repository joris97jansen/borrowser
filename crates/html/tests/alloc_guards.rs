#![cfg(all(feature = "count-alloc", feature = "html5"))]

use html::{AtomTable, HtmlParseOptions, HtmlParser, Node, parse_document};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

// Counters are intentionally lightweight: they measure allocation/realloc events and growth
// bytes while enabled, not current live heap usage.
struct CountingAlloc;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);

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

fn reset_alloc_counts() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    REALLOC_COUNT.store(0, Ordering::Relaxed);
    ENABLED.store(false, Ordering::Relaxed);
}

fn enable_alloc_counts() {
    ENABLED.store(true, Ordering::Relaxed);
}

fn disable_alloc_counts() {
    ENABLED.store(false, Ordering::Relaxed);
}

fn alloc_counts() -> (usize, usize, usize) {
    (
        ALLOC_COUNT.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
        REALLOC_COUNT.load(Ordering::Relaxed),
    )
}

// Snapshots and accumulation stay on the stack; formatting happens only after
// the original end-to-end measurement has completed and counting is disabled.
fn record_alloc_delta(phase: &mut (usize, usize, usize), checkpoint: &mut (usize, usize, usize)) {
    let current = alloc_counts();
    phase.0 += current.0 - checkpoint.0;
    phase.1 += current.1 - checkpoint.1;
    phase.2 += current.2 - checkpoint.2;
    *checkpoint = current;
}

struct AllocGuard;

impl AllocGuard {
    fn new() -> Self {
        reset_alloc_counts();
        enable_alloc_counts();
        Self
    }
}

impl Drop for AllocGuard {
    fn drop(&mut self) {
        disable_alloc_counts();
    }
}

fn node_count(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Element { element } => 1 + element.children().iter().map(node_count).sum::<usize>(),
        Node::DocumentType { .. } => 1,
        Node::Text { .. } => 1,
        Node::Comment { .. } => 1,
        Node::ProcessingInstruction { .. } => 1,
    }
}

fn text_node_count(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } => children.iter().map(text_node_count).sum(),
        Node::Element { element } => element.children().iter().map(text_node_count).sum(),
        Node::Text { .. } => 1,
        Node::Comment { .. } | Node::DocumentType { .. } | Node::ProcessingInstruction { .. } => 0,
    }
}

fn total_text_bytes(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } => children.iter().map(total_text_bytes).sum(),
        Node::Element { element } => element.children().iter().map(total_text_bytes).sum(),
        Node::Text { text, .. } => text.len(),
        Node::Comment { .. } | Node::DocumentType { .. } | Node::ProcessingInstruction { .. } => 0,
    }
}

fn measure_one_shot_parse(input: &str) -> (html::ParseOutput, usize, usize, usize) {
    let _ = parse_document(input, HtmlParseOptions::default());
    let _guard = AllocGuard::new();
    let output = parse_document(input, HtmlParseOptions::default())
        .expect("one-shot html5 parse should work");
    let (allocs, bytes, reallocs) = alloc_counts();
    (output, allocs, bytes, reallocs)
}

fn measure_streaming_parse(
    input: &str,
    chunk_sizes: &[usize],
) -> (html::ParseOutput, usize, usize, usize, usize) {
    let mut warm =
        HtmlParser::new(HtmlParseOptions::default()).expect("warm html5 parser should initialize");
    warm.push_bytes(input.as_bytes())
        .expect("warm parse push should succeed");
    warm.finish().expect("warm parse finish should succeed");
    let _ = warm
        .into_output()
        .expect("warm parse output should materialize");

    let mut phases = [
        ("construction", (0usize, 0usize, 0usize)),
        ("push_bytes", (0, 0, 0)),
        ("pump", (0, 0, 0)),
        ("patch_drain", (0, 0, 0)),
        ("finish_and_final_drain", (0, 0, 0)),
        ("materialization", (0, 0, 0)),
    ];
    let started = Instant::now();
    let guard = AllocGuard::new();
    let mut checkpoint = alloc_counts();
    let mut parser = HtmlParser::new(HtmlParseOptions::default())
        .expect("streaming html5 parser should initialize");
    record_alloc_delta(&mut phases[0].1, &mut checkpoint);
    let bytes = input.as_bytes();
    let mut offset = 0usize;
    let mut size_index = 0usize;
    let mut drained_patches = 0usize;
    let mut nonempty_drains = 0usize;
    while offset < bytes.len() {
        let size = chunk_sizes[size_index % chunk_sizes.len()];
        let end = (offset + size).min(bytes.len());
        parser
            .push_bytes(&bytes[offset..end])
            .expect("streaming chunk push should succeed");
        record_alloc_delta(&mut phases[1].1, &mut checkpoint);
        parser.pump().expect("streaming pump should succeed");
        record_alloc_delta(&mut phases[2].1, &mut checkpoint);
        let drained = parser
            .take_patches()
            .expect("streaming patch drain should succeed")
            .len();
        drained_patches = drained_patches.saturating_add(drained);
        nonempty_drains += usize::from(drained != 0);
        record_alloc_delta(&mut phases[3].1, &mut checkpoint);
        offset = end;
        size_index += 1;
    }
    parser.finish().expect("streaming finish should succeed");
    let drained = parser
        .take_patches()
        .expect("streaming final patch drain should succeed")
        .len();
    drained_patches = drained_patches.saturating_add(drained);
    nonempty_drains += usize::from(drained != 0);
    record_alloc_delta(&mut phases[4].1, &mut checkpoint);
    let output = parser
        .into_output()
        .expect("streaming output should materialize");
    record_alloc_delta(&mut phases[5].1, &mut checkpoint);
    let (allocs, bytes, reallocs) = alloc_counts();
    drop(guard);
    let elapsed = started.elapsed();

    eprintln!(
        "streaming allocations chunks={chunk_sizes:?} pushes={size_index} nonempty_drains={nonempty_drains} patches={drained_patches}: allocs={allocs} bytes={bytes} reallocs={reallocs} elapsed_ns={}",
        elapsed.as_nanos()
    );
    let mut attributed = (0, 0, 0);
    for (name, counts) in phases {
        attributed.0 += counts.0;
        attributed.1 += counts.1;
        attributed.2 += counts.2;
        eprintln!(
            "  {name}: allocs={} bytes={} reallocs={}",
            counts.0, counts.1, counts.2
        );
    }
    assert_eq!(
        attributed,
        (allocs, bytes, reallocs),
        "streaming allocation phases must reconcile with the original total"
    );
    (output, drained_patches, allocs, bytes, reallocs)
}

#[test]
fn parse_document_rawtext_allocation_is_bounded() {
    let mut body = String::new();
    for _ in 0..500_000 {
        body.push('x');
    }
    let input = format!("<script>{}</ScRiPt>", body);

    let (output, allocs, bytes, reallocs) = measure_one_shot_parse(&input);
    assert!(
        output.parse_errors.is_empty(),
        "unexpected parse errors: {:?}",
        output.parse_errors
    );
    assert_eq!(
        text_node_count(&output.document),
        1,
        "expected one rawtext node"
    );
    assert_eq!(
        total_text_bytes(&output.document),
        body.len(),
        "expected rawtext payload to survive parsing"
    );

    let overhead = 512 * 1024;
    let max_bytes = input.len().saturating_mul(10).saturating_add(overhead);
    let max_reallocs = 256;
    let max_allocs = 10_000;
    assert!(
        bytes <= max_bytes,
        "expected bounded html5 allocations; bytes={bytes} max={max_bytes}"
    );
    assert!(
        reallocs <= max_reallocs,
        "expected bounded html5 realloc churn; reallocs={reallocs} max={max_reallocs}"
    );
    assert!(
        allocs <= max_allocs,
        "expected bounded html5 allocation events; allocs={allocs} max={max_allocs}"
    );
}

#[test]
fn parse_document_plain_text_allocation_is_bounded() {
    let mut body = String::new();
    for _ in 0..500_000 {
        body.push('x');
    }
    let input = format!("<p>{}</p>", body);

    let (output, allocs, bytes, reallocs) = measure_one_shot_parse(&input);
    assert!(
        output.parse_errors.is_empty(),
        "unexpected parse errors: {:?}",
        output.parse_errors
    );
    assert_eq!(
        text_node_count(&output.document),
        1,
        "expected one text node"
    );
    assert_eq!(
        total_text_bytes(&output.document),
        body.len(),
        "expected text payload to survive parsing"
    );

    let overhead = 512 * 1024;
    let max_bytes = input.len().saturating_mul(10).saturating_add(overhead);
    let max_reallocs = 256;
    let max_allocs = 10_000;
    assert!(
        bytes <= max_bytes,
        "expected bounded html5 allocations; bytes={bytes} max={max_bytes}"
    );
    assert!(
        reallocs <= max_reallocs,
        "expected bounded html5 realloc churn; reallocs={reallocs} max={max_reallocs}"
    );
    assert!(
        allocs <= max_allocs,
        "expected bounded html5 allocation events; allocs={allocs} max={max_allocs}"
    );
}

#[test]
fn parse_document_attribute_values_avoid_unnecessary_allocs() {
    let plain = "<p data=TomJerry title=plain>ok</p>";
    let encoded = "<p data=Tom&amp;Jerry title=&#x3C;ok&#x3E;>ok</p>";

    let (plain_output, allocs_plain, bytes_plain, reallocs_plain) = measure_one_shot_parse(plain);
    let (encoded_output, allocs_encoded, bytes_encoded, reallocs_encoded) =
        measure_one_shot_parse(encoded);

    assert!(plain_output.parse_errors.is_empty());
    assert!(encoded_output.parse_errors.is_empty());

    let overhead = 32 * 1024;
    let max_allocs = 10_000;
    assert!(
        bytes_plain <= bytes_encoded.saturating_add(overhead),
        "expected plain attrs to avoid excess allocation bytes; plain={bytes_plain} encoded={bytes_encoded} overhead={overhead}"
    );
    assert!(
        allocs_plain <= allocs_encoded.saturating_add(256),
        "expected plain attrs to avoid excess alloc churn; plain={allocs_plain} encoded={allocs_encoded}"
    );
    assert!(
        reallocs_plain <= reallocs_encoded.saturating_add(64),
        "expected plain attrs to avoid excess realloc churn; plain={reallocs_plain} encoded={reallocs_encoded}"
    );
    assert!(
        allocs_plain <= max_allocs,
        "expected bounded allocation events for plain attrs; allocs={allocs_plain} max={max_allocs}"
    );
}

#[test]
fn streaming_parser_chunk_sizes_do_not_cause_pathological_alloc_growth() {
    let input = "<div><span>hi</span></div>".repeat(2_000);

    let (small_output, small_drained, allocs_small, bytes_small, reallocs_small) =
        measure_streaming_parse(&input, &[1]);
    let (large_output, large_drained, allocs_large, bytes_large, reallocs_large) =
        measure_streaming_parse(&input, &[1_024]);

    assert!(small_output.parse_errors.is_empty());
    assert!(large_output.parse_errors.is_empty());
    assert_eq!(
        node_count(&small_output.document),
        node_count(&large_output.document),
        "chunk size should not change html5 DOM shape"
    );
    assert_eq!(
        small_drained + small_output.patches.len(),
        large_drained + large_output.patches.len(),
        "chunk size should not change total emitted patch count"
    );
    assert_eq!(
        small_output.counters, large_output.counters,
        "chunk size should not change html5 parser counters"
    );

    let byte_slack = 256 * 1024;
    assert!(
        bytes_small <= bytes_large.saturating_mul(2).saturating_add(byte_slack),
        "small chunks caused pathological allocation growth: small={bytes_small} large={bytes_large}"
    );
    assert!(
        allocs_small <= allocs_large.saturating_mul(2).saturating_add(512),
        "small chunks caused pathological alloc churn: small={allocs_small} large={allocs_large}"
    );
    assert!(
        reallocs_small <= reallocs_large.saturating_add(512),
        "small chunks caused pathological realloc churn: small={reallocs_small} large={reallocs_large}"
    );
}

#[test]
fn atom_intern_reuse_does_not_allocate() {
    let mut atoms = AtomTable::new();
    let first = atoms.intern_ascii_lowercase("div");
    let _guard = AllocGuard::new();
    let (a1, b1, r1) = alloc_counts();
    let second = atoms.intern_ascii_lowercase("div");
    let (a2, b2, r2) = alloc_counts();

    assert_eq!(first, second);
    assert_eq!(a1, a2, "expected no extra alloc events on reuse");
    assert_eq!(b1, b2, "expected no extra alloc bytes on reuse");
    assert_eq!(r1, r2, "expected no extra reallocs on reuse");
}

#[test]
fn atom_lookup_path_does_not_allocate() {
    let mut atoms = AtomTable::new();
    atoms.intern_ascii_lowercase("div");

    let _guard = AllocGuard::new();
    let (a1, b1, r1) = alloc_counts();
    let _ = atoms.intern_ascii_lowercase("div");
    let (a2, b2, r2) = alloc_counts();

    assert_eq!((a1, b1, r1), (a2, b2, r2));
}

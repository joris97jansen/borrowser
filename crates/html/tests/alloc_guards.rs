#![cfg(feature = "count-alloc")]

use html::{AtomTable, Token, TokenStream, tokenize};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

fn text_eq(stream: &TokenStream, token: &Token, expected: &str) -> bool {
    stream.text(token) == Some(expected)
}

fn expected_tokenizer_bytes(input_len: usize) -> usize {
    let token_capacity = (input_len / 8).saturating_add(16);
    let text_pool_capacity = (input_len / 64).saturating_add(4);
    let token_bytes = token_capacity.saturating_mul(std::mem::size_of::<Token>());
    let text_pool_bytes = text_pool_capacity.saturating_mul(std::mem::size_of::<String>());
    input_len
        .saturating_add(token_bytes)
        .saturating_add(text_pool_bytes)
}

#[test]
fn tokenize_rawtext_allocation_is_bounded() {
    let mut body = String::new();
    for _ in 0..500_000 {
        body.push('x');
    }
    let input = format!("<script>{}</ScRiPt>", body);

    let _ = tokenize(&input);
    let _guard = AllocGuard::new();
    let stream = tokenize(&input);
    let atoms = stream.atoms();
    let (allocs, bytes, reallocs) = alloc_counts();

    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, text, &body)
                    && atoms.resolve(*end) == "script"
        ),
        "expected rawtext body to tokenize correctly, got: {stream:?}"
    );

    let overhead = 256 * 1024;
    let expected_bytes = expected_tokenizer_bytes(input.len());
    let max_reallocs = 128;
    let max_allocs = 50_000;
    assert!(
        bytes <= expected_bytes.saturating_add(overhead),
        "expected bounded allocations; bytes={bytes} expected={expected_bytes} overhead={overhead}"
    );
    assert!(
        reallocs <= max_reallocs,
        "expected bounded realloc churn; reallocs={reallocs} max={max_reallocs}"
    );
    assert!(
        allocs <= max_allocs,
        "expected bounded allocation events; allocs={allocs} max={max_allocs}"
    );
}

#[test]
fn tokenize_plain_text_allocation_is_bounded() {
    let mut body = String::new();
    for _ in 0..500_000 {
        body.push('x');
    }
    let input = format!("<p>{}</p>", body);

    let _ = tokenize(&input);
    let _guard = AllocGuard::new();
    let stream = tokenize(&input);
    let atoms = stream.atoms();
    let (allocs, bytes, reallocs) = alloc_counts();

    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                if atoms.resolve(*name) == "p"
                    && text_eq(&stream, text, &body)
                    && atoms.resolve(*end) == "p"
        ),
        "expected plain text to tokenize correctly, got: {stream:?}"
    );

    let overhead = 256 * 1024;
    let expected_bytes = expected_tokenizer_bytes(input.len());
    let max_reallocs = 128;
    let max_allocs = 50_000;
    assert!(
        bytes <= expected_bytes.saturating_add(overhead),
        "expected bounded allocations; bytes={bytes} expected={expected_bytes} overhead={overhead}"
    );
    assert!(
        reallocs <= max_reallocs,
        "expected bounded realloc churn; reallocs={reallocs} max={max_reallocs}"
    );
    assert!(
        allocs <= max_allocs,
        "expected bounded allocation events; allocs={allocs} max={max_allocs}"
    );
}

#[test]
fn tokenize_attribute_values_avoid_unnecessary_allocs() {
    fn measure(input: &str) -> (usize, usize, usize) {
        let _ = tokenize(input);
        let _guard = AllocGuard::new();
        let _ = tokenize(input);
        alloc_counts()
    }

    let plain = "<p data=Tom&Jerry title=plain>ok</p>";
    let encoded = "<p data=Tom&amp;Jerry title=&#x3C;ok&#x3E;>ok</p>";

    let (allocs_plain, bytes_plain, reallocs_plain) = measure(plain);
    let (allocs_encoded, _bytes_encoded, reallocs_encoded) = measure(encoded);
    let overhead = 64 * 1024;
    let baseline = plain.len();
    let max_allocs = 10_000;

    assert!(
        bytes_plain <= baseline + overhead,
        "expected bounded allocations for plain attrs; bytes={bytes_plain} input_len={baseline} overhead={overhead}"
    );
    assert!(
        allocs_plain <= max_allocs,
        "expected bounded allocation events for plain attrs; allocs={allocs_plain} max={max_allocs}"
    );
    assert!(
        allocs_plain <= allocs_encoded + 256,
        "expected plain attrs to avoid excess alloc churn; plain={allocs_plain} encoded={allocs_encoded}"
    );
    assert!(
        reallocs_plain <= 128,
        "expected bounded realloc churn for plain attrs; reallocs={reallocs_plain}"
    );
    assert!(
        reallocs_plain <= reallocs_encoded + 64,
        "expected plain attrs to avoid excess realloc churn; plain={reallocs_plain} encoded={reallocs_encoded}"
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

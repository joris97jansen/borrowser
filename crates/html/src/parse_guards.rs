use std::sync::atomic::{AtomicU64, Ordering};

static FULL_PARSE_ENTRY_CALLS: AtomicU64 = AtomicU64::new(0);
static FULL_PARSE_OUTPUT_CALLS: AtomicU64 = AtomicU64::new(0);
static DOM_MATERIALIZE_CALLS: AtomicU64 = AtomicU64::new(0);
static DOM_SNAPSHOT_COMPARE_CALLS: AtomicU64 = AtomicU64::new(0);
static DOM_DIFF_CALLS: AtomicU64 = AtomicU64::new(0);
static TOKENS_PROCESSED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseGuardCounts {
    pub full_parse_entry_calls: u64,
    pub full_parse_output_calls: u64,
    pub dom_materialize_calls: u64,
    pub dom_snapshot_compare_calls: u64,
    pub dom_diff_calls: u64,
    pub tokens_processed: u64,
}

pub fn reset() {
    FULL_PARSE_ENTRY_CALLS.store(0, Ordering::Relaxed);
    FULL_PARSE_OUTPUT_CALLS.store(0, Ordering::Relaxed);
    DOM_MATERIALIZE_CALLS.store(0, Ordering::Relaxed);
    DOM_SNAPSHOT_COMPARE_CALLS.store(0, Ordering::Relaxed);
    DOM_DIFF_CALLS.store(0, Ordering::Relaxed);
    TOKENS_PROCESSED.store(0, Ordering::Relaxed);
}

pub fn record_full_parse_entry() {
    FULL_PARSE_ENTRY_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_full_parse_output() {
    FULL_PARSE_OUTPUT_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_dom_materialize() {
    DOM_MATERIALIZE_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_dom_snapshot_compare() {
    DOM_SNAPSHOT_COMPARE_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_dom_diff() {
    DOM_DIFF_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_token_processed() {
    TOKENS_PROCESSED.fetch_add(1, Ordering::Relaxed);
}

pub fn counts() -> ParseGuardCounts {
    ParseGuardCounts {
        full_parse_entry_calls: FULL_PARSE_ENTRY_CALLS.load(Ordering::Relaxed),
        full_parse_output_calls: FULL_PARSE_OUTPUT_CALLS.load(Ordering::Relaxed),
        dom_materialize_calls: DOM_MATERIALIZE_CALLS.load(Ordering::Relaxed),
        dom_snapshot_compare_calls: DOM_SNAPSHOT_COMPARE_CALLS.load(Ordering::Relaxed),
        dom_diff_calls: DOM_DIFF_CALLS.load(Ordering::Relaxed),
        tokens_processed: TOKENS_PROCESSED.load(Ordering::Relaxed),
    }
}

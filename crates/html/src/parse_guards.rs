use std::sync::atomic::{AtomicU64, Ordering};

static FULL_TOKENIZE_CALLS: AtomicU64 = AtomicU64::new(0);
static FULL_BUILD_DOM_CALLS: AtomicU64 = AtomicU64::new(0);
static DOM_SNAPSHOT_COMPARE_CALLS: AtomicU64 = AtomicU64::new(0);
static DOM_DIFF_CALLS: AtomicU64 = AtomicU64::new(0);
static TOKENS_PROCESSED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseGuardCounts {
    pub full_tokenize_calls: u64,
    pub full_dom_build_calls: u64,
    pub dom_snapshot_compare_calls: u64,
    pub dom_diff_calls: u64,
    pub tokens_processed: u64,
}

pub fn reset() {
    FULL_TOKENIZE_CALLS.store(0, Ordering::Relaxed);
    FULL_BUILD_DOM_CALLS.store(0, Ordering::Relaxed);
    DOM_SNAPSHOT_COMPARE_CALLS.store(0, Ordering::Relaxed);
    DOM_DIFF_CALLS.store(0, Ordering::Relaxed);
    TOKENS_PROCESSED.store(0, Ordering::Relaxed);
}

pub fn record_full_tokenize() {
    FULL_TOKENIZE_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_full_build_dom() {
    FULL_BUILD_DOM_CALLS.fetch_add(1, Ordering::Relaxed);
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
        full_tokenize_calls: FULL_TOKENIZE_CALLS.load(Ordering::Relaxed),
        full_dom_build_calls: FULL_BUILD_DOM_CALLS.load(Ordering::Relaxed),
        dom_snapshot_compare_calls: DOM_SNAPSHOT_COMPARE_CALLS.load(Ordering::Relaxed),
        dom_diff_calls: DOM_DIFF_CALLS.load(Ordering::Relaxed),
        tokens_processed: TOKENS_PROCESSED.load(Ordering::Relaxed),
    }
}

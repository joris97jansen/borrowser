//! Document-level parse context (shared resources).

use super::{AtomTable, Counters, ErrorPolicy, ParseError};
use std::collections::VecDeque;

/// Document-level parse context shared by tokenizer and tree builder.
///
/// Owns document-lifetime resources such as atom tables, policies, and metrics.
#[derive(Debug)]
pub struct DocumentParseContext {
    pub atoms: AtomTable,
    pub counters: Counters,
    pub error_policy: ErrorPolicy,
    pub errors: Option<VecDeque<ParseError>>,
}

impl Default for DocumentParseContext {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentParseContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            atoms: AtomTable::default(),
            counters: Counters::default(),
            error_policy: ErrorPolicy::default(),
            errors: None,
        };
        if ctx.error_policy.track
            && ctx.error_policy.max_stored != 0
            && (!ctx.error_policy.debug_only || cfg!(debug_assertions))
        {
            ctx.errors = Some(VecDeque::new());
        }
        ctx
    }

    /// Record a recoverable parse error. Never panics on malformed input.
    pub fn record_error(&mut self, error: ParseError) {
        if self.error_policy.track_counters {
            self.counters.parse_errors = self.counters.parse_errors.saturating_add(1);
        }
        if !self.error_policy.track {
            return;
        }
        if self.error_policy.debug_only && !cfg!(debug_assertions) {
            return;
        }
        if self.error_policy.max_stored == 0 {
            return;
        }
        let errors = self.errors.get_or_insert_with(VecDeque::new);
        if errors.len() >= self.error_policy.max_stored {
            errors.pop_front();
            self.counters.errors_dropped = self.counters.errors_dropped.saturating_add(1);
        }
        errors.push_back(error);
    }
}

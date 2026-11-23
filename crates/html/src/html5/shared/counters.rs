//! Parser accounting. Decode categories are authoritative; their aggregate is derived.

use tools::utf8::Utf8DecodeIssueKind;

/// Replacement events, independent of optional canonical observation retention.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Utf8DecodeCounts {
    pub invalid_sequences: u64,
    pub incomplete_sequences_at_eof: u64,
}

impl Utf8DecodeCounts {
    pub fn total(self) -> u64 {
        self.invalid_sequences
            .saturating_add(self.incomplete_sequences_at_eof)
    }

    pub(crate) fn record(&mut self, kind: Utf8DecodeIssueKind) {
        let count = match kind {
            Utf8DecodeIssueKind::InvalidSequence => &mut self.invalid_sequences,
            Utf8DecodeIssueKind::IncompleteSequenceAtEof => &mut self.incomplete_sequences_at_eof,
        };
        *count = count.saturating_add(1);
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.invalid_sequences = self
            .invalid_sequences
            .saturating_add(other.invalid_sequences);
        self.incomplete_sequences_at_eof = self
            .incomplete_sequences_at_eof
            .saturating_add(other.incomplete_sequences_at_eof);
    }
}

/// Evidence at an existing parser terminal boundary, also readable after failure.
/// An empty decoder carry or zero replacements alone does not establish EOF.
#[cfg(any(test, feature = "internal-api"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlDecodeReport {
    pub eof_processed: bool,
    pub replacements: Utf8DecodeCounts,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub tokens_processed: u64,
    pub patches_emitted: u64,
    /// Decode errors (input/encoding issues), not engine invariants.
    pub(crate) utf8_decode: Utf8DecodeCounts,
    /// Adapter-level invariant violations (e.g., patch stream contract breaches).
    pub adapter_invariant_violations: u64,
    /// Tree builder invariant errors (engine bugs/corruption).
    pub tree_builder_invariant_errors: u64,
    /// Every genuine recoverable HTML parse error detected by the tokenizer or
    /// tree builder, independent of whether either observation surface retains
    /// an event.
    pub parse_errors: u64,
    /// Events evicted only from the bounded exact-position legacy deque.
    ///
    /// Canonical observation-capacity drops are reported by their canonical
    /// collection and do not change this counter.
    pub errors_dropped: u64,
    /// Max observed depth of the stack of open elements (session lifetime).
    pub max_open_elements_depth: u32,
    /// Max observed depth of the active formatting list (session lifetime).
    pub max_active_formatting_depth: u32,
    /// Total stack-of-open-elements push operations observed.
    pub soe_push_ops: u64,
    /// Total explicit stack-of-open-elements removals observed.
    /// Excludes SOE reset operations performed via `clear()`.
    pub soe_pop_ops: u64,
    /// Number of in-scope scans performed (probe-only and mutating paths).
    pub soe_scope_scan_calls: u64,
    /// Total SOE elements visited while evaluating in-scope scans.
    pub soe_scope_scan_steps: u64,
    pub soe_name_count_lookup_calls: u64,
    pub soe_name_count_lookup_steps: u64,
    pub soe_name_count_update_calls: u64,
    pub soe_name_count_update_steps: u64,
    pub soe_distinct_name_high_water: u32,
    /// Total patches emitted by the tree builder.
    pub tree_builder_patches_emitted: u64,
    /// Number of text-node creation patches emitted.
    pub tree_builder_text_nodes_created: u64,
    /// Number of append-text coalescing patches emitted.
    pub tree_builder_text_appends: u64,
    /// Number of times text coalescing state was invalidated by structural edits.
    pub tree_builder_text_coalescing_invalidations: u64,
}

impl Counters {
    /// Derived aggregate; the stable facade retains its `decode_errors` field.
    pub fn decode_errors(&self) -> u64 {
        self.utf8_decode.total()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aj4_aggregate_preserves_saturating_accounting_without_a_stored_total() {
        let mut counts = Utf8DecodeCounts::default();
        counts.record(Utf8DecodeIssueKind::InvalidSequence);
        counts.record(Utf8DecodeIssueKind::IncompleteSequenceAtEof);
        assert_eq!(counts.total(), 2);
        counts.merge(Utf8DecodeCounts {
            invalid_sequences: u64::MAX,
            incomplete_sequences_at_eof: 3,
        });
        assert_eq!(counts.invalid_sequences, u64::MAX);
        assert_eq!(counts.incomplete_sequences_at_eof, 4);
        assert_eq!(counts.total(), u64::MAX);
    }
}

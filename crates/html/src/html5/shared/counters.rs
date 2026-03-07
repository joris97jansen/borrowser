//! Optional counters for instrumentation.

#[derive(Clone, Debug, Default)]
pub struct Counters {
    pub tokens_processed: u64,
    pub patches_emitted: u64,
    /// Decode errors (input/encoding issues), not engine invariants.
    pub decode_errors: u64,
    /// Adapter-level invariant violations (e.g., patch stream contract breaches).
    pub adapter_invariant_violations: u64,
    /// Tree builder invariant errors (engine bugs/corruption).
    pub tree_builder_invariant_errors: u64,
    /// Recoverable HTML parse errors (spec-defined), not engine invariants.
    /// Tokenizer/tree builder will increment this once error reporting is wired.
    pub parse_errors: u64,
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
    /// Total patches emitted by the tree builder.
    pub tree_builder_patches_emitted: u64,
    /// Number of text-node creation patches emitted.
    pub tree_builder_text_nodes_created: u64,
    /// Number of append-text coalescing patches emitted.
    pub tree_builder_text_appends: u64,
    /// Number of times text coalescing state was invalidated by structural edits.
    pub tree_builder_text_coalescing_invalidations: u64,
}

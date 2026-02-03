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
}

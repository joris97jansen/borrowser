//! Optional counters for instrumentation.

#[derive(Clone, Debug, Default)]
pub struct Counters {
    pub tokens_emitted: u64,
    pub parse_errors: u64,
}

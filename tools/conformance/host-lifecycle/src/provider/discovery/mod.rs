//! Bounded, pure operation discovery. These in-memory results grant no authority.
mod completeness;
mod inputs;
mod policy_count;
pub mod preflight;
pub mod relationship_agreement;
pub mod relationships;
pub mod report_budget;
pub mod representation;
pub mod requirements;
pub mod scheduling;
pub use completeness::*;
#[cfg(test)]
pub(crate) mod test_support;
pub use inputs::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountingSnapshot {
    pub requests: u64,
    pub source_occurrences: u64,
    pub retained_outputs: u64,
    pub response_bytes: u64,
    pub normalized_bytes: u64,
    pub failure: Option<super::limits::LimitKind>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalRoundCheck {
    Pending,
    Passed,
    Failed(super::limits::LimitKind),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinatorStop {
    Deriving,
    Quiescent,
    MetadataExhausted,
    RoundStopped,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedRead {
    pub query: super::reviewed_subnet_routes_v1::DiscoveryQuery,
    pub reason: super::coverage::ReadFailureV1,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryExecutionFacts {
    pub accounting: AccountingSnapshot,
    pub rejected_before_admission: Vec<RejectedRead>,
    pub stop: CoordinatorStop,
    pub final_check: FinalRoundCheck,
}

mod accounting;
mod model;
mod projection;
mod runner;

pub use accounting::{
    AggregateAccounting, AggregateGroupingAccounting, AggregateVariantPopulationCounts,
    LogicalHeadlineCounts, TerminalOutcomeCounts,
};
pub use model::{
    AggregateCaseResult, AggregateComparisonKind, AggregateExecutionAttempt,
    AggregateExecutionRequest, AggregateExecutionVariantId, AggregateNotAttemptedReason,
    AggregateRun, AggregateSubsystemResult, AggregateTerminalOutcome, AggregateVariantKey,
    AggregateVariantResult, LaneSelection,
};
pub use runner::{AggregateReconciliationError, AggregateRunError, run_repository_aggregate};

mod advisory;
pub use advisory::*;
mod accounting;
mod external_registry;
mod identity;
mod model;
mod projection;
mod report;
mod runner;

pub use accounting::{
    AggregateAccounting, AggregateGroupingAccounting, AggregateVariantPopulationCounts,
    LogicalHeadlineCounts, TerminalOutcomeCounts,
};
pub use external_registry::{
    AdvisoryTrackId, BaselineNoteId, ComparableObservationSurface,
    ExternalRegistryAttachmentSubjectKey, ExternalRegistryDiagnostic,
    ExternalRegistryDiagnosticComponent, ExternalRegistryDiagnosticDetail,
    ExternalRegistryDiagnosticField, ExternalRegistryDiagnosticKind,
    ExternalRegistryDiagnosticSubjectKey, ExternalRegistryRecordCollection,
    ExternalRegistryTrackInvariantField, ExternalRegistryValidationPhase, ReconciledBaselineNote,
    ReconciledExternalAdvisoryEvidence, ReconciledExternalAttachment, StoredValidatedCapture,
    ValidatedAdvisoryTrack, load_repository_external_advisory_evidence,
};
pub use identity::{
    AGGREGATE_LOGICAL_CASE_MEMBER_IDENTITY_V1, AGGREGATE_LOGICAL_CASE_SOURCE_SET_IDENTITY_V1,
    AggregateIdentityError, AggregateLogicalCaseMemberDigest, AggregateLogicalCaseSourceSetDigest,
    AggregateLogicalSourceIdentity,
};
pub use model::{
    AggregateCaseResult, AggregateComparisonKind, AggregateEnvironmentAssessmentMode,
    AggregateExecutionAttempt, AggregateExecutionRequest, AggregateExecutionVariantId,
    AggregateNotAttemptedReason, AggregateRenderingCaseEvidence, AggregateRun,
    AggregateRunInvariantError, AggregateSubsystemResult, AggregateTerminalOutcome,
    AggregateVariantKey, AggregateVariantResult, LaneSelection,
};
pub use report::{
    AGGREGATE_DETAIL_FORMAT_V1, AGGREGATE_DETAIL_MAX_BYTES_V1, AGGREGATE_GRANULARITY_CONTRACT_V1,
    AGGREGATE_POPULATION_IDENTITY_CONTRACT_V1, AGGREGATE_SUMMARY_FORMAT_V1,
    AGGREGATE_SUMMARY_MAX_BYTES_V1, AggregateReportBuildError, AggregateReportPublicationError,
    build_aggregate_detail_v1, build_aggregate_summary_v1, build_and_write_aggregate_detail_v1,
    build_and_write_aggregate_summary_v1,
};
pub use runner::{AggregateReconciliationError, AggregateRunError, run_repository_aggregate};

mod advisory;
mod baseline;
mod binary_wire;
mod bounds;
pub use advisory::*;
mod accounting;
mod external_registry;
mod identity;
mod model;
mod projection;
mod report;
mod runner;
mod trend;

pub use accounting::{
    AggregateAccounting, AggregateGroupingAccounting, AggregateVariantPopulationCounts,
    LogicalHeadlineCounts, TerminalOutcomeCounts,
};
pub use baseline::{
    BASELINE_FORMAT_V1, BASELINE_MAX_BYTES_V1, BaselineError, BaselineSealError,
    SealedBaselineEvidence, build_and_write_baseline_v1, build_baseline_v1,
    seal_baseline_from_selected_operation, seal_baseline_without_evaluation,
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
pub use trend::{
    BaselineFileInputV1, ConformanceTrendV1, PopulationTrend, TREND_FORMAT_V1, TREND_MAX_BYTES_V1,
    TREND_VERSIONS_V1, TrendChangeKind, TrendCompatibilityField, TrendError, TrendInputSide,
    TrendPopulation, TrendPopulationCounts, TrendRecord, build_and_write_trend_v1, build_trend_v1,
    compare_baseline_files_and_build_trend_v1, compare_baseline_files_v1,
};

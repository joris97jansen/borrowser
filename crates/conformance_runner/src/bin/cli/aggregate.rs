use super::{
    arguments::{AggregateCommand, BaselineInput, CheckPolicy, LaneRequest},
    publication::PreparedArtifact,
};
use conformance_runner::*;
use conformance_test_support::ObservationSurface;
use std::path::Path;

#[derive(Debug)]
pub(super) enum Failure {
    Execution(AggregateRunError),
    Registry(ExternalRegistryDiagnostic),
    SelectedOperation(SelectedDomOperationError),
    Selection(DomObservationFailure),
    TrendInput(TrendError),
    Seal(BaselineSealError),
    Report(AggregateReportBuildError),
    Baseline(BaselineError),
    TrendBuild(TrendError),
    Output(std::io::Error),
}
impl Failure {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Execution(_)
            | Self::Registry(_)
            | Self::SelectedOperation(_)
            | Self::Selection(_)
            | Self::TrendInput(_) => 3,
            Self::Seal(_)
            | Self::Report(_)
            | Self::Baseline(_)
            | Self::TrendBuild(_)
            | Self::Output(_) => 4,
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Execution(e) => write!(f, "aggregate execution failed: {e}"),
            Self::Registry(e) => write!(f, "aggregate evidence preparation failed: {e:?}"),
            Self::SelectedOperation(e) => write!(f, "aggregate selected operation failed: {e:?}"),
            Self::Selection(e) => write!(
                f,
                "aggregate selected identity is unavailable or unsupported: {e:?}"
            ),
            Self::TrendInput(e) => write!(f, "aggregate trend input/comparison failed: {e}"),
            Self::Seal(e) => write!(f, "aggregate baseline sealing failed: {e:?}"),
            Self::Report(e) => write!(f, "aggregate report construction failed: {e:?}"),
            Self::Baseline(e) => write!(f, "aggregate baseline construction failed: {e}"),
            Self::TrendBuild(e) => write!(f, "aggregate trend construction failed: {e}"),
            Self::Output(e) => write!(f, "aggregate stdout publication failed: {e}"),
        }
    }
}

fn request(request: LaneRequest) -> AggregateExecutionRequest {
    AggregateExecutionRequest { lane: request.lane }
}
fn policy_failed(run: &AggregateRun, check: CheckPolicy) -> bool {
    matches!(check, CheckPolicy::RequireExpected)
        && run
            .cases()
            .iter()
            .flat_map(|case| &case.variants)
            .any(|variant| variant.policy.is_unexpected())
}
fn input(value: &BaselineInput) -> BaselineFileInputV1<'_> {
    BaselineFileInputV1 {
        root: &value.root,
        relative_path: &value.relative_path,
        expected_sha256: value.expected_sha256,
    }
}

#[allow(
    clippy::result_large_err,
    reason = "Preserve closed typed registry diagnostics without allocating while reporting failure"
)]
pub(super) fn prepare(root: &Path, command: AggregateCommand) -> Result<PreparedArtifact, Failure> {
    let (bytes, policy_failed) = match command {
        AggregateCommand::Summary(lane) => {
            let run = run_repository_aggregate(root, request(lane)).map_err(Failure::Execution)?;
            (
                build_aggregate_summary_v1(&run).map_err(Failure::Report)?,
                policy_failed(&run, lane.check),
            )
        }
        AggregateCommand::Detail(lane) => {
            let run = run_repository_aggregate(root, request(lane)).map_err(Failure::Execution)?;
            (
                build_aggregate_detail_v1(&run).map_err(Failure::Report)?,
                policy_failed(&run, lane.check),
            )
        }
        AggregateCommand::BaselineWithoutEvaluation(lane) => {
            let run = run_repository_aggregate(root, request(lane)).map_err(Failure::Execution)?;
            let evidence = load_repository_external_advisory_evidence(root, &run)
                .map_err(Failure::Registry)?;
            let sealed =
                seal_baseline_without_evaluation(&run, &evidence).map_err(Failure::Seal)?;
            (
                build_baseline_v1(&sealed).map_err(Failure::Baseline)?,
                policy_failed(&run, lane.check),
            )
        }
        AggregateCommand::BaselineSelectedDom {
            request: lane,
            test_id,
        } => {
            let selected = SelectedDomOperationRequest {
                selected: AggregateVariantKey {
                    test_id,
                    observation: ObservationSurface::DomTree,
                    variant: AggregateExecutionVariantId::Singleton(ExecutionVariantId::new(
                        SingletonExecutionVariant::Singleton,
                    )),
                },
            };
            let operation =
                run_repository_aggregate_for_selected_dom_operation(root, request(lane), selected)
                    .map_err(Failure::Execution)?;
            // Reject identity errors only. NotAttempted and preparation failures
            // remain AG9c evidence and cannot change ordinary lane execution.
            match operation.observation() {
                Err(
                    e @ (DomObservationFailure::UnknownVariant
                    | DomObservationFailure::UnsupportedSelection),
                ) => return Err(Failure::Selection(e)),
                Ok(_)
                | Err(
                    DomObservationFailure::NotAttempted
                    | DomObservationFailure::Preparation(_)
                    | DomObservationFailure::DuplicateHandoff,
                ) => {}
            }
            let evidence = operation
                .compare_external(root)
                .map_err(Failure::SelectedOperation)?;
            let sealed = seal_baseline_from_selected_operation(&evidence).map_err(Failure::Seal)?;
            (
                build_baseline_v1(&sealed).map_err(Failure::Baseline)?,
                policy_failed(operation.run(), lane.check),
            )
        }
        AggregateCommand::Trend { from, to } => {
            let trend =
                compare_baseline_files_v1(input(&from), input(&to)).map_err(Failure::TrendInput)?;
            (build_trend_v1(&trend).map_err(Failure::TrendBuild)?, false)
        }
    };
    Ok(PreparedArtifact {
        bytes,
        policy_failed,
    })
}

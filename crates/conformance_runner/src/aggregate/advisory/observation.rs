use super::model::*;
use crate::html_parser::ParserEvaluationObserver;
use crate::{
    AggregateExecutionRequest, AggregateExecutionVariantId, AggregateRunError,
    AggregateSubsystemResult, NormalizedCaseResult,
};
use conformance_test_support::ObservationSurface;
use external_test_provenance::sha256;
use html_test_support::parser_fixture::{
    FixtureEvaluation, ParserTargetKind, ScriptingMode, ValidatedFixtureSpec,
};
use std::path::Path;

pub fn run_repository_aggregate_for_selected_dom_operation(
    root: &Path,
    request: AggregateExecutionRequest,
    selected: SelectedDomOperationRequest,
) -> Result<SelectedDomOperationRun, AggregateRunError> {
    let mut observer = Observer {
        request: selected,
        observation: Err(DomObservationFailure::NotAttempted),
        seen: false,
    };
    let run =
        super::super::runner::run_repository_aggregate_observing(root, request, &mut observer)?;
    let variant = run
        .cases()
        .iter()
        .flat_map(|case| &case.variants)
        .find(|v| v.key == observer.request.selected);
    match variant {
        None => observer.observation = Err(DomObservationFailure::UnknownVariant),
        Some(v)
            if !matches!(v.subsystem, AggregateSubsystemResult::Parser(_))
                || v.key.observation != ObservationSurface::DomTree =>
        {
            observer.observation = Err(DomObservationFailure::UnsupportedSelection)
        }
        Some(_) => {}
    }
    Ok(SelectedDomOperationRun {
        run,
        selected: observer.request.selected,
        observation: observer.observation,
    })
}
struct Observer {
    request: SelectedDomOperationRequest,
    observation: Result<ProducedObservation, DomObservationFailure>,
    seen: bool,
}
impl ParserEvaluationObserver for Observer {
    fn observe(
        &mut self,
        case: &NormalizedCaseResult,
        fixture: &ValidatedFixtureSpec,
        evaluation: &FixtureEvaluation,
    ) {
        let key = &self.request.selected;
        if key.test_id != case.ag.test_id
            || key.observation != case.ag.observation
            || key.variant != AggregateExecutionVariantId::Singleton(case.variant.clone())
        {
            return;
        }
        if self.seen {
            self.observation = Err(DomObservationFailure::DuplicateHandoff);
            return;
        }
        self.seen = true;
        if key.observation != ObservationSurface::DomTree
            || fixture.target_kind() != ParserTargetKind::Document
            || fixture.scripting_mode() != Some(ScriptingMode::Disabled)
        {
            self.observation = Err(DomObservationFailure::UnsupportedSelection);
            return;
        }
        self.observation = evaluation
            .serialize_web_observable_dom_tree_v1()
            .map(|bytes| ProducedObservation {
                bytes,
                fixture_sha256: sha256(fixture.input_bytes()),
            })
            .map_err(DomObservationFailure::Preparation);
    }
}

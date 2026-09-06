use std::collections::BTreeMap;

use conformance_test_support::{ObservationSurface, SubsystemOwner};

use crate::{
    AgExpectation, CapabilityAvailability, ClassificationCompleteness, Eligibility, Stability,
};

use super::model::{
    AggregateAttemptProjection, AggregateEligibilityProjection, AggregateSelectionProjection,
    attempt_projection, selection_projection,
};
use super::{AggregateCaseResult, AggregateComparisonKind, AggregateTerminalOutcome};
#[cfg(test)]
use super::{AggregateExecutionAttempt, LaneSelection};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LogicalExecutionProjection {
    pub pass: bool,
    pub fail: bool,
    pub excluded_only: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogicalHeadlineCounts {
    pub total_tests: u64,
    pub pass_count: u64,
    pub fail_count: u64,
    pub expected_fail_count: u64,
    pub unsupported_count: u64,
    pub skipped_count: u64,
    pub flaky_count: u64,
    pub unclassified_count: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AggregateVariantPopulationCounts {
    pub materialized_variants: u64,
    pub runnable_variants: u64,
    pub not_runnable_variants: u64,
    pub eligibility_not_established_variants: u64,
    pub selected_variants: u64,
    pub excluded_variants: u64,
    pub selection_not_applicable_variants: u64,
    pub attempted_variants: u64,
    pub not_attempted_variants: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalOutcomeCounts {
    pub semantic_pass: u64,
    pub semantic_fail: u64,
    pub execution_failure: u64,
    pub resource_failure: u64,
    pub incomplete_observation: u64,
    pub invariant_failure: u64,
    pub timeout: u64,
}

impl TerminalOutcomeCounts {
    fn increment(&mut self, outcome: AggregateTerminalOutcome) -> Result<(), AccountingError> {
        let count = match outcome {
            AggregateTerminalOutcome::SemanticPass => &mut self.semantic_pass,
            AggregateTerminalOutcome::SemanticFail => &mut self.semantic_fail,
            AggregateTerminalOutcome::ExecutionFailure => &mut self.execution_failure,
            AggregateTerminalOutcome::ResourceFailure => &mut self.resource_failure,
            AggregateTerminalOutcome::IncompleteObservation => &mut self.incomplete_observation,
            AggregateTerminalOutcome::InvariantFailure => &mut self.invariant_failure,
            AggregateTerminalOutcome::Timeout => &mut self.timeout,
        };
        increment(count)
    }

    fn checked_total(&self) -> Result<u64, AccountingError> {
        checked_sum([
            self.semantic_pass,
            self.semantic_fail,
            self.execution_failure,
            self.resource_failure,
            self.incomplete_observation,
            self.invariant_failure,
            self.timeout,
        ])
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AggregateGroupingAccounting {
    pub logical_cases_by_subsystem: BTreeMap<SubsystemOwner, u64>,
    pub variants_by_subsystem: BTreeMap<SubsystemOwner, u64>,
    pub logical_cases_by_observation: BTreeMap<ObservationSurface, u64>,
    pub variants_by_observation: BTreeMap<ObservationSurface, u64>,
    pub variants_by_comparison: BTreeMap<AggregateComparisonKind, u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AggregateAccounting {
    pub logical: LogicalHeadlineCounts,
    pub variants: AggregateVariantPopulationCounts,
    pub terminals: TerminalOutcomeCounts,
    pub groupings: AggregateGroupingAccounting,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FixedAggregateAccountingProjection {
    pub logical: LogicalHeadlineCounts,
    pub variants: AggregateVariantPopulationCounts,
    pub terminals: TerminalOutcomeCounts,
    pub owners: [[u64; 2]; 5],
    pub surfaces: [[u64; 2]; 10],
    pub comparisons: [u64; 5],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AccountingError {
    Overflow,
    Invariant(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) struct LogicalCaseAccountingProjection {
    pub owner: SubsystemOwner,
    pub observation: ObservationSurface,
    pub eligibility: AggregateEligibilityProjection,
    pub expected_fail: bool,
    pub unsupported: bool,
    pub flaky: bool,
    pub unclassified: bool,
}

pub(crate) struct VariantAccountingProjection {
    pub comparison: AggregateComparisonKind,
    pub selection: AggregateSelectionProjection,
    pub attempt: AggregateAttemptProjection,
}

pub(crate) fn build_accounting(
    cases: &[AggregateCaseResult],
) -> Result<AggregateAccounting, AccountingError> {
    let mut accounting = AggregateAccounting::default();
    for case in cases {
        accumulate_case_projection(
            &mut accounting,
            LogicalCaseAccountingProjection {
                owner: case.owner,
                observation: case.ag.observation,
                eligibility: eligibility_projection(&case.ag.eligibility),
                expected_fail: is_expected_fail(&case.ag),
                unsupported: is_unsupported(&case.ag),
                flaky: is_flaky(&case.ag),
                unclassified: is_unclassified(&case.ag),
            },
            case.variants
                .iter()
                .map(|variant| VariantAccountingProjection {
                    comparison: variant.comparison.clone(),
                    selection: selection_projection(&variant.selection),
                    attempt: attempt_projection(&variant.execution),
                }),
        )?;
    }
    validate_accounting(&accounting)?;
    Ok(accounting)
}

fn eligibility_projection(value: &Eligibility) -> AggregateEligibilityProjection {
    match value {
        Eligibility::Runnable => AggregateEligibilityProjection::Runnable,
        Eligibility::NotRunnable { .. } => AggregateEligibilityProjection::NotRunnable,
        Eligibility::NotYetEstablished { .. } => AggregateEligibilityProjection::NotYetEstablished,
    }
}

pub(crate) fn accumulate_case_projection<T: ProjectionAccountingTarget>(
    accounting: &mut T,
    case: LogicalCaseAccountingProjection,
    variants: impl IntoIterator<Item = VariantAccountingProjection>,
) -> Result<(), AccountingError> {
    increment(&mut accounting.logical_mut().total_tests)?;
    accounting.increment_logical_owner(case.owner)?;
    accounting.increment_logical_surface(case.observation)?;

    let mut execution = LogicalExecutionAccumulator::default();
    for variant in variants {
        execution.observe(variant.selection, variant.attempt);
        increment(&mut accounting.variants_mut().materialized_variants)?;
        accounting.increment_variant_owner(case.owner)?;
        accounting.increment_variant_surface(case.observation)?;
        accounting.increment_variant_comparison(&variant.comparison)?;

        match case.eligibility {
            AggregateEligibilityProjection::Runnable => {
                increment(&mut accounting.variants_mut().runnable_variants)?
            }
            AggregateEligibilityProjection::NotRunnable => {
                increment(&mut accounting.variants_mut().not_runnable_variants)?
            }
            AggregateEligibilityProjection::NotYetEstablished => increment(
                &mut accounting
                    .variants_mut()
                    .eligibility_not_established_variants,
            )?,
        }
        match variant.selection {
            AggregateSelectionProjection::Selected => {
                increment(&mut accounting.variants_mut().selected_variants)?
            }
            AggregateSelectionProjection::Excluded => {
                increment(&mut accounting.variants_mut().excluded_variants)?
            }
            AggregateSelectionProjection::NotApplicable => {
                increment(&mut accounting.variants_mut().selection_not_applicable_variants)?
            }
        }
        match variant.attempt {
            AggregateAttemptProjection::Attempted(outcome) => {
                increment(&mut accounting.variants_mut().attempted_variants)?;
                accounting.terminals_mut().increment(outcome)?;
            }
            AggregateAttemptProjection::NotAttempted(_) => {
                increment(&mut accounting.variants_mut().not_attempted_variants)?
            }
        }
    }
    let execution = execution.finish();
    if execution.pass {
        increment(&mut accounting.logical_mut().pass_count)?;
    }
    if execution.fail {
        increment(&mut accounting.logical_mut().fail_count)?;
    }
    if case.expected_fail {
        increment(&mut accounting.logical_mut().expected_fail_count)?;
    }
    if case.unsupported {
        increment(&mut accounting.logical_mut().unsupported_count)?;
    }
    if case.eligibility == AggregateEligibilityProjection::Runnable && execution.excluded_only {
        increment(&mut accounting.logical_mut().skipped_count)?;
    }
    if case.flaky {
        increment(&mut accounting.logical_mut().flaky_count)?;
    }
    if case.unclassified {
        increment(&mut accounting.logical_mut().unclassified_count)?;
    }
    Ok(())
}

pub(crate) trait ProjectionAccountingTarget {
    fn logical_mut(&mut self) -> &mut LogicalHeadlineCounts;
    fn variants_mut(&mut self) -> &mut AggregateVariantPopulationCounts;
    fn terminals_mut(&mut self) -> &mut TerminalOutcomeCounts;
    fn increment_logical_owner(&mut self, owner: SubsystemOwner) -> Result<(), AccountingError>;
    fn increment_variant_owner(&mut self, owner: SubsystemOwner) -> Result<(), AccountingError>;
    fn increment_logical_surface(
        &mut self,
        surface: ObservationSurface,
    ) -> Result<(), AccountingError>;
    fn increment_variant_surface(
        &mut self,
        surface: ObservationSurface,
    ) -> Result<(), AccountingError>;
    fn increment_variant_comparison(
        &mut self,
        comparison: &AggregateComparisonKind,
    ) -> Result<(), AccountingError>;
}

impl ProjectionAccountingTarget for AggregateAccounting {
    fn logical_mut(&mut self) -> &mut LogicalHeadlineCounts {
        &mut self.logical
    }

    fn variants_mut(&mut self) -> &mut AggregateVariantPopulationCounts {
        &mut self.variants
    }

    fn terminals_mut(&mut self) -> &mut TerminalOutcomeCounts {
        &mut self.terminals
    }

    fn increment_logical_owner(&mut self, owner: SubsystemOwner) -> Result<(), AccountingError> {
        increment_map(&mut self.groupings.logical_cases_by_subsystem, owner)
    }

    fn increment_variant_owner(&mut self, owner: SubsystemOwner) -> Result<(), AccountingError> {
        increment_map(&mut self.groupings.variants_by_subsystem, owner)
    }

    fn increment_logical_surface(
        &mut self,
        surface: ObservationSurface,
    ) -> Result<(), AccountingError> {
        increment_map(&mut self.groupings.logical_cases_by_observation, surface)
    }

    fn increment_variant_surface(
        &mut self,
        surface: ObservationSurface,
    ) -> Result<(), AccountingError> {
        increment_map(&mut self.groupings.variants_by_observation, surface)
    }

    fn increment_variant_comparison(
        &mut self,
        comparison: &AggregateComparisonKind,
    ) -> Result<(), AccountingError> {
        increment_map(
            &mut self.groupings.variants_by_comparison,
            comparison.clone(),
        )
    }
}

impl ProjectionAccountingTarget for FixedAggregateAccountingProjection {
    fn logical_mut(&mut self) -> &mut LogicalHeadlineCounts {
        &mut self.logical
    }

    fn variants_mut(&mut self) -> &mut AggregateVariantPopulationCounts {
        &mut self.variants
    }

    fn terminals_mut(&mut self) -> &mut TerminalOutcomeCounts {
        &mut self.terminals
    }

    fn increment_logical_owner(&mut self, owner: SubsystemOwner) -> Result<(), AccountingError> {
        increment(&mut self.owners[owner_index(owner)][0])
    }

    fn increment_variant_owner(&mut self, owner: SubsystemOwner) -> Result<(), AccountingError> {
        increment(&mut self.owners[owner_index(owner)][1])
    }

    fn increment_logical_surface(
        &mut self,
        surface: ObservationSurface,
    ) -> Result<(), AccountingError> {
        increment(&mut self.surfaces[surface_index(surface)][0])
    }

    fn increment_variant_surface(
        &mut self,
        surface: ObservationSurface,
    ) -> Result<(), AccountingError> {
        increment(&mut self.surfaces[surface_index(surface)][1])
    }

    fn increment_variant_comparison(
        &mut self,
        comparison: &AggregateComparisonKind,
    ) -> Result<(), AccountingError> {
        increment(&mut self.comparisons[comparison_index(comparison)])
    }
}

const fn owner_index(owner: SubsystemOwner) -> usize {
    match owner {
        SubsystemOwner::HtmlParser => 0,
        SubsystemOwner::Css => 1,
        SubsystemOwner::Layout => 2,
        SubsystemOwner::Paint => 3,
        SubsystemOwner::BrowserRuntime => 4,
    }
}

const fn surface_index(surface: ObservationSurface) -> usize {
    match surface {
        ObservationSurface::HtmlTokenizer => 0,
        ObservationSurface::HtmlTreeConstruction => 1,
        ObservationSurface::DomTree => 2,
        ObservationSurface::CssParsing => 3,
        ObservationSurface::CssSelectors => 4,
        ObservationSurface::CssCascade => 5,
        ObservationSurface::ComputedStyle => 6,
        ObservationSurface::LayoutGeometry => 7,
        ObservationSurface::PaintOperations => 8,
        ObservationSurface::BrowserRuntimeSemantic => 9,
    }
}

const fn comparison_index(comparison: &AggregateComparisonKind) -> usize {
    match comparison {
        AggregateComparisonKind::AuthoredExpectedObservation => 0,
        AggregateComparisonKind::StaticDocumentReference {
            reference_kind: conformance_test_support::ReferenceKind::Semantic,
            relation: conformance_test_support::ReferenceRelation::Match,
        } => 1,
        AggregateComparisonKind::StaticDocumentReference {
            reference_kind: conformance_test_support::ReferenceKind::Semantic,
            relation: conformance_test_support::ReferenceRelation::Mismatch,
        } => 2,
        AggregateComparisonKind::StaticDocumentReference {
            reference_kind: conformance_test_support::ReferenceKind::Structural,
            relation: conformance_test_support::ReferenceRelation::Match,
        } => 3,
        AggregateComparisonKind::StaticDocumentReference {
            reference_kind: conformance_test_support::ReferenceKind::Structural,
            relation: conformance_test_support::ReferenceRelation::Mismatch,
        } => 4,
    }
}

fn is_expected_fail(ag: &crate::AgCaseState) -> bool {
    matches!(ag.expectation, AgExpectation::ExpectedFail { .. })
}

fn is_unsupported(ag: &crate::AgCaseState) -> bool {
    matches!(
        ag.capability,
        Some(CapabilityAvailability::Unavailable { .. })
    )
}

fn is_flaky(ag: &crate::AgCaseState) -> bool {
    matches!(ag.stability, Some(Stability::Flaky { .. }))
}

fn is_unclassified(ag: &crate::AgCaseState) -> bool {
    matches!(
        ag.classification,
        ClassificationCompleteness::NotYetClassified { .. }
    )
}

#[cfg(test)]
fn logical_pass_states<'a>(
    states: impl IntoIterator<Item = (&'a LaneSelection, &'a AggregateExecutionAttempt)>,
) -> bool {
    logical_execution_projection(states.into_iter().map(|(selection, execution)| {
        (
            selection_projection(selection),
            attempt_projection(execution),
        )
    }))
    .pass
}

#[cfg(test)]
fn logical_fail_states<'a>(
    states: impl IntoIterator<Item = (&'a LaneSelection, &'a AggregateExecutionAttempt)>,
) -> bool {
    logical_execution_projection(states.into_iter().map(|(selection, execution)| {
        (
            selection_projection(selection),
            attempt_projection(execution),
        )
    }))
    .fail
}

#[cfg(test)]
fn logical_skipped_states<'a>(
    states: impl IntoIterator<Item = (&'a LaneSelection, &'a AggregateExecutionAttempt)>,
) -> bool {
    logical_execution_projection(states.into_iter().map(|(selection, execution)| {
        (
            selection_projection(selection),
            attempt_projection(execution),
        )
    }))
    .excluded_only
}

#[cfg(test)]
pub(crate) fn logical_execution_projection(
    states: impl IntoIterator<Item = (AggregateSelectionProjection, AggregateAttemptProjection)>,
) -> LogicalExecutionProjection {
    let mut accumulator = LogicalExecutionAccumulator::default();
    for (selection, attempt) in states {
        accumulator.observe(selection, attempt);
    }
    accumulator.finish()
}

struct LogicalExecutionAccumulator {
    selected: bool,
    all_selected_pass: bool,
    fail: bool,
    excluded: bool,
}

impl Default for LogicalExecutionAccumulator {
    fn default() -> Self {
        Self {
            selected: false,
            all_selected_pass: true,
            fail: false,
            excluded: false,
        }
    }
}

impl LogicalExecutionAccumulator {
    fn observe(
        &mut self,
        selection: AggregateSelectionProjection,
        attempt: AggregateAttemptProjection,
    ) {
        match selection {
            AggregateSelectionProjection::Selected => {
                self.selected = true;
                self.all_selected_pass &= matches!(
                    attempt,
                    AggregateAttemptProjection::Attempted(AggregateTerminalOutcome::SemanticPass)
                );
                self.fail |= matches!(
                    attempt,
                    AggregateAttemptProjection::Attempted(AggregateTerminalOutcome::SemanticFail)
                );
            }
            AggregateSelectionProjection::Excluded => self.excluded = true,
            AggregateSelectionProjection::NotApplicable => {}
        }
    }

    fn finish(self) -> LogicalExecutionProjection {
        LogicalExecutionProjection {
            pass: self.selected && self.all_selected_pass,
            fail: self.fail,
            excluded_only: self.excluded && !self.selected,
        }
    }
}

pub(crate) fn validate_accounting(accounting: &AggregateAccounting) -> Result<(), AccountingError> {
    validate_count_reconciliation(&accounting.variants, &accounting.terminals)
}

pub(crate) fn validate_fixed_accounting(
    accounting: &FixedAggregateAccountingProjection,
) -> Result<(), AccountingError> {
    validate_count_reconciliation(&accounting.variants, &accounting.terminals)
}

fn validate_count_reconciliation(
    variants: &AggregateVariantPopulationCounts,
    terminals: &TerminalOutcomeCounts,
) -> Result<(), AccountingError> {
    validate_projection_reconciliation(
        variants.materialized_variants,
        [
            variants.runnable_variants,
            variants.not_runnable_variants,
            variants.eligibility_not_established_variants,
        ],
        [
            variants.selected_variants,
            variants.excluded_variants,
            variants.selection_not_applicable_variants,
        ],
        [variants.attempted_variants, variants.not_attempted_variants],
        terminals.checked_total()?,
    )
}

pub(crate) fn validate_projection_reconciliation(
    materialized: u64,
    eligibility: [u64; 3],
    selection: [u64; 3],
    attempt: [u64; 2],
    terminal_total: u64,
) -> Result<(), AccountingError> {
    if checked_sum(eligibility)? != materialized {
        return Err(AccountingError::Invariant(
            "materialized variants do not reconcile with eligibility populations",
        ));
    }
    if checked_sum([selection[0], selection[1]])? != eligibility[0] {
        return Err(AccountingError::Invariant(
            "runnable variants do not reconcile with named-lane selection",
        ));
    }
    if selection[2] != checked_sum([eligibility[1], eligibility[2]])? {
        return Err(AccountingError::Invariant(
            "not-applicable selection does not reconcile with ineligible variants",
        ));
    }
    if checked_sum(attempt)? != materialized {
        return Err(AccountingError::Invariant(
            "attempt state does not reconcile with materialized variants",
        ));
    }
    if terminal_total != attempt[0] {
        return Err(AccountingError::Invariant(
            "terminal outcomes do not reconcile with attempted variants",
        ));
    }
    Ok(())
}

fn increment(value: &mut u64) -> Result<(), AccountingError> {
    *value = value.checked_add(1).ok_or(AccountingError::Overflow)?;
    Ok(())
}

fn increment_map<K: Ord>(map: &mut BTreeMap<K, u64>, key: K) -> Result<(), AccountingError> {
    increment(map.entry(key).or_default())
}

fn checked_sum(values: impl IntoIterator<Item = u64>) -> Result<u64, AccountingError> {
    values.into_iter().try_fold(0_u64, |total, value| {
        total.checked_add(value).ok_or(AccountingError::Overflow)
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use conformance_test_support::{
        ExpectedFailureClassification, LanePolicyScope, ObservationSurface, TestId,
    };

    use crate::{
        AgCaseState, AgExpectation, CapabilityAvailability, ClassificationCompleteness,
        Eligibility, HarnessReadiness, Stability,
    };

    use super::*;

    fn selected(outcome: AggregateTerminalOutcome) -> (LaneSelection, AggregateExecutionAttempt) {
        (
            LaneSelection::Selected {
                lane: LanePolicyScope::NormalCi,
            },
            AggregateExecutionAttempt::Attempted { outcome },
        )
    }

    fn selected_not_attempted() -> (LaneSelection, AggregateExecutionAttempt) {
        (
            LaneSelection::Selected {
                lane: LanePolicyScope::NormalCi,
            },
            AggregateExecutionAttempt::NotAttempted {
                reason: crate::AggregateNotAttemptedReason::ParserPreAttemptEvaluation,
            },
        )
    }

    fn excluded() -> (LaneSelection, AggregateExecutionAttempt) {
        (
            LaneSelection::Excluded {
                lane: LanePolicyScope::NormalCi,
                reason: "synthetic exclusion".to_owned(),
            },
            AggregateExecutionAttempt::NotAttempted {
                reason: crate::AggregateNotAttemptedReason::LaneExcluded,
            },
        )
    }

    fn pass_fail(states: &[(LaneSelection, AggregateExecutionAttempt)]) -> (bool, bool) {
        let view = || {
            states
                .iter()
                .map(|(selection, attempt)| (selection, attempt))
        };
        (logical_pass_states(view()), logical_fail_states(view()))
    }

    #[test]
    fn logical_pass_and_fail_truth_table_is_exact() {
        assert_eq!(
            pass_fail(&[selected(AggregateTerminalOutcome::SemanticPass)]),
            (true, false)
        );
        assert_eq!(
            pass_fail(&[
                selected(AggregateTerminalOutcome::SemanticPass),
                selected(AggregateTerminalOutcome::SemanticPass),
            ]),
            (true, false)
        );
        assert_eq!(
            pass_fail(&[
                selected(AggregateTerminalOutcome::SemanticPass),
                selected_not_attempted(),
            ]),
            (false, false)
        );
        assert_eq!(
            pass_fail(&[
                selected(AggregateTerminalOutcome::SemanticPass),
                selected(AggregateTerminalOutcome::SemanticFail),
            ]),
            (false, true)
        );
        assert_eq!(
            pass_fail(&[
                selected(AggregateTerminalOutcome::SemanticPass),
                selected(AggregateTerminalOutcome::ExecutionFailure),
            ]),
            (false, false)
        );
        assert_eq!(pass_fail(&[]), (false, false));
        assert_eq!(pass_fail(&[excluded()]), (false, false));
        let excluded = [excluded()];
        assert!(logical_skipped_states(
            excluded
                .iter()
                .map(|(selection, attempt)| (selection, attempt))
        ));
    }

    #[test]
    fn checked_accounting_arithmetic_rejects_overflow() {
        assert_eq!(checked_sum([u64::MAX, 1]), Err(AccountingError::Overflow));
        let mut maximum = u64::MAX;
        assert_eq!(increment(&mut maximum), Err(AccountingError::Overflow));
        assert_eq!(maximum, u64::MAX);
    }

    #[test]
    fn fixed_historical_storage_uses_the_shared_projection_without_heap_fields() {
        fn case() -> LogicalCaseAccountingProjection {
            LogicalCaseAccountingProjection {
                owner: SubsystemOwner::Css,
                observation: ObservationSurface::CssParsing,
                eligibility: AggregateEligibilityProjection::Runnable,
                expected_fail: false,
                unsupported: false,
                flaky: false,
                unclassified: false,
            }
        }
        fn variants() -> [VariantAccountingProjection; 2] {
            [
                VariantAccountingProjection {
                    comparison: AggregateComparisonKind::AuthoredExpectedObservation,
                    selection: AggregateSelectionProjection::Selected,
                    attempt: AggregateAttemptProjection::Attempted(
                        AggregateTerminalOutcome::SemanticPass,
                    ),
                },
                VariantAccountingProjection {
                    comparison: AggregateComparisonKind::StaticDocumentReference {
                        reference_kind: conformance_test_support::ReferenceKind::Semantic,
                        relation: conformance_test_support::ReferenceRelation::Match,
                    },
                    selection: AggregateSelectionProjection::Excluded,
                    attempt: AggregateAttemptProjection::NotAttempted(
                        crate::AggregateNotAttemptedReason::LaneExcluded,
                    ),
                },
            ]
        }

        let mut live = AggregateAccounting::default();
        accumulate_case_projection(&mut live, case(), variants()).unwrap();
        let mut fixed = FixedAggregateAccountingProjection::default();
        accumulate_case_projection(&mut fixed, case(), variants()).unwrap();

        assert_eq!(fixed.logical, live.logical);
        assert_eq!(fixed.variants, live.variants);
        assert_eq!(fixed.terminals, live.terminals);
        assert_eq!(fixed.owners[owner_index(SubsystemOwner::Css)], [1, 2]);
        assert_eq!(
            fixed.surfaces[surface_index(ObservationSurface::CssParsing)],
            [1, 2]
        );
        assert_eq!(fixed.comparisons, [1, 1, 0, 0, 0]);
        validate_accounting(&live).unwrap();
        validate_fixed_accounting(&fixed).unwrap();
        assert!(
            !std::mem::needs_drop::<FixedAggregateAccountingProjection>(),
            "the closed historical accounting projection must not own heap storage"
        );
    }

    fn ag_state() -> AgCaseState {
        AgCaseState {
            test_id: TestId::parse("synthetic-accounting").unwrap(),
            observation: ObservationSurface::CssParsing,
            classification: ClassificationCompleteness::Classified,
            requirements: vec![],
            capability: Some(CapabilityAvailability::Available),
            harness: Some(HarnessReadiness::Ready),
            environment_requirements: vec![],
            stability: Some(Stability::Stable),
            lane_exclusions: vec![],
            eligibility: Eligibility::Runnable,
            expectation: AgExpectation::ExpectedPass,
        }
    }

    #[test]
    fn headline_metadata_dimensions_are_orthogonal() {
        let mut ag = ag_state();
        ag.expectation = AgExpectation::ExpectedFail {
            failure: ExpectedFailureClassification::SemanticMismatch,
            reason: "known mismatch".to_owned(),
        };
        assert!(is_expected_fail(&ag));
        assert!(logical_fail_states(
            [selected(AggregateTerminalOutcome::SemanticFail)]
                .iter()
                .map(|(selection, attempt)| (selection, attempt))
        ));
        assert!(!is_unsupported(&ag));

        ag.stability = Some(Stability::Flaky {
            reason: "historical instability".to_owned(),
        });
        assert!(is_flaky(&ag));
        assert!(is_expected_fail(&ag));

        ag.capability = Some(CapabilityAvailability::Unavailable { missing: vec![] });
        assert!(is_unsupported(&ag));
        ag.capability = Some(CapabilityAvailability::Available);
        ag.harness = Some(HarnessReadiness::NotReady {
            limitations: vec![],
        });
        assert!(!is_unsupported(&ag));
        ag.eligibility = Eligibility::NotYetEstablished { unresolved: vec![] };
        assert!(!is_unsupported(&ag));

        ag.classification = ClassificationCompleteness::NotYetClassified {
            reason: "classification pending".to_owned(),
        };
        assert!(is_unclassified(&ag));
        assert!(!is_unsupported(&ag));

        assert!(
            is_expected_fail(&ag),
            "expectation is independent of policy"
        );
    }

    #[test]
    fn accounting_counts_excluded_only_and_expected_failure_as_overlapping_projections() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let run = crate::run_repository_aggregate(
            root,
            crate::AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap();
        let mut case = run
            .cases()
            .iter()
            .find(|case| {
                matches!(case.ag.eligibility, Eligibility::Runnable) && !case.variants.is_empty()
            })
            .unwrap()
            .clone();

        for variant in &mut case.variants {
            variant.selection = LaneSelection::Excluded {
                lane: LanePolicyScope::NormalCi,
                reason: "synthetic accounting exclusion".to_owned(),
            };
            variant.execution = AggregateExecutionAttempt::NotAttempted {
                reason: crate::AggregateNotAttemptedReason::LaneExcluded,
            };
        }
        let skipped = build_accounting(&[case.clone()]).unwrap();
        assert_eq!(skipped.logical.skipped_count, 1);
        assert_eq!(skipped.logical.pass_count, 0);
        assert_eq!(skipped.logical.fail_count, 0);
        assert_eq!(skipped.variants.selected_variants, 0);
        assert_eq!(
            skipped.variants.excluded_variants,
            skipped.variants.runnable_variants
        );

        case.ag.expectation = AgExpectation::ExpectedFail {
            failure: ExpectedFailureClassification::SemanticMismatch,
            reason: "known mismatch".to_owned(),
        };
        case.ag.stability = Some(Stability::Flaky {
            reason: "historical instability".to_owned(),
        });
        for variant in &mut case.variants {
            variant.selection = LaneSelection::Selected {
                lane: LanePolicyScope::NormalCi,
            };
            variant.execution = AggregateExecutionAttempt::Attempted {
                outcome: AggregateTerminalOutcome::SemanticFail,
            };
        }
        let failing = build_accounting(&[case]).unwrap();
        assert_eq!(failing.logical.fail_count, 1);
        assert_eq!(failing.logical.expected_fail_count, 1);
        assert_eq!(failing.logical.flaky_count, 1);
        assert_eq!(failing.logical.pass_count, 0);
    }
}

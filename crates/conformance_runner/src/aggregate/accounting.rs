use std::collections::BTreeMap;

use conformance_test_support::{ObservationSurface, SubsystemOwner};

use crate::{
    AgExpectation, CapabilityAvailability, ClassificationCompleteness, Eligibility, Stability,
};

use super::{
    AggregateCaseResult, AggregateComparisonKind, AggregateExecutionAttempt,
    AggregateTerminalOutcome, LaneSelection,
};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AccountingError {
    Overflow,
    Invariant(&'static str),
}

pub(crate) fn build_accounting(
    cases: &[AggregateCaseResult],
) -> Result<AggregateAccounting, AccountingError> {
    let mut accounting = AggregateAccounting::default();
    for case in cases {
        increment(&mut accounting.logical.total_tests)?;
        increment_map(
            &mut accounting.groupings.logical_cases_by_subsystem,
            case.owner,
        )?;
        increment_map(
            &mut accounting.groupings.logical_cases_by_observation,
            case.ag.observation,
        )?;

        if logical_pass(case) {
            increment(&mut accounting.logical.pass_count)?;
        }
        if logical_fail(case) {
            increment(&mut accounting.logical.fail_count)?;
        }
        if is_expected_fail(&case.ag) {
            increment(&mut accounting.logical.expected_fail_count)?;
        }
        if is_unsupported(&case.ag) {
            increment(&mut accounting.logical.unsupported_count)?;
        }
        if logical_skipped(case) {
            increment(&mut accounting.logical.skipped_count)?;
        }
        if is_flaky(&case.ag) {
            increment(&mut accounting.logical.flaky_count)?;
        }
        if is_unclassified(&case.ag) {
            increment(&mut accounting.logical.unclassified_count)?;
        }

        for variant in &case.variants {
            increment(&mut accounting.variants.materialized_variants)?;
            increment_map(&mut accounting.groupings.variants_by_subsystem, case.owner)?;
            increment_map(
                &mut accounting.groupings.variants_by_observation,
                case.ag.observation,
            )?;
            increment_map(
                &mut accounting.groupings.variants_by_comparison,
                variant.comparison,
            )?;

            match case.ag.eligibility {
                Eligibility::Runnable => increment(&mut accounting.variants.runnable_variants)?,
                Eligibility::NotRunnable { .. } => {
                    increment(&mut accounting.variants.not_runnable_variants)?
                }
                Eligibility::NotYetEstablished { .. } => {
                    increment(&mut accounting.variants.eligibility_not_established_variants)?
                }
            }
            match variant.selection {
                LaneSelection::NotApplicable => {
                    increment(&mut accounting.variants.selection_not_applicable_variants)?
                }
                LaneSelection::Selected { .. } => {
                    increment(&mut accounting.variants.selected_variants)?
                }
                LaneSelection::Excluded { .. } => {
                    increment(&mut accounting.variants.excluded_variants)?
                }
            }
            match variant.execution {
                AggregateExecutionAttempt::NotAttempted { .. } => {
                    increment(&mut accounting.variants.not_attempted_variants)?
                }
                AggregateExecutionAttempt::Attempted { outcome } => {
                    increment(&mut accounting.variants.attempted_variants)?;
                    accounting.terminals.increment(outcome)?;
                }
            }
        }
    }
    validate_accounting(&accounting)?;
    Ok(accounting)
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

fn logical_pass(case: &AggregateCaseResult) -> bool {
    logical_pass_states(
        case.variants
            .iter()
            .map(|variant| (&variant.selection, &variant.execution)),
    )
}

fn logical_fail(case: &AggregateCaseResult) -> bool {
    logical_fail_states(
        case.variants
            .iter()
            .map(|variant| (&variant.selection, &variant.execution)),
    )
}

fn logical_skipped(case: &AggregateCaseResult) -> bool {
    matches!(case.ag.eligibility, Eligibility::Runnable)
        && logical_skipped_states(
            case.variants
                .iter()
                .map(|variant| (&variant.selection, &variant.execution)),
        )
}

fn logical_pass_states<'a>(
    states: impl IntoIterator<Item = (&'a LaneSelection, &'a AggregateExecutionAttempt)>,
) -> bool {
    let mut selected = false;
    for (selection, execution) in states {
        if matches!(selection, LaneSelection::Selected { .. }) {
            selected = true;
            if !matches!(
                execution,
                AggregateExecutionAttempt::Attempted {
                    outcome: AggregateTerminalOutcome::SemanticPass
                }
            ) {
                return false;
            }
        }
    }
    selected
}

fn logical_fail_states<'a>(
    states: impl IntoIterator<Item = (&'a LaneSelection, &'a AggregateExecutionAttempt)>,
) -> bool {
    states.into_iter().any(|(selection, execution)| {
        matches!(selection, LaneSelection::Selected { .. })
            && matches!(
                execution,
                AggregateExecutionAttempt::Attempted {
                    outcome: AggregateTerminalOutcome::SemanticFail
                }
            )
    })
}

fn logical_skipped_states<'a>(
    states: impl IntoIterator<Item = (&'a LaneSelection, &'a AggregateExecutionAttempt)>,
) -> bool {
    let mut excluded = false;
    for (selection, _) in states {
        match selection {
            LaneSelection::Selected { .. } => return false,
            LaneSelection::Excluded { .. } => excluded = true,
            LaneSelection::NotApplicable => {}
        }
    }
    excluded
}

fn validate_accounting(accounting: &AggregateAccounting) -> Result<(), AccountingError> {
    if checked_sum([
        accounting.variants.runnable_variants,
        accounting.variants.not_runnable_variants,
        accounting.variants.eligibility_not_established_variants,
    ])? != accounting.variants.materialized_variants
    {
        return Err(AccountingError::Invariant(
            "materialized variants do not reconcile with eligibility populations",
        ));
    }
    if checked_sum([
        accounting.variants.selected_variants,
        accounting.variants.excluded_variants,
    ])? != accounting.variants.runnable_variants
    {
        return Err(AccountingError::Invariant(
            "runnable variants do not reconcile with named-lane selection",
        ));
    }
    if accounting.variants.selection_not_applicable_variants
        != checked_sum([
            accounting.variants.not_runnable_variants,
            accounting.variants.eligibility_not_established_variants,
        ])?
    {
        return Err(AccountingError::Invariant(
            "not-applicable selection does not reconcile with ineligible variants",
        ));
    }
    if checked_sum([
        accounting.variants.attempted_variants,
        accounting.variants.not_attempted_variants,
    ])? != accounting.variants.materialized_variants
    {
        return Err(AccountingError::Invariant(
            "attempt state does not reconcile with materialized variants",
        ));
    }
    if accounting.terminals.checked_total()? != accounting.variants.attempted_variants {
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

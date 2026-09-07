//! Historical policy validation consumes selection as well as eligibility.
//! Live subsystem policy remains authoritative and is not changed here.
use crate::aggregate::model::{
    AggregateAttemptProjection as Attempt, AggregateEligibilityProjection as Eligibility,
    AggregateNotAttemptedReason as Reason, AggregateSelectionProjection as Selection,
    AggregateTerminalOutcome as Terminal, valid_selection_attempt_projection,
};
use crate::model::{
    DerivedPolicyResult, ObservedPolicyClass, PolicyEligibilityClass, PolicyExpectationClass,
    derive_policy_from_projection,
};

pub(super) fn historical_policy(
    eligibility: Eligibility,
    selection: Selection,
    attempt: Attempt,
    expectation: PolicyExpectationClass,
) -> Option<DerivedPolicyResult> {
    // Reject every impossible tuple before projecting policy. In particular,
    // exclusion cannot hide an attempt or a selected pre-attempt failure.
    if !valid_selection_attempt_projection(eligibility, selection, attempt) {
        return None;
    }
    let observed = match attempt {
        Attempt::NotAttempted(Reason::LaneExcluded) => {
            // The shared invariant proves Runnable + Excluded + LaneExcluded.
            return Some(DerivedPolicyResult::NotRun);
        }
        Attempt::NotAttempted(
            Reason::Eligibility
            | Reason::ParserPreAttemptEvaluation
            | Reason::CssFragmentCapabilityUnavailable,
        ) => None,
        Attempt::Attempted(Terminal::SemanticPass) => Some(ObservedPolicyClass::SemanticPass),
        Attempt::Attempted(Terminal::SemanticFail) => Some(ObservedPolicyClass::SemanticMismatch),
        Attempt::Attempted(
            Terminal::ExecutionFailure
            | Terminal::ResourceFailure
            | Terminal::IncompleteObservation
            | Terminal::InvariantFailure
            | Terminal::Timeout,
        ) => Some(ObservedPolicyClass::OtherTerminalOutcome),
    };
    let eligibility = match eligibility {
        Eligibility::Runnable => PolicyEligibilityClass::Runnable,
        Eligibility::NotRunnable => PolicyEligibilityClass::NotRunnable,
        Eligibility::NotYetEstablished => PolicyEligibilityClass::NotYetEstablished,
    };
    Some(derive_policy_from_projection(
        expectation,
        eligibility,
        observed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use DerivedPolicyResult as Policy;
    use PolicyExpectationClass as Expectation;

    #[test]
    fn complete_typed_state_and_expectation_matrix() {
        let attempts = [
            Attempt::NotAttempted(Reason::Eligibility),
            Attempt::NotAttempted(Reason::LaneExcluded),
            Attempt::NotAttempted(Reason::ParserPreAttemptEvaluation),
            Attempt::NotAttempted(Reason::CssFragmentCapabilityUnavailable),
            Attempt::Attempted(Terminal::SemanticPass),
            Attempt::Attempted(Terminal::SemanticFail),
            Attempt::Attempted(Terminal::ExecutionFailure),
            Attempt::Attempted(Terminal::ResourceFailure),
            Attempt::Attempted(Terminal::IncompleteObservation),
            Attempt::Attempted(Terminal::InvariantFailure),
            Attempt::Attempted(Terminal::Timeout),
        ];
        for eligibility in [
            Eligibility::Runnable,
            Eligibility::NotRunnable,
            Eligibility::NotYetEstablished,
        ] {
            for selection in [
                Selection::Selected,
                Selection::Excluded,
                Selection::NotApplicable,
            ] {
                for attempt in attempts {
                    for expectation in [
                        Expectation::ExpectedPass,
                        Expectation::ExpectedFailSemanticMismatch,
                        Expectation::NotEstablished,
                    ] {
                        // Independent exhaustive specification of the accepted
                        // tuples and policies, including reserved terminal timeout.
                        let expected = match (eligibility, selection, attempt) {
                            (
                                Eligibility::NotRunnable,
                                Selection::NotApplicable,
                                Attempt::NotAttempted(Reason::Eligibility),
                            ) => Some(Policy::NotRun),
                            (
                                Eligibility::NotYetEstablished,
                                Selection::NotApplicable,
                                Attempt::NotAttempted(Reason::Eligibility),
                            ) => Some(Policy::NotYetEstablished),
                            (
                                Eligibility::Runnable,
                                Selection::Excluded,
                                Attempt::NotAttempted(Reason::LaneExcluded),
                            ) => Some(Policy::NotRun),
                            (
                                Eligibility::Runnable,
                                Selection::Selected,
                                Attempt::NotAttempted(
                                    Reason::ParserPreAttemptEvaluation
                                    | Reason::CssFragmentCapabilityUnavailable,
                                ),
                            ) => Some(Policy::UnexpectedOutcome),
                            (
                                Eligibility::Runnable,
                                Selection::Selected,
                                Attempt::Attempted(terminal),
                            ) => Some(match (expectation, terminal) {
                                (Expectation::NotEstablished, _) => Policy::NotYetEstablished,
                                (Expectation::ExpectedPass, Terminal::SemanticPass) => {
                                    Policy::ExpectedPass
                                }
                                (Expectation::ExpectedPass, Terminal::SemanticFail) => {
                                    Policy::UnexpectedFail
                                }
                                (
                                    Expectation::ExpectedFailSemanticMismatch,
                                    Terminal::SemanticPass,
                                ) => Policy::UnexpectedPass,
                                (
                                    Expectation::ExpectedFailSemanticMismatch,
                                    Terminal::SemanticFail,
                                ) => Policy::ExpectedFail,
                                (
                                    _,
                                    Terminal::ExecutionFailure
                                    | Terminal::ResourceFailure
                                    | Terminal::IncompleteObservation
                                    | Terminal::InvariantFailure
                                    | Terminal::Timeout,
                                ) => Policy::UnexpectedOutcome,
                            }),
                            _ => None,
                        };
                        assert_eq!(
                            historical_policy(eligibility, selection, attempt, expectation),
                            expected,
                            "{eligibility:?} {selection:?} {attempt:?} {expectation:?}"
                        );
                        assert_eq!(
                            expected.is_some(),
                            valid_selection_attempt_projection(eligibility, selection, attempt)
                        );
                    }
                }
            }
        }
    }
}

use std::convert::Infallible;

use crate::property_registry;

use super::declarations::{CascadeDeclarationInput, CascadeSpecifiedValue};
use super::priority::CascadePriority;
use super::properties::CascadePropertyId;
use super::rules::ValidatedCascadeRuleInputs;
use super::sources::CascadeDeclarationSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateDataMismatch {
    Priority,
    Value,
    PriorityAndValue,
    ExpansionMetadata,
}

impl CandidateDataMismatch {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Priority => "priority",
            Self::Value => "value",
            Self::PriorityAndValue => "priority-and-value",
            Self::ExpansionMetadata => "expansion-metadata",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleInputSequenceViolation {
    NonIncreasingStylesheetOrder,
    StylesheetAfterInline,
    MultipleInline,
    NonInlineAtInlineBoundary,
}

impl RuleInputSequenceViolation {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::NonIncreasingStylesheetOrder => "non-increasing-stylesheet-order",
            Self::StylesheetAfterInline => "stylesheet-after-inline",
            Self::MultipleInline => "multiple-inline-inputs",
            Self::NonInlineAtInlineBoundary => "non-inline-at-inline-boundary",
        }
    }
}

/// CSS-owned failures produced while validating or selecting cascade winners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeResolutionError {
    CandidateCeilingOverflow {
        stylesheet_limit: usize,
        inline_limit: usize,
    },
    RuleInputCeilingOverflow {
        matched_rule_limit: usize,
    },
    UnsupportedLocatorLimit {
        coordinate: &'static str,
        configured: usize,
        maximum: usize,
    },
    CandidateLimitExceeded {
        required: usize,
        maximum: usize,
    },
    WinnerWorkspaceReservationFailed {
        requested: usize,
    },
    WinnerOutputReservationFailed {
        requested: usize,
    },
    RuleInputStorageReservationFailed {
        requested: usize,
    },
    RuleInputSequenceInvariant {
        previous_source: Option<super::sources::CascadeRuleSource>,
        current_source: super::sources::CascadeRuleSource,
        violation: RuleInputSequenceViolation,
    },
    DeclarationSourceOrderInvariant {
        rule_source: super::sources::CascadeRuleSource,
        previous_source: CascadeDeclarationSource,
        current_source: CascadeDeclarationSource,
        previous_order: super::order::DeclarationOrder,
        current_order: super::order::DeclarationOrder,
    },
    DuplicateCandidateIdentity {
        property: CascadePropertyId,
        first_source: CascadeDeclarationSource,
        second_source: CascadeDeclarationSource,
        first_priority: CascadePriority,
        second_priority: CascadePriority,
    },
    InconsistentCandidateIdentity {
        property: CascadePropertyId,
        first_source: CascadeDeclarationSource,
        second_source: CascadeDeclarationSource,
        first_priority: CascadePriority,
        second_priority: CascadePriority,
        mismatch: CandidateDataMismatch,
    },
    EqualPriorityDistinctCandidates {
        property: CascadePropertyId,
        first_source: CascadeDeclarationSource,
        second_source: CascadeDeclarationSource,
        first_priority: CascadePriority,
        second_priority: CascadePriority,
    },
}

impl CascadeResolutionError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::CandidateCeilingOverflow { .. } => "candidate-ceiling-overflow",
            Self::RuleInputCeilingOverflow { .. } => "rule-input-ceiling-overflow",
            Self::UnsupportedLocatorLimit { .. } => "unsupported-locator-limit",
            Self::CandidateLimitExceeded { .. } => "candidate-limit-exceeded",
            Self::WinnerWorkspaceReservationFailed { .. } => "winner-workspace-reservation-failed",
            Self::WinnerOutputReservationFailed { .. } => "winner-output-reservation-failed",
            Self::RuleInputStorageReservationFailed { .. } => {
                "rule-input-storage-reservation-failed"
            }
            Self::RuleInputSequenceInvariant { .. } => "rule-input-sequence-invariant",
            Self::DeclarationSourceOrderInvariant { .. } => "declaration-source-order-invariant",
            Self::DuplicateCandidateIdentity { .. } => "duplicate-candidate-identity",
            Self::InconsistentCandidateIdentity { .. } => "inconsistent-candidate-identity",
            Self::EqualPriorityDistinctCandidates { .. } => "equal-priority-distinct-candidates",
        }
    }
}

impl std::fmt::Display for CascadeResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cascade resolution failure: {}",
            self.stable_label()
        )?;
        match self {
            Self::CandidateCeilingOverflow {
                stylesheet_limit,
                inline_limit,
            } => write!(
                formatter,
                " stylesheet-limit={stylesheet_limit} inline-limit={inline_limit}"
            ),
            Self::RuleInputCeilingOverflow { matched_rule_limit } => {
                write!(formatter, " matched-rule-limit={matched_rule_limit}")
            }
            Self::UnsupportedLocatorLimit {
                coordinate,
                configured,
                maximum,
            } => write!(
                formatter,
                " coordinate={coordinate} configured={configured} maximum={maximum}"
            ),
            Self::CandidateLimitExceeded { required, maximum } => {
                write!(formatter, " required={required} maximum={maximum}")
            }
            Self::WinnerWorkspaceReservationFailed { requested }
            | Self::WinnerOutputReservationFailed { requested }
            | Self::RuleInputStorageReservationFailed { requested } => {
                write!(formatter, " requested={requested}")
            }
            Self::RuleInputSequenceInvariant {
                previous_source,
                current_source,
                violation,
            } => {
                formatter.write_str(" previous-source=")?;
                if let Some(previous) = previous_source {
                    write_rule_source(formatter, *previous)?;
                } else {
                    formatter.write_str("none")?;
                }
                formatter.write_str(" current-source=")?;
                write_rule_source(formatter, *current_source)?;
                write!(formatter, " violation={}", violation.stable_label())
            }
            Self::DeclarationSourceOrderInvariant {
                rule_source,
                previous_source,
                current_source,
                previous_order,
                current_order,
            } => {
                formatter.write_str(" rule-source=")?;
                write_rule_source(formatter, *rule_source)?;
                formatter.write_str(" previous-source=")?;
                write_declaration_source(formatter, *previous_source)?;
                formatter.write_str(" current-source=")?;
                write_declaration_source(formatter, *current_source)?;
                write!(
                    formatter,
                    " previous-order={} current-order={}",
                    previous_order.get(),
                    current_order.get()
                )
            }
            Self::DuplicateCandidateIdentity {
                property,
                first_source,
                second_source,
                first_priority,
                second_priority,
            }
            | Self::InconsistentCandidateIdentity {
                property,
                first_source,
                second_source,
                first_priority,
                second_priority,
                ..
            } => {
                write!(formatter, " property={} first-source=", property.name())?;
                write_declaration_source(formatter, *first_source)?;
                formatter.write_str(" second-source=")?;
                write_declaration_source(formatter, *second_source)?;
                formatter.write_str(" first-priority=")?;
                write_priority(formatter, *first_priority)?;
                formatter.write_str(" second-priority=")?;
                write_priority(formatter, *second_priority)?;
                if let Self::InconsistentCandidateIdentity { mismatch, .. } = self {
                    write!(formatter, " mismatch={}", mismatch.stable_label())?;
                }
                Ok(())
            }
            Self::EqualPriorityDistinctCandidates {
                property,
                first_source,
                second_source,
                first_priority,
                second_priority,
            } => {
                write!(formatter, " property={} first-source=", property.name())?;
                write_declaration_source(formatter, *first_source)?;
                formatter.write_str(" second-source=")?;
                write_declaration_source(formatter, *second_source)?;
                formatter.write_str(" first-priority=")?;
                write_priority(formatter, *first_priority)?;
                formatter.write_str(" second-priority=")?;
                write_priority(formatter, *second_priority)
            }
        }
    }
}

fn write_priority(
    formatter: &mut std::fmt::Formatter<'_>,
    priority: CascadePriority,
) -> std::fmt::Result {
    write!(formatter, "{}:", priority.band().as_debug_label())?;
    if priority.declaration_precedence().is_element_attached() {
        write!(
            formatter,
            "element-attached:{}",
            priority.declaration_order().get()
        )
    } else {
        let specificity = priority
            .specificity()
            .expect("style-rule priority has specificity");
        let order = priority
            .source_order()
            .expect("style-rule priority has source order");
        write!(
            formatter,
            "style-rule:{},{},{}:{}/{}:{}",
            specificity.a(),
            specificity.b(),
            specificity.c(),
            order.stylesheet().get(),
            order.rule().get(),
            priority.declaration_order().get()
        )
    }
}

fn write_rule_source(
    formatter: &mut std::fmt::Formatter<'_>,
    source: super::sources::CascadeRuleSource,
) -> std::fmt::Result {
    match source {
        super::sources::CascadeRuleSource::Stylesheet(source) => write!(
            formatter,
            "stylesheet[{}/{}]",
            source.source_id().get(),
            source.raw_rule_index().get()
        ),
        super::sources::CascadeRuleSource::InlineStyle(source) => {
            formatter.write_str("inline-style[")?;
            write_inline_rule_source(formatter, source)?;
            formatter.write_str("]")
        }
    }
}

fn write_declaration_source(
    formatter: &mut std::fmt::Formatter<'_>,
    source: CascadeDeclarationSource,
) -> std::fmt::Result {
    match source {
        CascadeDeclarationSource::Stylesheet(source) => write!(
            formatter,
            "stylesheet[{}/{}]/declaration[{}]",
            source.source_id().get(),
            source.raw_rule_index().get(),
            source.declaration_index().get()
        ),
        CascadeDeclarationSource::InlineStyle(source) => {
            formatter.write_str("inline-style[")?;
            write_inline_rule_source(formatter, source.inline_style())?;
            write!(
                formatter,
                "]/declaration[{}]",
                source.declaration_index().get()
            )
        }
    }
}

fn write_inline_rule_source(
    formatter: &mut std::fmt::Formatter<'_>,
    source: super::sources::InlineStyleRuleRef,
) -> std::fmt::Result {
    match source {
        super::sources::InlineStyleRuleRef::Element(element) => {
            write!(formatter, "element={}", element.get())
        }
        super::sources::InlineStyleRuleRef::Diagnostic => formatter.write_str("diagnostic"),
        #[cfg(test)]
        super::sources::InlineStyleRuleRef::CompatibilityScope(scope) => {
            write!(formatter, "compatibility={scope}")
        }
    }
}

impl std::error::Error for CascadeResolutionError {}

/// Checked production resource budget derived once per style execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CascadeResolutionBudget {
    maximum_candidates_per_element: usize,
    winner_capacity: usize,
}

impl CascadeResolutionBudget {
    pub(crate) fn try_new(
        maximum_stylesheet_declarations: usize,
        maximum_inline_declarations: usize,
        maximum_matched_rules: usize,
    ) -> Result<Self, CascadeResolutionError> {
        let maximum_candidates_per_element = maximum_stylesheet_declarations
            .checked_add(maximum_inline_declarations)
            .ok_or(CascadeResolutionError::CandidateCeilingOverflow {
                stylesheet_limit: maximum_stylesheet_declarations,
                inline_limit: maximum_inline_declarations,
            })?;
        let maximum_rule_inputs = maximum_matched_rules.checked_add(1).ok_or(
            CascadeResolutionError::RuleInputCeilingOverflow {
                matched_rule_limit: maximum_matched_rules,
            },
        )?;
        validate_locator_limit("rule-input-index", maximum_rule_inputs)?;
        validate_locator_limit(
            "declaration-input-index",
            maximum_stylesheet_declarations.max(maximum_inline_declarations),
        )?;
        let winner_capacity = property_registry().entries().len();
        validate_locator_limit("property-registry-index", winner_capacity)?;
        Ok(Self {
            maximum_candidates_per_element,
            winner_capacity,
        })
    }

    pub(crate) const fn maximum_candidates_per_element(self) -> usize {
        self.maximum_candidates_per_element
    }

    pub(crate) const fn winner_capacity(self) -> usize {
        self.winner_capacity
    }
}

fn validate_locator_limit(
    coordinate: &'static str,
    configured: usize,
) -> Result<(), CascadeResolutionError> {
    let maximum = u32::MAX as usize;
    if configured > maximum {
        return Err(CascadeResolutionError::UnsupportedLocatorLimit {
            coordinate,
            configured,
            maximum,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CascadeRuleInputIndex(u32);

impl CascadeRuleInputIndex {
    fn try_from_usize(value: usize) -> Result<Self, CascadeResolutionError> {
        u32::try_from(value).map(Self).map_err(|_| {
            CascadeResolutionError::UnsupportedLocatorLimit {
                coordinate: "rule-input-index",
                configured: value,
                maximum: u32::MAX as usize,
            }
        })
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CascadeDeclarationInputIndex(u32);

impl CascadeDeclarationInputIndex {
    fn try_from_usize(value: usize) -> Result<Self, CascadeResolutionError> {
        u32::try_from(value).map(Self).map_err(|_| {
            CascadeResolutionError::UnsupportedLocatorLimit {
                coordinate: "declaration-input-index",
                configured: value,
                maximum: u32::MAX as usize,
            }
        })
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CascadeCandidateLocator {
    rule_input: CascadeRuleInputIndex,
    declaration_input: CascadeDeclarationInputIndex,
}

impl CascadeCandidateLocator {
    fn try_new(
        rule_input: usize,
        declaration_input: usize,
    ) -> Result<Self, CascadeResolutionError> {
        Ok(Self {
            rule_input: CascadeRuleInputIndex::try_from_usize(rule_input)?,
            declaration_input: CascadeDeclarationInputIndex::try_from_usize(declaration_input)?,
        })
    }
}

/// Borrowed candidate observed by the shared production/diagnostic evaluator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CascadeDeclarationCandidate<'a> {
    property: CascadePropertyId,
    source: CascadeDeclarationSource,
    priority: CascadePriority,
    value: &'a CascadeSpecifiedValue,
    locator: CascadeCandidateLocator,
}

impl<'a> CascadeDeclarationCandidate<'a> {
    pub(crate) fn property(self) -> CascadePropertyId {
        self.property
    }

    pub(crate) fn source(self) -> CascadeDeclarationSource {
        self.source
    }

    pub(crate) fn priority(self) -> CascadePriority {
        self.priority
    }

    pub(crate) fn value(self) -> &'a CascadeSpecifiedValue {
        self.value
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CascadeWinnerScratch {
    locator: CascadeCandidateLocator,
    observation_index: CascadeCandidateObservationIndex,
    source: CascadeDeclarationSource,
    priority: CascadePriority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CascadeCandidateObservationIndex(usize);

impl CascadeCandidateObservationIndex {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Reusable O(registered-properties) workspace local to one style execution.
#[derive(Debug)]
pub(crate) struct CascadeResolutionWorkspace {
    winner_slots: Vec<Option<CascadeWinnerScratch>>,
}

impl CascadeResolutionWorkspace {
    pub(crate) fn try_new(budget: CascadeResolutionBudget) -> Result<Self, CascadeResolutionError> {
        let mut winner_slots = Vec::new();
        try_reserve_winner_workspace(&mut winner_slots, budget.winner_capacity())?;
        winner_slots.resize(budget.winner_capacity(), None);
        Ok(Self { winner_slots })
    }

    pub(crate) fn clear(&mut self) {
        self.winner_slots.fill(None);
    }

    #[cfg(any(test, feature = "count-alloc"))]
    pub(crate) fn capacity(&self) -> usize {
        self.winner_slots.capacity()
    }
}

fn try_reserve_winner_workspace<T>(
    storage: &mut Vec<T>,
    requested: usize,
) -> Result<(), CascadeResolutionError> {
    storage
        .try_reserve_exact(requested)
        .map_err(|_| CascadeResolutionError::WinnerWorkspaceReservationFailed { requested })
}

fn try_reserve_winner_output(
    storage: &mut Vec<CascadeWinnerEntry>,
    requested: usize,
) -> Result<(), CascadeResolutionError> {
    storage
        .try_reserve_exact(requested)
        .map_err(|_| CascadeResolutionError::WinnerOutputReservationFailed { requested })
}

pub(crate) trait CascadeEvaluationObserver {
    type Error;

    fn candidate(
        &mut self,
        _observation_index: CascadeCandidateObservationIndex,
        _candidate: CascadeDeclarationCandidate<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Receives only the winner remaining after every candidate was compared.
    fn final_winner(
        &mut self,
        _property: CascadePropertyId,
        _observation_index: CascadeCandidateObservationIndex,
        _candidate: CascadeDeclarationCandidate<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) struct NoopCascadeEvaluationObserver;

impl CascadeEvaluationObserver for NoopCascadeEvaluationObserver {
    type Error = Infallible;
}

pub(crate) enum CascadeEvaluationFailure<ObserverError> {
    Cascade(CascadeResolutionError),
    Observer(ObserverError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeWinner {
    pub source: CascadeDeclarationSource,
    pub priority: CascadePriority,
    pub value: CascadeSpecifiedValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeWinnerEntry {
    property: CascadePropertyId,
    winner: CascadeWinner,
}

impl CascadeWinnerEntry {
    pub fn property(&self) -> CascadePropertyId {
        self.property
    }

    pub fn winner(&self) -> &CascadeWinner {
        &self.winner
    }

    pub(crate) fn into_parts(self) -> (CascadePropertyId, CascadeWinner) {
        (self.property, self.winner)
    }
}

/// Sparse cascaded-value projection for AF6's supported feature subset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CascadeWinnerSet {
    entries: Vec<CascadeWinnerEntry>,
}

impl CascadeWinnerSet {
    pub fn entries(&self) -> &[CascadeWinnerEntry] {
        &self.entries
    }

    pub fn get(&self, property: CascadePropertyId) -> Option<&CascadeWinner> {
        self.entries
            .binary_search_by_key(&property, CascadeWinnerEntry::property)
            .ok()
            .map(|index| self.entries[index].winner())
    }

    pub(crate) fn into_entries(self) -> impl Iterator<Item = CascadeWinnerEntry> {
        self.entries.into_iter()
    }
}

pub(crate) fn resolve_cascade_winners_from_validated_inputs<O>(
    inputs: &ValidatedCascadeRuleInputs<'_>,
    budget: CascadeResolutionBudget,
    workspace: &mut CascadeResolutionWorkspace,
    observer: &mut O,
) -> Result<CascadeWinnerSet, CascadeEvaluationFailure<O::Error>>
where
    O: CascadeEvaluationObserver,
{
    if inputs.admitted_candidate_count() > budget.maximum_candidates_per_element() {
        return Err(CascadeEvaluationFailure::Cascade(
            CascadeResolutionError::CandidateLimitExceeded {
                required: inputs.admitted_candidate_count(),
                maximum: budget.maximum_candidates_per_element(),
            },
        ));
    }
    workspace.clear();
    let mut observation_index = 0usize;

    for (rule_index, rule) in inputs.inputs().iter().enumerate() {
        for (declaration_index, declaration) in rule.declarations().iter().enumerate() {
            let Some(property) = declaration.applicability().supported_property() else {
                continue;
            };
            let locator = CascadeCandidateLocator::try_new(rule_index, declaration_index)
                .map_err(CascadeEvaluationFailure::Cascade)?;
            let candidate = CascadeDeclarationCandidate {
                property,
                source: declaration.source(),
                priority: rule.context().priority_for_declaration(
                    declaration.importance(),
                    declaration.declaration_order(),
                ),
                value: declaration.value(),
                locator,
            };
            let candidate_observation = CascadeCandidateObservationIndex(observation_index);
            observation_index = observation_index.checked_add(1).ok_or_else(|| {
                CascadeEvaluationFailure::Cascade(CascadeResolutionError::CandidateLimitExceeded {
                    required: usize::MAX,
                    maximum: budget.maximum_candidates_per_element(),
                })
            })?;
            observer
                .candidate(candidate_observation, candidate)
                .map_err(CascadeEvaluationFailure::Observer)?;

            let slot = &mut workspace.winner_slots[property.as_index()];
            match slot {
                None => {
                    *slot = Some(CascadeWinnerScratch {
                        locator,
                        observation_index: candidate_observation,
                        source: candidate.source,
                        priority: candidate.priority,
                    });
                }
                Some(current) if candidate.priority > current.priority => {
                    *current = CascadeWinnerScratch {
                        locator,
                        observation_index: candidate_observation,
                        source: candidate.source,
                        priority: candidate.priority,
                    };
                }
                Some(current) if candidate.priority == current.priority => {
                    return Err(CascadeEvaluationFailure::Cascade(
                        CascadeResolutionError::EqualPriorityDistinctCandidates {
                            property,
                            first_source: current.source,
                            second_source: candidate.source,
                            first_priority: current.priority,
                            second_priority: candidate.priority,
                        },
                    ));
                }
                Some(_) => {}
            }
        }
    }

    let expected = inputs
        .admitted_candidate_count()
        .min(budget.winner_capacity());
    let mut entries = Vec::new();
    try_reserve_winner_output(&mut entries, expected).map_err(CascadeEvaluationFailure::Cascade)?;

    for property in property_registry().ids() {
        let Some(scratch) = workspace.winner_slots[property.as_index()] else {
            continue;
        };
        let candidate =
            resolve_locator(inputs, scratch.locator).map_err(CascadeEvaluationFailure::Cascade)?;
        observer
            .final_winner(property, scratch.observation_index, candidate)
            .map_err(CascadeEvaluationFailure::Observer)?;
        entries.push(CascadeWinnerEntry {
            property,
            winner: CascadeWinner {
                source: candidate.source,
                priority: candidate.priority,
                value: candidate.value.clone(),
            },
        });
    }

    Ok(CascadeWinnerSet { entries })
}

fn resolve_locator<'a>(
    inputs: &'a ValidatedCascadeRuleInputs<'_>,
    locator: CascadeCandidateLocator,
) -> Result<CascadeDeclarationCandidate<'a>, CascadeResolutionError> {
    let rule = &inputs.inputs()[locator.rule_input.as_usize()];
    let declaration: &'a CascadeDeclarationInput =
        &rule.declarations()[locator.declaration_input.as_usize()];
    let property = declaration
        .applicability()
        .supported_property()
        .expect("winner locator must reference an admitted supported declaration");
    Ok(CascadeDeclarationCandidate {
        property,
        source: declaration.source(),
        priority: rule
            .context()
            .priority_for_declaration(declaration.importance(), declaration.declaration_order()),
        value: declaration.value(),
        locator,
    })
}

pub(crate) fn resolve_cascade_winners(
    inputs: &ValidatedCascadeRuleInputs<'_>,
    budget: CascadeResolutionBudget,
    workspace: &mut CascadeResolutionWorkspace,
) -> Result<CascadeWinnerSet, CascadeResolutionError> {
    let mut observer = NoopCascadeEvaluationObserver;
    match resolve_cascade_winners_from_validated_inputs(inputs, budget, workspace, &mut observer) {
        Ok(winners) => Ok(winners),
        Err(CascadeEvaluationFailure::Cascade(error)) => Err(error),
        Err(CascadeEvaluationFailure::Observer(error)) => match error {},
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn checked_budget_rejects_candidate_arithmetic_and_locator_overflow() {
        assert!(matches!(
            CascadeResolutionBudget::try_new(usize::MAX, 1, 0),
            Err(CascadeResolutionError::CandidateCeilingOverflow { .. })
        ));
        assert!(matches!(
            CascadeResolutionBudget::try_new(0, 0, usize::MAX),
            Err(CascadeResolutionError::RuleInputCeilingOverflow { .. })
        ));
        assert!(matches!(
            CascadeResolutionBudget::try_new(0, 0, u32::MAX as usize),
            Err(CascadeResolutionError::UnsupportedLocatorLimit { .. })
        ));
    }

    #[test]
    fn production_reservation_sites_return_their_typed_errors() {
        let mut workspace = Vec::<u8>::new();
        assert_eq!(
            try_reserve_winner_workspace(&mut workspace, usize::MAX),
            Err(CascadeResolutionError::WinnerWorkspaceReservationFailed {
                requested: usize::MAX,
            })
        );
        let mut output = Vec::<CascadeWinnerEntry>::new();
        assert_eq!(
            try_reserve_winner_output(&mut output, usize::MAX),
            Err(CascadeResolutionError::WinnerOutputReservationFailed {
                requested: usize::MAX,
            })
        );
    }

    #[test]
    fn workspace_clear_reuses_registry_derived_capacity_without_retaining_locators() {
        let budget = CascadeResolutionBudget::try_new(8, 4, 2).unwrap();
        let mut workspace = CascadeResolutionWorkspace::try_new(budget).unwrap();
        let capacity = workspace.capacity();
        assert!(capacity >= property_registry().entries().len());
        workspace.winner_slots[0] = Some(CascadeWinnerScratch {
            locator: CascadeCandidateLocator::try_new(0, 0).unwrap(),
            observation_index: CascadeCandidateObservationIndex(0),
            source: CascadeDeclarationSource::Stylesheet(
                super::super::sources::StylesheetDeclarationRef::new(
                    super::super::order::StylesheetSourceId::compatibility_generation_index(0),
                    super::super::order::RawRuleIndex::new(0),
                    super::super::order::DeclarationSourceIndex::new(0),
                ),
            ),
            priority: super::super::sources::CascadeRuleContext::for_stylesheet(
                super::super::priority::CascadeOrigin::Author,
                crate::selectors::Specificity::ZERO,
                super::super::order::StylesheetRuleOrder::new(
                    super::super::order::StylesheetOrder::new(0),
                    super::super::order::StyleRulePosition::new(0),
                ),
            )
            .priority_for_declaration(
                super::super::priority::CascadeImportance::Normal,
                super::super::order::DeclarationOrder::new(0),
            ),
        });
        workspace.clear();
        assert_eq!(workspace.capacity(), capacity);
        assert!(workspace.winner_slots.iter().all(Option::is_none));
    }
}

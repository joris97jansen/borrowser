use crate::selectors::SelectorListMatchOutcome;

use super::declarations::CascadeDeclarationInput;
use super::order::StylesheetRuleOrder;
#[cfg(test)]
use super::order::{StyleRulePosition, StylesheetOrder};
#[cfg(test)]
use super::priority::CascadeOrigin;
use super::sources::{
    CascadeDeclarationSource, CascadeRuleContext, CascadeRuleMatch, CascadeRuleSource,
    InlineStyleRuleRef,
};
use super::winners::{
    CandidateDataMismatch, CascadeResolutionBudget, CascadeResolutionError,
    RuleInputSequenceViolation,
};

/// Authoritative matched rule input. Stylesheet declarations are borrowed from
/// the pass-scoped collection arena; inline declarations are owned by the one
/// element whose style attribute was parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeRuleInput<'collection> {
    Stylesheet(MatchedStylesheetRuleInput<'collection>),
    Inline(InlineStyleRuleInput),
    #[cfg(test)]
    Compatibility(CompatibilityRuleInput),
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityRuleInput {
    source: CascadeRuleSource,
    context: CascadeRuleContext,
    declarations: Vec<CascadeDeclarationInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchedStylesheetRuleInput<'collection> {
    rule_ref: super::sources::StylesheetRuleRef,
    rule_match: CascadeRuleMatch,
    context: CascadeRuleContext,
    declarations: &'collection [CascadeDeclarationInput],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineStyleRuleInput {
    source: InlineStyleRuleRef,
    context: CascadeRuleContext,
    declarations: Vec<CascadeDeclarationInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeRuleInputBuildError {
    rule_source: CascadeRuleSource,
    declaration_source: CascadeDeclarationSource,
    declaration_position: usize,
}

impl CascadeRuleInputBuildError {
    pub fn rule_source(&self) -> CascadeRuleSource {
        self.rule_source
    }

    pub fn declaration_source(&self) -> CascadeDeclarationSource {
        self.declaration_source
    }

    pub fn declaration_position(&self) -> usize {
        self.declaration_position
    }
}

impl std::fmt::Display for CascadeRuleInputBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cascade rule input declaration at position {} does not belong to its rule source",
            self.declaration_position
        )
    }
}

impl std::error::Error for CascadeRuleInputBuildError {}

impl<'collection> CascadeRuleInput<'collection> {
    /// Constructs the production AF5 handoff without re-walking declarations.
    /// `RuleCollection` created both the rule and every declaration source in
    /// one checked source-coordinate traversal.
    pub(crate) fn from_validated_stylesheet_match(
        rule_ref: super::sources::StylesheetRuleRef,
        origin: super::priority::CascadeOrigin,
        source_order: StylesheetRuleOrder,
        outcome: SelectorListMatchOutcome,
        declarations: &'collection [CascadeDeclarationInput],
    ) -> Option<Self> {
        let rule_match = CascadeRuleMatch::new(rule_ref, outcome);
        let context = CascadeRuleContext::from_stylesheet_match(origin, source_order, &rule_match)?;
        Some(Self::Stylesheet(MatchedStylesheetRuleInput {
            rule_ref,
            rule_match,
            context,
            declarations,
        }))
    }

    pub fn from_inline_style_collected(
        inline_style: InlineStyleRuleRef,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        let source = CascadeRuleSource::InlineStyle(inline_style);
        validate_declaration_sources(source, &declarations)?;
        Ok(Self::Inline(InlineStyleRuleInput {
            source: inline_style,
            context: CascadeRuleContext::for_inline_style(),
            declarations,
        }))
    }

    /// Constructs the production inline handoff from declarations emitted by
    /// the inline classifier for this exact element-attached source.
    pub(crate) fn from_validated_inline_style(
        inline_style: InlineStyleRuleRef,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Self {
        Self::Inline(InlineStyleRuleInput {
            source: inline_style,
            context: CascadeRuleContext::for_inline_style(),
            declarations,
        })
    }

    #[cfg(test)]
    pub fn new(
        source: CascadeRuleSource,
        context: CascadeRuleContext,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        validate_declaration_sources(source, &declarations)?;
        Ok(Self::Compatibility(CompatibilityRuleInput {
            source,
            context,
            declarations,
        }))
    }

    #[cfg(test)]
    pub fn from_stylesheet_match(
        rule_match: &CascadeRuleMatch,
        origin: CascadeOrigin,
        rule_order: u32,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Option<Self>, CascadeRuleInputBuildError> {
        let Some(context) = CascadeRuleContext::from_stylesheet_match(
            origin,
            StylesheetRuleOrder::new(StylesheetOrder::new(0), StyleRulePosition::new(rule_order)),
            rule_match,
        ) else {
            return Ok(None);
        };
        Self::new(
            CascadeRuleSource::Stylesheet(rule_match.rule_ref()),
            context,
            declarations,
        )
        .map(Some)
    }

    #[cfg(test)]
    pub fn from_inline_style(
        inline_style: InlineStyleRuleRef,
        _legacy_rule_order: u32,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        Self::new(
            CascadeRuleSource::InlineStyle(inline_style),
            CascadeRuleContext::for_inline_style(),
            declarations,
        )
    }

    pub fn source(&self) -> CascadeRuleSource {
        match self {
            Self::Stylesheet(input) => CascadeRuleSource::Stylesheet(input.rule_ref),
            Self::Inline(input) => CascadeRuleSource::InlineStyle(input.source),
            #[cfg(test)]
            Self::Compatibility(input) => input.source,
        }
    }

    pub fn context(&self) -> CascadeRuleContext {
        match self {
            Self::Stylesheet(input) => input.context,
            Self::Inline(input) => input.context,
            #[cfg(test)]
            Self::Compatibility(input) => input.context,
        }
    }

    pub fn declarations(&self) -> &[CascadeDeclarationInput] {
        match self {
            Self::Stylesheet(input) => input.declarations,
            Self::Inline(input) => &input.declarations,
            #[cfg(test)]
            Self::Compatibility(input) => &input.declarations,
        }
    }

    pub fn stylesheet_match_outcome(&self) -> Option<&SelectorListMatchOutcome> {
        match self {
            Self::Stylesheet(input) => Some(input.rule_match.outcome()),
            Self::Inline(_) => None,
            #[cfg(test)]
            Self::Compatibility(_) => None,
        }
    }

    pub fn stylesheet_rule_order(&self) -> Option<StylesheetRuleOrder> {
        match self {
            Self::Stylesheet(input) => match input.context {
                CascadeRuleContext::Stylesheet { source_order, .. } => Some(source_order),
                CascadeRuleContext::InlineStyle => None,
            },
            Self::Inline(_) => None,
            #[cfg(test)]
            Self::Compatibility(_) => None,
        }
    }
}

/// Opaque AF6 input view constructed by one matched-rule traversal and at most
/// one inline-style append. Its admitted count is produced by the same pass
/// that validates declaration identities.
#[derive(Debug)]
pub(crate) struct ValidatedCascadeRuleInputs<'collection> {
    inputs: Vec<CascadeRuleInput<'collection>>,
    admitted_candidate_count: usize,
}

impl<'collection> ValidatedCascadeRuleInputs<'collection> {
    pub(crate) fn inputs(&self) -> &[CascadeRuleInput<'collection>] {
        &self.inputs
    }

    pub(crate) const fn admitted_candidate_count(&self) -> usize {
        self.admitted_candidate_count
    }

    #[cfg(test)]
    pub(crate) fn try_from_checked_inputs(
        inputs: Vec<CascadeRuleInput<'collection>>,
        budget: CascadeResolutionBudget,
    ) -> Result<Self, CascadeResolutionError> {
        let mut builder = ValidatedCascadeRuleInputBuilder::new(budget);
        for input in inputs {
            builder.push_checked(input)?;
        }
        let validated = builder.finish();
        validate_checked_candidate_uniqueness(&validated)?;
        Ok(validated)
    }
}

/// Pass-one builder. Production callers can add stylesheet rules only in
/// strictly increasing AF5 order and can append at most one inline input.
pub(crate) struct ValidatedCascadeRuleInputBuilder<'collection> {
    inputs: Vec<CascadeRuleInput<'collection>>,
    admitted_candidate_count: usize,
    maximum_candidates: usize,
    last_stylesheet_order: Option<StylesheetRuleOrder>,
    last_input_source: Option<CascadeRuleSource>,
    inline_seen: bool,
}

impl<'collection> ValidatedCascadeRuleInputBuilder<'collection> {
    pub(crate) fn new(budget: CascadeResolutionBudget) -> Self {
        Self {
            inputs: Vec::new(),
            admitted_candidate_count: 0,
            maximum_candidates: budget.maximum_candidates_per_element(),
            last_stylesheet_order: None,
            last_input_source: None,
            inline_seen: false,
        }
    }

    pub(crate) fn try_reserve_rule_inputs(
        &mut self,
        additional: usize,
    ) -> Result<(), CascadeResolutionError> {
        self.inputs.try_reserve(additional).map_err(|_| {
            CascadeResolutionError::RuleInputStorageReservationFailed {
                requested: additional,
            }
        })
    }

    pub(crate) fn push_stylesheet(
        &mut self,
        input: CascadeRuleInput<'collection>,
    ) -> Result<(), CascadeResolutionError> {
        let order = input
            .stylesheet_rule_order()
            .expect("stylesheet input builder accepts only stylesheet rules");
        if self.inline_seen {
            return Err(CascadeResolutionError::RuleInputSequenceInvariant {
                previous_source: self.last_input_source,
                current_source: input.source(),
                violation: RuleInputSequenceViolation::StylesheetAfterInline,
            });
        }
        if self
            .last_stylesheet_order
            .is_some_and(|previous| order <= previous)
        {
            return Err(CascadeResolutionError::RuleInputSequenceInvariant {
                previous_source: self.last_input_source,
                current_source: input.source(),
                violation: RuleInputSequenceViolation::NonIncreasingStylesheetOrder,
            });
        }
        self.last_stylesheet_order = Some(order);
        self.last_input_source = Some(input.source());
        self.push_validated(input)
    }

    pub(crate) fn push_inline(
        &mut self,
        input: CascadeRuleInput<'collection>,
    ) -> Result<(), CascadeResolutionError> {
        if self.inline_seen {
            return Err(CascadeResolutionError::RuleInputSequenceInvariant {
                previous_source: self.last_input_source,
                current_source: input.source(),
                violation: RuleInputSequenceViolation::MultipleInline,
            });
        }
        if !matches!(input, CascadeRuleInput::Inline(_)) {
            return Err(CascadeResolutionError::RuleInputSequenceInvariant {
                previous_source: self.last_input_source,
                current_source: input.source(),
                violation: RuleInputSequenceViolation::NonInlineAtInlineBoundary,
            });
        }
        self.inline_seen = true;
        self.last_input_source = Some(input.source());
        self.push_validated(input)
    }

    #[cfg(test)]
    fn push_checked(
        &mut self,
        input: CascadeRuleInput<'collection>,
    ) -> Result<(), CascadeResolutionError> {
        match input {
            CascadeRuleInput::Stylesheet(_) => self.push_stylesheet(input),
            CascadeRuleInput::Inline(_) => self.push_inline(input),
            #[cfg(test)]
            CascadeRuleInput::Compatibility(_) => self.push_validated(input),
        }
    }

    fn push_validated(
        &mut self,
        input: CascadeRuleInput<'collection>,
    ) -> Result<(), CascadeResolutionError> {
        let admitted = validate_and_count_rule_candidates(&input)?;
        self.admitted_candidate_count = self.admitted_candidate_count.checked_add(admitted).ok_or(
            CascadeResolutionError::CandidateLimitExceeded {
                required: usize::MAX,
                maximum: self.maximum_candidates,
            },
        )?;
        if self.admitted_candidate_count > self.maximum_candidates {
            return Err(CascadeResolutionError::CandidateLimitExceeded {
                required: self.admitted_candidate_count,
                maximum: self.maximum_candidates,
            });
        }
        self.inputs.push(input);
        Ok(())
    }

    pub(crate) fn finish(self) -> ValidatedCascadeRuleInputs<'collection> {
        ValidatedCascadeRuleInputs {
            inputs: self.inputs,
            admitted_candidate_count: self.admitted_candidate_count,
        }
    }
}

fn validate_and_count_rule_candidates(
    input: &CascadeRuleInput<'_>,
) -> Result<usize, CascadeResolutionError> {
    let declarations = input.declarations();
    let mut admitted = 0usize;
    let mut group_start = 0usize;
    let mut previous_source = None;
    let mut previous_order = None;

    for (position, declaration) in declarations.iter().enumerate() {
        let source = declaration.source();
        if previous_source != Some(source) {
            if previous_order.is_some_and(|order| declaration.declaration_order() <= order) {
                return Err(CascadeResolutionError::DeclarationSourceOrderInvariant {
                    rule_source: input.source(),
                    previous_source: previous_source
                        .expect("a previous order is accompanied by a previous source"),
                    current_source: source,
                    previous_order: previous_order
                        .expect("the declaration-order predicate observed an order"),
                    current_order: declaration.declaration_order(),
                });
            }
            group_start = position;
            previous_source = Some(source);
            previous_order = Some(declaration.declaration_order());
        }

        let Some(property) = declaration.applicability().supported_property() else {
            continue;
        };
        admitted =
            admitted
                .checked_add(1)
                .ok_or(CascadeResolutionError::CandidateLimitExceeded {
                    required: usize::MAX,
                    maximum: usize::MAX,
                })?;

        for earlier in &declarations[group_start..position] {
            if earlier.source() != source
                || earlier.applicability().supported_property() != Some(property)
            {
                continue;
            }
            let first_priority = input
                .context()
                .priority_for_declaration(earlier.importance(), earlier.declaration_order());
            let second_priority = input.context().priority_for_declaration(
                declaration.importance(),
                declaration.declaration_order(),
            );
            let same_value = earlier.value().semantically_eq(declaration.value());
            let same_expansion_metadata =
                earlier.expansion_order() == declaration.expansion_order();
            if first_priority == second_priority && same_value && same_expansion_metadata {
                return Err(CascadeResolutionError::DuplicateCandidateIdentity {
                    property,
                    first_source: earlier.source(),
                    second_source: source,
                    first_priority,
                    second_priority,
                });
            }
            let mismatch = match (
                first_priority == second_priority,
                same_value,
                same_expansion_metadata,
            ) {
                (_, _, false) => CandidateDataMismatch::ExpansionMetadata,
                (false, true, true) => CandidateDataMismatch::Priority,
                (true, false, true) => CandidateDataMismatch::Value,
                (false, false, true) => CandidateDataMismatch::PriorityAndValue,
                (true, true, true) => unreachable!(),
            };
            return Err(CascadeResolutionError::InconsistentCandidateIdentity {
                property,
                first_source: earlier.source(),
                second_source: source,
                first_priority,
                second_priority,
                mismatch,
            });
        }
    }
    Ok(admitted)
}

#[cfg(test)]
fn validate_checked_candidate_uniqueness(
    inputs: &ValidatedCascadeRuleInputs<'_>,
) -> Result<(), CascadeResolutionError> {
    let mut candidates: Vec<(
        super::properties::CascadePropertyId,
        CascadeDeclarationSource,
        super::priority::CascadePriority,
        &super::declarations::CascadeSpecifiedValue,
        u16,
    )> = Vec::new();
    for rule in inputs.inputs() {
        for declaration in rule.declarations() {
            let Some(property) = declaration.applicability().supported_property() else {
                continue;
            };
            let priority = rule.context().priority_for_declaration(
                declaration.importance(),
                declaration.declaration_order(),
            );
            for (
                first_property,
                first_source,
                first_priority,
                first_value,
                first_expansion_order,
            ) in &candidates
            {
                if *first_property != property {
                    continue;
                }
                if *first_source == declaration.source() {
                    let same_value = first_value.semantically_eq(declaration.value());
                    let same_expansion_metadata =
                        *first_expansion_order == declaration.expansion_order();
                    if *first_priority == priority && same_value && same_expansion_metadata {
                        return Err(CascadeResolutionError::DuplicateCandidateIdentity {
                            property,
                            first_source: *first_source,
                            second_source: declaration.source(),
                            first_priority: *first_priority,
                            second_priority: priority,
                        });
                    }
                    let mismatch = match (
                        *first_priority == priority,
                        same_value,
                        same_expansion_metadata,
                    ) {
                        (_, _, false) => CandidateDataMismatch::ExpansionMetadata,
                        (false, true, true) => CandidateDataMismatch::Priority,
                        (true, false, true) => CandidateDataMismatch::Value,
                        (false, false, true) => CandidateDataMismatch::PriorityAndValue,
                        (true, true, true) => unreachable!(),
                    };
                    return Err(CascadeResolutionError::InconsistentCandidateIdentity {
                        property,
                        first_source: *first_source,
                        second_source: declaration.source(),
                        first_priority: *first_priority,
                        second_priority: priority,
                        mismatch,
                    });
                }
                if *first_priority == priority {
                    return Err(CascadeResolutionError::EqualPriorityDistinctCandidates {
                        property,
                        first_source: *first_source,
                        second_source: declaration.source(),
                        first_priority: *first_priority,
                        second_priority: priority,
                    });
                }
            }
            candidates.push((
                property,
                declaration.source(),
                priority,
                declaration.value(),
                declaration.expansion_order(),
            ));
        }
    }
    Ok(())
}

fn validate_declaration_sources(
    rule_source: CascadeRuleSource,
    declarations: &[CascadeDeclarationInput],
) -> Result<(), CascadeRuleInputBuildError> {
    if let Some((declaration_position, declaration)) = declarations
        .iter()
        .enumerate()
        .find(|(_, declaration)| !rule_source.owns_declaration_source(declaration.source()))
    {
        return Err(CascadeRuleInputBuildError {
            rule_source,
            declaration_source: declaration.source(),
            declaration_position,
        });
    }
    Ok(())
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn validated_input_storage_reservation_is_a_typed_cascade_failure() {
        let budget = CascadeResolutionBudget::try_new(1, 1, 1).unwrap();
        let mut builder = ValidatedCascadeRuleInputBuilder::new(budget);
        assert_eq!(
            builder.try_reserve_rule_inputs(usize::MAX),
            Err(CascadeResolutionError::RuleInputStorageReservationFailed {
                requested: usize::MAX,
            })
        );
    }
}

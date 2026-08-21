use std::ops::Range;

use super::declarations::declaration_inputs_from_model;
use super::limits::{StyleResolutionLimit, StyleResolutionLimits};
use super::source::{StylesheetCollectionInput, StylesheetConditionStatus};
use crate::cascade::contract::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeOrigin, DeclarationOrder,
    DeclarationSourceIndex, RawRuleIndex, SourceCoordinateError, StyleRulePosition,
    StylesheetDeclarationRef, StylesheetOrder, StylesheetRuleOrder, StylesheetRuleRef,
    StylesheetSourceId,
};
use crate::model;
use crate::selectors::{
    InvalidSelectorReason, SelectorList, SelectorNamespaceConstraint, UnsupportedSelectorFeature,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StorageRange {
    start: u32,
    len: u32,
}

impl StorageRange {
    fn from_bounds(start: usize, end: usize) -> Result<Self, SourceCoordinateError> {
        let len = end
            .checked_sub(start)
            .ok_or(SourceCoordinateError::CounterExhausted {
                coordinate: "collection-storage-range",
            })?;
        Ok(Self {
            start: u32::try_from(start).map_err(|_| SourceCoordinateError::Unrepresentable {
                coordinate: "collection-storage-start",
                value: start,
                maximum: u32::MAX as usize,
            })?,
            len: u32::try_from(len).map_err(|_| SourceCoordinateError::Unrepresentable {
                coordinate: "collection-storage-length",
                value: len,
                maximum: u32::MAX as usize,
            })?,
        })
    }

    fn as_range(self) -> Option<Range<usize>> {
        let start = self.start as usize;
        let end = start.checked_add(self.len as usize)?;
        Some(start..end)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CollectedStylesheet<'source> {
    source_id: StylesheetSourceId,
    order: StylesheetOrder,
    origin: CascadeOrigin,
    namespace_constraint: SelectorNamespaceConstraint,
    condition: StylesheetConditionStatus<'source>,
    rules: StorageRange,
}

impl<'source> CollectedStylesheet<'source> {
    pub const fn source_id(&self) -> StylesheetSourceId {
        self.source_id
    }
    pub const fn order(&self) -> StylesheetOrder {
        self.order
    }
    pub const fn origin(&self) -> CascadeOrigin {
        self.origin
    }
    pub const fn namespace_constraint(&self) -> SelectorNamespaceConstraint {
        self.namespace_constraint
    }
    pub const fn condition(&self) -> StylesheetConditionStatus<'source> {
        self.condition
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CollectedRule<'source> {
    ActiveStyle(ActiveCollectedStyleRule<'source>),
    InactiveStyle(InactiveCollectedStyleRule<'source>),
    SkippedAtRule(CollectedAtRule<'source>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActiveCollectedStyleRule<'source> {
    rule_ref: StylesheetRuleRef,
    origin: CascadeOrigin,
    namespace_constraint: SelectorNamespaceConstraint,
    style_position: StyleRulePosition,
    source_order: StylesheetRuleOrder,
    selectors: &'source SelectorList,
    declarations: StorageRange,
}

impl<'source> ActiveCollectedStyleRule<'source> {
    pub const fn rule_ref(&self) -> StylesheetRuleRef {
        self.rule_ref
    }
    pub const fn origin(&self) -> CascadeOrigin {
        self.origin
    }
    pub const fn namespace_constraint(&self) -> SelectorNamespaceConstraint {
        self.namespace_constraint
    }
    pub const fn style_position(&self) -> StyleRulePosition {
        self.style_position
    }
    pub const fn source_order(&self) -> StylesheetRuleOrder {
        self.source_order
    }
    pub const fn selectors(&self) -> &'source SelectorList {
        self.selectors
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InactiveCollectedStyleRule<'source> {
    rule_ref: StylesheetRuleRef,
    style_position: StyleRulePosition,
    reason: InactiveStyleRuleReason<'source>,
}

impl<'source> InactiveCollectedStyleRule<'source> {
    pub const fn rule_ref(&self) -> StylesheetRuleRef {
        self.rule_ref
    }
    pub const fn style_position(&self) -> StyleRulePosition {
        self.style_position
    }
    pub const fn reason(&self) -> &InactiveStyleRuleReason<'source> {
        &self.reason
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InactiveStyleRuleReason<'source> {
    StylesheetConditionDeferred {
        raw: &'source str,
    },
    InvalidSelector {
        reason: InvalidSelectorReason,
    },
    UnsupportedSelector {
        features: &'source [UnsupportedSelectorFeature],
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CollectedAtRule<'source> {
    rule_ref: StylesheetRuleRef,
    name: Option<&'source str>,
    reason: AtRuleSkipReason,
}

impl<'source> CollectedAtRule<'source> {
    pub const fn rule_ref(&self) -> StylesheetRuleRef {
        self.rule_ref
    }
    pub const fn name(&self) -> Option<&'source str> {
        self.name
    }
    pub const fn reason(&self) -> AtRuleSkipReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtRuleSkipReason {
    MediaDeferred,
    SupportsDeferred,
    ImportDeferred,
    Unknown,
    UnresolvedName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleCollectionStorage {
    Stylesheets,
    Rules,
    Declarations,
}

impl RuleCollectionStorage {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Stylesheets => "stylesheets",
            Self::Rules => "rules",
            Self::Declarations => "declarations",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleCollectionBuildError {
    UnsupportedConfiguration {
        limit: StyleResolutionLimit,
        configured: usize,
        maximum: usize,
    },
    LimitExceeded {
        limit: StyleResolutionLimit,
        configured: usize,
        observed: usize,
    },
    DuplicateSourceId {
        source_id: StylesheetSourceId,
    },
    DuplicateStylesheetOrder {
        order: StylesheetOrder,
    },
    NonMonotonicStylesheetOrder {
        previous: StylesheetOrder,
        current: StylesheetOrder,
    },
    SelectorStateInvariant {
        source_id: StylesheetSourceId,
        raw_rule_index: RawRuleIndex,
    },
    Coordinate(SourceCoordinateError),
    Reservation {
        storage: RuleCollectionStorage,
    },
}

impl RuleCollectionBuildError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::UnsupportedConfiguration { .. } => "unsupported-configuration",
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::DuplicateSourceId { .. } => "duplicate-source-id",
            Self::DuplicateStylesheetOrder { .. } => "duplicate-stylesheet-order",
            Self::NonMonotonicStylesheetOrder { .. } => "non-monotonic-stylesheet-order",
            Self::SelectorStateInvariant { .. } => "selector-state-invariant",
            Self::Coordinate(error) => error.stable_label(),
            Self::Reservation { .. } => "reservation",
        }
    }
}

impl From<SourceCoordinateError> for RuleCollectionBuildError {
    fn from(error: SourceCoordinateError) -> Self {
        Self::Coordinate(error)
    }
}

impl std::fmt::Display for RuleCollectionBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedConfiguration {
                limit,
                configured,
                maximum,
            } => write!(
                formatter,
                "rule collection configured {} limit {configured} above representable maximum {maximum}",
                limit.stable_label()
            ),
            Self::LimitExceeded {
                limit,
                configured,
                observed,
            } => write!(
                formatter,
                "rule collection observed {observed} entries above {} limit {configured}",
                limit.stable_label()
            ),
            Self::DuplicateSourceId { source_id } => write!(
                formatter,
                "duplicate stylesheet source id {}",
                source_id.get()
            ),
            Self::DuplicateStylesheetOrder { order } => {
                write!(formatter, "duplicate stylesheet order {}", order.get())
            }
            Self::NonMonotonicStylesheetOrder { previous, current } => write!(
                formatter,
                "stylesheet order {} follows non-earlier order {}",
                current.get(),
                previous.get()
            ),
            Self::SelectorStateInvariant {
                source_id,
                raw_rule_index,
            } => write!(
                formatter,
                "stylesheet source {} raw rule {} has no classified selector state",
                source_id.get(),
                raw_rule_index.get()
            ),
            Self::Coordinate(error) => write!(formatter, "{error}"),
            Self::Reservation { storage } => write!(
                formatter,
                "failed to reserve rule collection {} storage",
                storage.stable_label()
            ),
        }
    }
}

impl std::error::Error for RuleCollectionBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            _ => None,
        }
    }
}

/// Immutable, CSS-owned stylesheet collection for one style execution.
pub struct RuleCollection<'source> {
    stylesheets: Vec<CollectedStylesheet<'source>>,
    rules: Vec<CollectedRule<'source>>,
    declarations: Vec<CascadeDeclarationInput>,
}

impl<'source> RuleCollection<'source> {
    pub fn try_new(
        inputs: &[StylesheetCollectionInput<'source>],
        limits: &StyleResolutionLimits,
    ) -> Result<Self, RuleCollectionBuildError> {
        validate_collection_limit(
            StyleResolutionLimit::StylesheetsPerStylePass,
            limits.max_stylesheets_per_style_pass,
        )?;
        validate_collection_limit(
            StyleResolutionLimit::TopLevelRulesPerDocument,
            limits.max_top_level_rules_per_document,
        )?;
        validate_collection_limit(
            StyleResolutionLimit::CollectedDeclarationInputsPerDocument,
            limits.max_collected_declaration_inputs_per_document,
        )?;
        if inputs.len() > limits.max_stylesheets_per_style_pass {
            return Err(RuleCollectionBuildError::LimitExceeded {
                limit: StyleResolutionLimit::StylesheetsPerStylePass,
                configured: limits.max_stylesheets_per_style_pass,
                observed: inputs.len(),
            });
        }

        let mut stylesheets = Vec::new();
        try_reserve_collection_storage(
            &mut stylesheets,
            inputs.len(),
            RuleCollectionStorage::Stylesheets,
        )?;
        let mut rules = Vec::new();
        let mut declarations = Vec::new();
        let mut previous_order = None;

        for input in inputs.iter().copied() {
            if stylesheets
                .iter()
                .any(|sheet: &CollectedStylesheet<'_>| sheet.source_id == input.source_id())
            {
                return Err(RuleCollectionBuildError::DuplicateSourceId {
                    source_id: input.source_id(),
                });
            }
            if let Some(previous) = previous_order {
                match input.order().cmp(&previous) {
                    std::cmp::Ordering::Less => {
                        return Err(RuleCollectionBuildError::NonMonotonicStylesheetOrder {
                            previous,
                            current: input.order(),
                        });
                    }
                    std::cmp::Ordering::Equal => {
                        return Err(RuleCollectionBuildError::DuplicateStylesheetOrder {
                            order: input.order(),
                        });
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            previous_order = Some(input.order());

            let model_rules = &input.stylesheet().stylesheet.rules;
            let prospective = rules.len().checked_add(model_rules.len()).ok_or(
                SourceCoordinateError::CounterExhausted {
                    coordinate: "top-level-rule-count",
                },
            )?;
            if prospective > limits.max_top_level_rules_per_document {
                return Err(RuleCollectionBuildError::LimitExceeded {
                    limit: StyleResolutionLimit::TopLevelRulesPerDocument,
                    configured: limits.max_top_level_rules_per_document,
                    observed: prospective,
                });
            }
            try_reserve_collection_storage(
                &mut rules,
                model_rules.len(),
                RuleCollectionStorage::Rules,
            )?;

            let rule_start = rules.len();
            let condition = input.condition().classify();
            let mut style_position = 0usize;

            for (raw_index, rule) in model_rules.iter().enumerate() {
                let raw_rule_index = RawRuleIndex::from_usize(raw_index)?;
                let rule_ref = StylesheetRuleRef::new(input.source_id(), raw_rule_index);
                match rule {
                    model::Rule::At(at_rule) => {
                        rules.push(CollectedRule::SkippedAtRule(CollectedAtRule {
                            rule_ref,
                            name: at_rule.name.as_deref(),
                            reason: at_rule_skip_reason(at_rule.name.as_deref()),
                        }))
                    }
                    model::Rule::Style(style_rule) => {
                        let current_style_position = StyleRulePosition::from_usize(style_position)?;
                        style_position = style_position.checked_add(1).ok_or(
                            SourceCoordinateError::CounterExhausted {
                                coordinate: "style-rule-position",
                            },
                        )?;

                        let inactive_reason =
                            if let StylesheetConditionStatus::DeferredUnsupported { raw } =
                                condition
                            {
                                Some(InactiveStyleRuleReason::StylesheetConditionDeferred { raw })
                            } else if let Some(invalid) = style_rule.selectors.invalid() {
                                Some(InactiveStyleRuleReason::InvalidSelector {
                                    reason: invalid.reason(),
                                })
                            } else {
                                style_rule.selectors.unsupported().map(|unsupported| {
                                    InactiveStyleRuleReason::UnsupportedSelector {
                                        features: unsupported.features(),
                                    }
                                })
                            };

                        if let Some(reason) = inactive_reason {
                            rules.push(CollectedRule::InactiveStyle(InactiveCollectedStyleRule {
                                rule_ref,
                                style_position: current_style_position,
                                reason,
                            }));
                            continue;
                        }

                        let Some(selectors) = style_rule.selectors.parsed() else {
                            return Err(RuleCollectionBuildError::SelectorStateInvariant {
                                source_id: input.source_id(),
                                raw_rule_index,
                            });
                        };
                        let declaration_start = declarations.len();
                        for (index, declaration) in
                            style_rule.declarations.declarations.iter().enumerate()
                        {
                            let declaration_index = DeclarationSourceIndex::from_usize(index)?;
                            let declaration_order = DeclarationOrder::from_usize(index)?;
                            let inputs = declaration_inputs_from_model(
                                CascadeDeclarationSource::Stylesheet(
                                    StylesheetDeclarationRef::new(
                                        input.source_id(),
                                        raw_rule_index,
                                        declaration_index,
                                    ),
                                ),
                                declaration_order,
                                declaration,
                            );
                            let observed = declarations.len().checked_add(inputs.len()).ok_or(
                                SourceCoordinateError::CounterExhausted {
                                    coordinate: "collected-declaration-count",
                                },
                            )?;
                            if observed > limits.max_collected_declaration_inputs_per_document {
                                return Err(RuleCollectionBuildError::LimitExceeded {
                                    limit:
                                        StyleResolutionLimit::CollectedDeclarationInputsPerDocument,
                                    configured: limits
                                        .max_collected_declaration_inputs_per_document,
                                    observed,
                                });
                            }
                            try_reserve_collection_storage(
                                &mut declarations,
                                inputs.len(),
                                RuleCollectionStorage::Declarations,
                            )?;
                            declarations.extend(inputs);
                        }
                        let declaration_range =
                            StorageRange::from_bounds(declaration_start, declarations.len())?;
                        rules.push(CollectedRule::ActiveStyle(ActiveCollectedStyleRule {
                            rule_ref,
                            origin: input.origin(),
                            namespace_constraint: input.namespace_constraint(),
                            style_position: current_style_position,
                            source_order: StylesheetRuleOrder::new(
                                input.order(),
                                current_style_position,
                            ),
                            selectors,
                            declarations: declaration_range,
                        }));
                    }
                }
            }
            stylesheets.push(CollectedStylesheet {
                source_id: input.source_id(),
                order: input.order(),
                origin: input.origin(),
                namespace_constraint: input.namespace_constraint(),
                condition,
                rules: StorageRange::from_bounds(rule_start, rules.len())?,
            });
        }

        Ok(Self {
            stylesheets,
            rules,
            declarations,
        })
    }

    pub(crate) fn stylesheets(&self) -> &[CollectedStylesheet<'source>] {
        &self.stylesheets
    }
    pub(crate) fn rules(&self) -> &[CollectedRule<'source>] {
        &self.rules
    }
    pub(crate) fn declarations(&self) -> &[CascadeDeclarationInput] {
        &self.declarations
    }

    pub(crate) fn declarations_for_rule(
        &self,
        rule: &ActiveCollectedStyleRule<'source>,
    ) -> &[CascadeDeclarationInput] {
        let range = rule
            .declarations
            .as_range()
            .expect("checked collection declaration range remains representable");
        self.declarations
            .get(range)
            .expect("active rule declaration range belongs to this collection")
    }
}

fn try_reserve_collection_storage<T>(
    storage: &mut Vec<T>,
    additional: usize,
    kind: RuleCollectionStorage,
) -> Result<(), RuleCollectionBuildError> {
    storage
        .try_reserve(additional)
        .map_err(|_| RuleCollectionBuildError::Reservation { storage: kind })
}

fn validate_collection_limit(
    limit: StyleResolutionLimit,
    configured: usize,
) -> Result<(), RuleCollectionBuildError> {
    let maximum = u32::MAX as usize;
    if configured > maximum {
        return Err(RuleCollectionBuildError::UnsupportedConfiguration {
            limit,
            configured,
            maximum,
        });
    }
    Ok(())
}

fn at_rule_skip_reason(name: Option<&str>) -> AtRuleSkipReason {
    let Some(name) = name else {
        return AtRuleSkipReason::UnresolvedName;
    };
    if name.eq_ignore_ascii_case("media") {
        AtRuleSkipReason::MediaDeferred
    } else if name.eq_ignore_ascii_case("supports") {
        AtRuleSkipReason::SupportsDeferred
    } else if name.eq_ignore_ascii_case("import") {
        AtRuleSkipReason::ImportDeferred
    } else {
        AtRuleSkipReason::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_container_reservation_failure_is_typed() {
        let mut storage = Vec::<u8>::new();
        assert_eq!(
            try_reserve_collection_storage(
                &mut storage,
                usize::MAX,
                RuleCollectionStorage::Declarations,
            ),
            Err(RuleCollectionBuildError::Reservation {
                storage: RuleCollectionStorage::Declarations,
            })
        );
    }
}

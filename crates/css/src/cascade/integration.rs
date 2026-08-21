mod collection;
mod collection_diagnostic;
mod debug_snapshot;
mod declarations;
mod limits;
mod rule_inputs;
mod selector_dom;
mod source;

#[cfg(test)]
pub(crate) use self::collection::{
    ActiveCollectedStyleRule, CollectedRule, InactiveStyleRuleReason,
};
pub use self::collection::{
    AtRuleSkipReason, RuleCollection, RuleCollectionBuildError, RuleCollectionStorage,
};
pub use self::collection_diagnostic::{
    BoundedDiagnosticText, DiagnosticCondition, DiagnosticDeclarationProperty, DiagnosticRuleState,
    RULE_COLLECTION_DIAGNOSTIC_VERSION, RuleCollectionDiagnostic, RuleCollectionDiagnosticFailure,
    RuleCollectionDiagnosticLimit, RuleCollectionDiagnosticLimits, RuleCollectionDiagnosticRecord,
    RuleCollectionDiagnosticSnapshot, RuleCollectionDiagnosticStorage, rule_collection_diagnostic,
};
pub use self::debug_snapshot::{
    declaration_list_pipeline_debug_snapshot, resolve_document_styles_debug_snapshot,
};
pub use self::limits::{StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits};
pub(crate) use self::source::StylesheetConditionStatus;
pub use self::source::{
    StylesheetCollectionInput, StylesheetCollectionInputBuildError, StylesheetConditionInput,
    get_inline_style, is_css,
};

#[cfg(test)]
pub(crate) use self::declarations::{
    declaration_classification_count, reset_declaration_classification_count,
};

use self::limits::validate_representation_limits;
use self::rule_inputs::rule_inputs_for_element_with_limits;
use self::selector_dom::{
    build_document_selector_dom_with_element_limit,
    build_element_subtree_selector_dom_with_element_limit,
};
use super::contract::{
    StylesheetOrder, StylesheetSourceId, resolve_cascade_style_from_rule_inputs,
};
use super::document::{ResolvedDocumentStyle, ResolvedElementStyle};
use crate::model;
use crate::selectors::{SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment};
use html::{ElementNode, Node, internal::Id};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncrementalStyleResolutionStats {
    pub reused_prefix_len: usize,
    pub recomputed_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncrementalResolvedDocumentStyle {
    pub resolved: ResolvedDocumentStyle,
    pub stats: IncrementalStyleResolutionStats,
}

/// One pass-scoped selector-DOM and rule-collection view. Browser orchestration
/// can reuse this value when an incremental attempt falls back to a full pass.
pub struct StyleResolutionExecution<'dom, 'collection, 'source> {
    root: &'dom Node,
    index: SelectorDomIndex<'dom>,
    matching_environment: SelectorMatchingEnvironment,
    collection: &'collection RuleCollection<'source>,
    limits: StyleResolutionLimits,
}

impl<'dom, 'collection, 'source> StyleResolutionExecution<'dom, 'collection, 'source> {
    pub fn try_new(
        root: &'dom Node,
        matching_environment: SelectorMatchingEnvironment,
        collection: &'collection RuleCollection<'source>,
        limits: &StyleResolutionLimits,
    ) -> Result<Self, StyleResolutionError> {
        validate_representation_limits(limits)?;
        let index = build_document_selector_dom_with_element_limit(
            root,
            limits.max_styled_elements_per_document,
        )?;
        Ok(Self {
            root,
            index,
            matching_environment,
            collection,
            limits: *limits,
        })
    }

    pub(crate) fn root(&self) -> &'dom Node {
        self.root
    }

    pub fn resolve_document_styles(&self) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
        resolve_document_styles_with_index(
            &self.index,
            self.matching_environment,
            self.collection,
            &self.limits,
        )
    }

    pub fn resolve_document_styles_incremental_suffix(
        &self,
        previous: &ResolvedDocumentStyle,
        dirty_node_ids: &[Id],
    ) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
        resolve_document_styles_incremental_suffix_with_index(
            &self.index,
            self.matching_environment,
            self.collection,
            previous,
            dirty_node_ids,
            &self.limits,
        )
    }
}

pub fn resolve_document_styles(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    try_resolve_document_styles_with_limits(
        root,
        matching_environment,
        sheets,
        &StyleResolutionLimits::default(),
    )
}

pub fn resolve_document_styles_from_cascade_inputs(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCollectionInput<'_>],
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    try_resolve_document_styles_from_cascade_inputs_with_limits(
        root,
        matching_environment,
        sheets,
        &StyleResolutionLimits::default(),
    )
}

pub fn try_resolve_document_styles_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    let inputs =
        compatibility_author_inputs(sheets).map_err(StyleResolutionError::StylesheetInputBuild)?;
    let collection = RuleCollection::try_new(&inputs, limits)
        .map_err(StyleResolutionError::RuleCollectionBuild)?;
    try_resolve_document_styles_from_rule_collection_with_limits(
        root,
        matching_environment,
        &collection,
        limits,
    )
}

pub fn try_resolve_document_styles_from_cascade_inputs_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCollectionInput<'_>],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    let collection = RuleCollection::try_new(sheets, limits)
        .map_err(StyleResolutionError::RuleCollectionBuild)?;
    try_resolve_document_styles_from_rule_collection_with_limits(
        root,
        matching_environment,
        &collection,
        limits,
    )
}

pub fn try_resolve_document_styles_from_rule_collection_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    collection: &RuleCollection<'_>,
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    StyleResolutionExecution::try_new(root, matching_environment, collection, limits)?
        .resolve_document_styles()
}

/// Allocation-regression seam for AF5's borrowed matched-rule contract.
///
/// This is available only with the opt-in allocation-counting feature. It
/// performs production selector matching and rule-input construction without
/// materializing AF6/R candidates or winners.
#[cfg(feature = "count-alloc")]
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Af5AllocationGuardError {
    StyleResolution(StyleResolutionError),
    CounterExhausted { counter: &'static str },
}

#[cfg(feature = "count-alloc")]
impl From<StyleResolutionError> for Af5AllocationGuardError {
    fn from(error: StyleResolutionError) -> Self {
        Self::StyleResolution(error)
    }
}

#[cfg(feature = "count-alloc")]
impl std::fmt::Display for Af5AllocationGuardError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StyleResolution(error) => write!(formatter, "{error}"),
            Self::CounterExhausted { counter } => {
                write!(
                    formatter,
                    "AF5 allocation guard {counter} counter exhausted"
                )
            }
        }
    }
}

#[cfg(feature = "count-alloc")]
impl std::error::Error for Af5AllocationGuardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StyleResolution(error) => Some(error),
            Self::CounterExhausted { .. } => None,
        }
    }
}

#[cfg(feature = "count-alloc")]
#[doc(hidden)]
pub fn af5_match_rule_inputs_for_allocation_guard(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    collection: &RuleCollection<'_>,
    limits: &StyleResolutionLimits,
) -> Result<(usize, usize), Af5AllocationGuardError> {
    validate_representation_limits(limits)?;
    let index = build_document_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;
    let context = SelectorMatchingContext::with_limits(
        &index,
        matching_environment,
        limits.selector_matching,
    );
    let mut matched_rules = 0usize;
    let mut borrowed_declarations = 0usize;
    for element in index.elements() {
        let inputs =
            rule_inputs_for_element_with_limits(&index, &context, element, collection, limits)?;
        for input in inputs {
            if matches!(input, super::contract::CascadeRuleInput::Stylesheet(_)) {
                matched_rules = matched_rules.checked_add(1).ok_or(
                    Af5AllocationGuardError::CounterExhausted {
                        counter: "matched-rule",
                    },
                )?;
                borrowed_declarations = borrowed_declarations
                    .checked_add(input.declarations().len())
                    .ok_or(Af5AllocationGuardError::CounterExhausted {
                        counter: "borrowed-declaration",
                    })?;
            }
        }
    }
    Ok((matched_rules, borrowed_declarations))
}

fn resolve_document_styles_with_index(
    index: &SelectorDomIndex<'_>,
    matching_environment: SelectorMatchingEnvironment,
    collection: &RuleCollection<'_>,
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    let context =
        SelectorMatchingContext::with_limits(index, matching_environment, limits.selector_matching);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for element in index.elements() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));
        let rule_inputs =
            rule_inputs_for_element_with_limits(index, &context, element, collection, limits)?;
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);
        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_namespace(element),
            context.element_local_name(element).to_string(),
            style,
        ));
    }

    Ok(ResolvedDocumentStyle::new(matching_environment, entries))
}

pub fn try_resolve_document_styles_incremental_suffix_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    previous: &ResolvedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
    validate_representation_limits(limits)?;
    let inputs =
        compatibility_author_inputs(sheets).map_err(StyleResolutionError::StylesheetInputBuild)?;
    let collection = RuleCollection::try_new(&inputs, limits)
        .map_err(StyleResolutionError::RuleCollectionBuild)?;
    try_resolve_document_styles_incremental_suffix_from_rule_collection_with_limits(
        root,
        matching_environment,
        &collection,
        previous,
        dirty_node_ids,
        limits,
    )
}

pub fn try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCollectionInput<'_>],
    previous: &ResolvedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
    validate_representation_limits(limits)?;
    let collection = RuleCollection::try_new(sheets, limits)
        .map_err(StyleResolutionError::RuleCollectionBuild)?;
    try_resolve_document_styles_incremental_suffix_from_rule_collection_with_limits(
        root,
        matching_environment,
        &collection,
        previous,
        dirty_node_ids,
        limits,
    )
}

pub fn try_resolve_document_styles_incremental_suffix_from_rule_collection_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    collection: &RuleCollection<'_>,
    previous: &ResolvedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
    StyleResolutionExecution::try_new(root, matching_environment, collection, limits)?
        .resolve_document_styles_incremental_suffix(previous, dirty_node_ids)
}

fn resolve_document_styles_incremental_suffix_with_index(
    index: &SelectorDomIndex<'_>,
    matching_environment: SelectorMatchingEnvironment,
    collection: &RuleCollection<'_>,
    previous: &ResolvedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
    if previous.matching_environment() != matching_environment {
        return Err(StyleResolutionError::MatchingEnvironmentMismatch {
            expected: matching_environment,
            actual: previous.matching_environment(),
        });
    }
    if dirty_node_ids.is_empty() || previous.entries().len() != index.len() {
        return Ok(None);
    }
    let Some(reused_prefix_len) = earliest_dirty_element_index(index, dirty_node_ids) else {
        return Ok(None);
    };

    let context =
        SelectorMatchingContext::with_limits(index, matching_environment, limits.selector_matching);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for (element_index, element) in index.elements().enumerate() {
        if element_index < reused_prefix_len {
            let Some(previous_entry) = previous.entries().get(element_index) else {
                return Ok(None);
            };
            if previous_entry.selector_element_id() != element
                || previous_entry.element_namespace() != context.element_namespace(element)
                || previous_entry.element_name() != context.element_local_name(element)
            {
                return Ok(None);
            }
            styles_by_element.insert(element, previous_entry.style().clone());
            entries.push(previous_entry.clone());
            continue;
        }

        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));
        let rule_inputs =
            rule_inputs_for_element_with_limits(index, &context, element, collection, limits)?;
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);
        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_namespace(element),
            context.element_local_name(element).to_string(),
            style,
        ));
    }

    Ok(Some(IncrementalResolvedDocumentStyle {
        resolved: ResolvedDocumentStyle::new(matching_environment, entries),
        stats: IncrementalStyleResolutionStats {
            reused_prefix_len,
            recomputed_len: index.len() - reused_prefix_len,
        },
    }))
}

pub(crate) fn try_resolve_element_subtree_styles_with_limits(
    root: &ElementNode,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    let inputs =
        compatibility_author_inputs(sheets).map_err(StyleResolutionError::StylesheetInputBuild)?;
    let collection = RuleCollection::try_new(&inputs, limits)
        .map_err(StyleResolutionError::RuleCollectionBuild)?;
    let index = build_element_subtree_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;
    resolve_document_styles_with_index(&index, matching_environment, &collection, limits)
}

fn compatibility_author_inputs<'source>(
    sheets: &'source [model::StylesheetParse],
) -> Result<Vec<StylesheetCollectionInput<'source>>, StylesheetCollectionInputBuildError> {
    let mut inputs = Vec::new();
    try_reserve_compatibility_inputs(&mut inputs, sheets.len())?;
    for (index, stylesheet) in sheets.iter().enumerate() {
        inputs.push(compatibility_author_input(index, stylesheet)?);
    }
    Ok(inputs)
}

fn try_reserve_compatibility_inputs<'source>(
    inputs: &mut Vec<StylesheetCollectionInput<'source>>,
    additional: usize,
) -> Result<(), StylesheetCollectionInputBuildError> {
    inputs
        .try_reserve(additional)
        .map_err(|_| StylesheetCollectionInputBuildError::Reservation)
}

fn compatibility_author_input(
    index: usize,
    stylesheet: &model::StylesheetParse,
) -> Result<StylesheetCollectionInput<'_>, StylesheetCollectionInputBuildError> {
    let order = StylesheetOrder::from_usize(index)?;
    Ok(StylesheetCollectionInput::author(
        StylesheetSourceId::compatibility_generation_index(order.get()),
        order,
        stylesheet,
        StylesheetConditionInput::None,
    ))
}

fn earliest_dirty_element_index(
    index: &SelectorDomIndex<'_>,
    dirty_node_ids: &[Id],
) -> Option<usize> {
    dirty_node_ids
        .iter()
        .filter_map(|node_id| index.element_for_node_id(*node_id))
        .map(|element| (element.get() - 1) as usize)
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_input_coordinate_failure_stays_in_input_build_taxonomy() {
        let stylesheet =
            model::parse_stylesheet_with_options("", &crate::syntax::ParseOptions::stylesheet());
        let error = compatibility_author_input((u32::MAX as usize) + 1, &stylesheet)
            .expect_err("compatibility stylesheet order is u32-backed");
        assert!(matches!(
            error,
            StylesheetCollectionInputBuildError::Coordinate(
                super::super::contract::SourceCoordinateError::Unrepresentable {
                    coordinate: "stylesheet-order",
                    ..
                }
            )
        ));
        let execution_error = StyleResolutionError::StylesheetInputBuild(error);
        assert_eq!(execution_error.stable_label(), "coordinate");
    }

    #[test]
    fn compatibility_input_reservation_failure_stays_in_input_build_taxonomy() {
        let mut inputs = Vec::new();
        assert_eq!(
            try_reserve_compatibility_inputs(&mut inputs, usize::MAX),
            Err(StylesheetCollectionInputBuildError::Reservation)
        );
    }
}

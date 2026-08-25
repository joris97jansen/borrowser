//! CSS-owned style-input invalidation contracts.
//!
//! Browser/runtime reports what changed through [`StyleChangeFacts`]. CSS
//! classifies that fact into an opaque plan describing which retained style
//! results remain semantically reusable. The plan deliberately does not
//! describe the computation that will eventually execute; retained artifact
//! availability and identity validation remain runtime concerns.

use std::{fmt::Write, num::NonZeroUsize};

use html::internal::Id;

mod dependencies;

use dependencies::DependencyClassificationFailure;
pub use dependencies::{STYLE_DEPENDENCY_ARTIFACT_DEBUG_VERSION, StyleDependencyArtifact};

/// CSS-owned, invariant-safe facts for one changed-node mutation dimension.
///
/// `occurred` is deliberately distinct from `node_ids`: a valid mutation may
/// target only nodes that no longer survive in the published tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedStyleNodeFacts {
    occurred: bool,
    node_ids: Vec<Id>,
    historical_target_count: usize,
}

impl ChangedStyleNodeFacts {
    #[must_use]
    pub fn unchanged() -> Self {
        Self {
            occurred: false,
            node_ids: Vec::new(),
            historical_target_count: 0,
        }
    }

    #[must_use]
    pub fn changed(node_ids: impl IntoIterator<Item = Id>) -> Self {
        Self {
            occurred: true,
            node_ids: canonicalize_node_ids(node_ids.into_iter().collect()),
            historical_target_count: 0,
        }
    }

    #[must_use]
    pub fn changed_with_historical_targets(
        node_ids: impl IntoIterator<Item = Id>,
        historical_target_count: usize,
    ) -> Self {
        Self {
            occurred: true,
            node_ids: canonicalize_node_ids(node_ids.into_iter().collect()),
            historical_target_count,
        }
    }

    #[must_use]
    pub fn occurred(&self) -> bool {
        self.occurred
    }

    #[must_use]
    pub fn node_ids(&self) -> &[Id] {
        &self.node_ids
    }

    #[must_use]
    pub fn historical_target_count(&self) -> usize {
        self.historical_target_count
    }
}

/// Complete neutral DOM facts for one publication, as accepted by CSS.
///
/// Fields are private so callers cannot create contradictory states. Browser
/// supplies neutral facts through [`DomStyleChangeFactsBuilder`]; CSS alone
/// assigns selector/style meaning to the aggregate publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomStyleChangeFacts {
    document_replaced: bool,
    ordinary_nodes_allocated: bool,
    tree_topology_or_order_operation: bool,
    template_contents_associated: bool,
    attributes: ChangedStyleNodeFacts,
    text: ChangedStyleNodeFacts,
    unclassified_patch_count: usize,
}

impl DomStyleChangeFacts {
    #[must_use]
    pub fn builder() -> DomStyleChangeFactsBuilder {
        DomStyleChangeFactsBuilder::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomStyleChangeFactsBuilder {
    facts: DomStyleChangeFacts,
}

impl DomStyleChangeFactsBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            facts: DomStyleChangeFacts {
                document_replaced: false,
                ordinary_nodes_allocated: false,
                tree_topology_or_order_operation: false,
                template_contents_associated: false,
                attributes: ChangedStyleNodeFacts::unchanged(),
                text: ChangedStyleNodeFacts::unchanged(),
                unclassified_patch_count: 0,
            },
        }
    }

    #[must_use]
    pub fn document_replaced(mut self) -> Self {
        self.facts.document_replaced = true;
        self
    }

    #[must_use]
    pub fn ordinary_nodes_allocated(mut self) -> Self {
        self.facts.ordinary_nodes_allocated = true;
        self
    }

    #[must_use]
    pub fn tree_topology_or_order_operation(mut self) -> Self {
        self.facts.tree_topology_or_order_operation = true;
        self
    }

    #[must_use]
    pub fn template_contents_associated(mut self) -> Self {
        self.facts.template_contents_associated = true;
        self
    }

    #[must_use]
    pub fn attributes(mut self, attributes: ChangedStyleNodeFacts) -> Self {
        self.facts.attributes = attributes;
        self
    }

    #[must_use]
    pub fn text(mut self, text: ChangedStyleNodeFacts) -> Self {
        self.facts.text = text;
        self
    }

    #[must_use]
    pub fn unclassified_patches(mut self, count: NonZeroUsize) -> Self {
        self.facts.unclassified_patch_count = count.get();
        self
    }

    #[must_use]
    pub fn build(self) -> DomStyleChangeFacts {
        self.facts
    }
}

impl Default for DomStyleChangeFactsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleChangeFacts {
    DomPublication(DomStyleChangeFacts),
    StylesheetSetChanged,
}

#[derive(Clone, Copy, Debug)]
pub struct DomStyleAttributeMutation<'a> {
    node_id: Id,
    element_namespace: html::ElementNamespace,
    before: Option<&'a [html::ParserCreatedAttribute]>,
    after: &'a [html::ParserCreatedAttribute],
}

impl<'a> DomStyleAttributeMutation<'a> {
    pub fn new(
        node_id: Id,
        element_namespace: html::ElementNamespace,
        before: Option<&'a [html::ParserCreatedAttribute]>,
        after: &'a [html::ParserCreatedAttribute],
    ) -> Self {
        Self {
            node_id,
            element_namespace,
            before,
            after,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DomStyleTextMutation<'a> {
    node_id: Id,
    parent_element: Option<(Id, html::ElementNamespace)>,
    before: Option<&'a str>,
    after: &'a str,
}

impl<'a> DomStyleTextMutation<'a> {
    pub fn new(
        node_id: Id,
        parent_element: Option<(Id, html::ElementNamespace)>,
        before: Option<&'a str>,
        after: &'a str,
    ) -> Self {
        Self {
            node_id,
            parent_element,
            before,
            after,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StyleInvalidationInput<'a> {
    change: &'a StyleChangeFacts,
    dependency_artifact: Option<&'a StyleDependencyArtifact>,
    matching_environment: crate::SelectorMatchingEnvironment,
    attribute_mutations: Option<&'a [DomStyleAttributeMutation<'a>]>,
    text_mutations: Option<&'a [DomStyleTextMutation<'a>]>,
}

impl<'a> StyleInvalidationInput<'a> {
    pub fn new(
        change: &'a StyleChangeFacts,
        dependency_artifact: Option<&'a StyleDependencyArtifact>,
        matching_environment: crate::SelectorMatchingEnvironment,
    ) -> Self {
        Self {
            change,
            dependency_artifact,
            matching_environment,
            attribute_mutations: None,
            text_mutations: None,
        }
    }

    pub fn with_attribute_mutations(
        mut self,
        mutations: Option<&'a [DomStyleAttributeMutation<'a>]>,
    ) -> Self {
        self.attribute_mutations = mutations;
        self
    }

    pub fn with_text_mutations(
        mut self,
        mutations: Option<&'a [DomStyleTextMutation<'a>]>,
    ) -> Self {
        self.text_mutations = mutations;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleInvalidationDecision {
    plan: Option<StyleInvalidationPlan>,
    reason: StyleInvalidationReason,
    dependency_hits: DependencyHitSummary,
    inline_style_changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StyleInvalidationReason {
    NoStyleEffect,
    StylesheetSetChanged,
    DocumentReplaced,
    StructuralMutationRequiresFullRebuild,
    UnclassifiedMutation,
    MissingOrIncompatibleDependencies,
    DependencyEvaluationLimitExceeded,
    DependencyClassificationResourceUnavailable,
    ExactMutationDetailsUnavailable,
    HistoricalMutationTarget,
    SelectorDependencyMatched,
    InlineCascadeInputChanged,
    EmptyDependencyMatched,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DependencyHitSummary {
    id: bool,
    class: bool,
    attribute: bool,
    empty: bool,
}

impl StyleInvalidationDecision {
    #[cfg(test)]
    pub(crate) fn plan(&self) -> Option<&StyleInvalidationPlan> {
        self.plan.as_ref()
    }

    pub fn into_plan(self) -> Option<StyleInvalidationPlan> {
        self.plan
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write invalidation decision snapshot");
        writeln!(&mut out, "af9-style-invalidation-decision")
            .expect("write invalidation decision snapshot");
        writeln!(
            &mut out,
            "reason: {}",
            invalidation_reason_label(self.reason)
        )
        .expect("write invalidation decision snapshot");
        writeln!(
            &mut out,
            "dependency-hits: id={} class={} attribute={} empty={}",
            self.dependency_hits.id,
            self.dependency_hits.class,
            self.dependency_hits.attribute,
            self.dependency_hits.empty,
        )
        .expect("write invalidation decision snapshot");
        writeln!(
            &mut out,
            "inline-style-changed: {}",
            self.inline_style_changed
        )
        .expect("write invalidation decision snapshot");
        writeln!(
            &mut out,
            "selected-plan: {}",
            self.plan.as_ref().map_or_else(
                || "none".to_string(),
                StyleInvalidationPlan::to_debug_snapshot
            )
        )
        .expect("write invalidation decision snapshot");
        out
    }
}

impl StyleChangeFacts {
    #[must_use]
    pub fn dom_publication(facts: DomStyleChangeFacts) -> Self {
        Self::DomPublication(facts)
    }

    #[must_use]
    pub fn stylesheet_set_changed() -> Self {
        Self::StylesheetSetChanged
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleInvalidationPlan {
    kind: StyleInvalidationPlanKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StyleInvalidationPlanKind {
    DocumentSuffix { node_ids: Vec<Id> },
    FullDocument,
}

impl StyleInvalidationPlan {
    fn document_suffix(node_ids: Vec<Id>) -> Option<Self> {
        let node_ids = canonicalize_node_ids(node_ids);
        (!node_ids.is_empty()).then_some(Self {
            kind: StyleInvalidationPlanKind::DocumentSuffix { node_ids },
        })
    }

    fn full_document() -> Self {
        Self {
            kind: StyleInvalidationPlanKind::FullDocument,
        }
    }

    /// Returns whether CSS proved that all retained style artifacts are stale.
    ///
    /// This is a CSS-owned behavioral query, not a public semantic enum. The
    /// Browser/runtime uses it only to apply the resulting retained-artifact
    /// policy; it cannot construct or combine plans from this information.
    pub fn invalidates_all_cached_style_artifacts(&self) -> bool {
        matches!(self.kind, StyleInvalidationPlanKind::FullDocument)
    }

    pub(crate) fn incremental_node_ids(&self) -> Option<&[Id]> {
        match &self.kind {
            StyleInvalidationPlanKind::DocumentSuffix { node_ids } => Some(node_ids),
            StyleInvalidationPlanKind::FullDocument => None,
        }
    }

    /// Produces the stable CSS-owned representation used by retained-render
    /// regression snapshots.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        match &self.kind {
            StyleInvalidationPlanKind::DocumentSuffix { node_ids } => {
                write!(&mut out, "scope: document-suffix node-ids: [")
                    .expect("write style invalidation snapshot");
                for (index, node_id) in node_ids.iter().enumerate() {
                    if index > 0 {
                        write!(&mut out, ", ").expect("write style invalidation snapshot");
                    }
                    write!(&mut out, "{}", node_id.0).expect("write style invalidation snapshot");
                }
                write!(&mut out, "]").expect("write style invalidation snapshot");
            }
            StyleInvalidationPlanKind::FullDocument => {
                write!(&mut out, "scope: full-document")
                    .expect("write style invalidation snapshot");
            }
        }
        out
    }
}

/// Classifies a Browser-owned change fact using the currently supported CSS
/// selector, cascade, and inheritance model.
///
/// This coarse entry point remains for initial/stylesheet lifecycle paths and
/// focused contract tests. DOM publication code that has an AF9 dependency
/// artifact must use [`classify_style_invalidation_with_dependencies`].
pub fn classify_style_invalidation(change: &StyleChangeFacts) -> Option<StyleInvalidationPlan> {
    match change {
        StyleChangeFacts::StylesheetSetChanged => Some(StyleInvalidationPlan::full_document()),
        StyleChangeFacts::DomPublication(facts) => {
            if facts.document_replaced
                || facts.tree_topology_or_order_operation
                || facts.text.occurred
                || facts.unclassified_patch_count > 0
            {
                // `:empty` depends on exact ordinary direct-text facts. Without
                // reverse selector-dependency indexing, CSS cannot prove which
                // elements are unaffected, so text is safely full-document.
                return Some(StyleInvalidationPlan::full_document());
            }
            if facts.attributes.occurred {
                return StyleInvalidationPlan::document_suffix(facts.attributes.node_ids.clone())
                    .or_else(|| Some(StyleInvalidationPlan::full_document()));
            }
            // Allocation alone and template-content association alone do not
            // alter selector-visible relationships in the published document.
            let _ = (
                facts.ordinary_nodes_allocated,
                facts.template_contents_associated,
            );
            None
        }
    }
}

/// Classifies exact neutral DOM transitions through the retained CSS-owned
/// dependency artifact. Browser supplies lifecycle context and transports the
/// opaque result; all selector and inline-cascade meaning remains here.
pub fn classify_style_invalidation_with_dependencies(
    input: StyleInvalidationInput<'_>,
) -> StyleInvalidationDecision {
    match input.change {
        StyleChangeFacts::StylesheetSetChanged => decision(
            Some(StyleInvalidationPlan::full_document()),
            StyleInvalidationReason::StylesheetSetChanged,
            DependencyHitSummary::default(),
            false,
        ),
        StyleChangeFacts::DomPublication(facts) => {
            if facts.document_replaced {
                return decision(
                    Some(StyleInvalidationPlan::full_document()),
                    StyleInvalidationReason::DocumentReplaced,
                    DependencyHitSummary::default(),
                    false,
                );
            }
            if facts.tree_topology_or_order_operation {
                return decision(
                    Some(StyleInvalidationPlan::full_document()),
                    StyleInvalidationReason::StructuralMutationRequiresFullRebuild,
                    DependencyHitSummary::default(),
                    false,
                );
            }
            if facts.unclassified_patch_count > 0 {
                return decision(
                    Some(StyleInvalidationPlan::full_document()),
                    StyleInvalidationReason::UnclassifiedMutation,
                    DependencyHitSummary::default(),
                    false,
                );
            }

            let Some(artifact) = input
                .dependency_artifact
                .filter(|artifact| artifact.matches_environment(input.matching_environment))
            else {
                if facts.attributes.occurred || facts.text.occurred {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::MissingOrIncompatibleDependencies,
                        DependencyHitSummary::default(),
                        false,
                    );
                }
                return decision(
                    None,
                    StyleInvalidationReason::NoStyleEffect,
                    DependencyHitSummary::default(),
                    false,
                );
            };

            if artifact.complete_index().is_none()
                && (facts.attributes.occurred || facts.text.occurred)
            {
                return decision(
                    Some(StyleInvalidationPlan::full_document()),
                    StyleInvalidationReason::MissingOrIncompatibleDependencies,
                    DependencyHitSummary::default(),
                    false,
                );
            }

            let mut dependency_budget = artifact.classification_budget();
            let mut node_ids = Vec::new();
            let mut hits = DependencyHitSummary::default();
            let mut inline_style_changed = false;
            let mut reason = StyleInvalidationReason::NoStyleEffect;

            if facts.attributes.occurred {
                if facts.attributes.historical_target_count > 0 {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::HistoricalMutationTarget,
                        hits,
                        false,
                    );
                }
                if facts.attributes.node_ids.is_empty() {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::ExactMutationDetailsUnavailable,
                        hits,
                        false,
                    );
                }
                let Some(mutations) = input.attribute_mutations else {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::ExactMutationDetailsUnavailable,
                        hits,
                        false,
                    );
                };
                if !mutation_ids_match(
                    &facts.attributes.node_ids,
                    mutations.iter().map(|mutation| mutation.node_id),
                ) {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::ExactMutationDetailsUnavailable,
                        hits,
                        false,
                    );
                }
                for mutation in mutations {
                    let before = mutation.before.unwrap_or(&[]);
                    if before == mutation.after {
                        continue;
                    }
                    let dependency_match = match artifact.classify_attribute_transition(
                        mutation.element_namespace,
                        before,
                        mutation.after,
                        &mut dependency_budget,
                    ) {
                        Ok(matched) => matched,
                        Err(failure) => {
                            return decision(
                                Some(StyleInvalidationPlan::full_document()),
                                dependency_failure_reason(&failure),
                                hits,
                                false,
                            );
                        }
                    };
                    let inline_changed = effective_unqualified_attribute_value(
                        mutation.element_namespace,
                        before,
                        "style",
                    ) != effective_unqualified_attribute_value(
                        mutation.element_namespace,
                        mutation.after,
                        "style",
                    );
                    if dependency_match.any || inline_changed {
                        if node_ids.try_reserve(1).is_err() {
                            return decision(
                                Some(StyleInvalidationPlan::full_document()),
                                StyleInvalidationReason::DependencyClassificationResourceUnavailable,
                                hits,
                                inline_style_changed,
                            );
                        }
                        node_ids.push(mutation.node_id);
                    }
                    hits.id |= dependency_match.id;
                    hits.class |= dependency_match.class;
                    hits.attribute |= dependency_match.attribute;
                    inline_style_changed |= inline_changed;
                }
                if hits.id || hits.class || hits.attribute {
                    reason = StyleInvalidationReason::SelectorDependencyMatched;
                } else if inline_style_changed {
                    reason = StyleInvalidationReason::InlineCascadeInputChanged;
                }
            }

            if facts.text.occurred {
                if facts.text.historical_target_count > 0 {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::HistoricalMutationTarget,
                        hits,
                        inline_style_changed,
                    );
                }
                if facts.text.node_ids.is_empty() {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::ExactMutationDetailsUnavailable,
                        hits,
                        inline_style_changed,
                    );
                }
                let Some(mutations) = input.text_mutations else {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::ExactMutationDetailsUnavailable,
                        hits,
                        inline_style_changed,
                    );
                };
                if !mutation_ids_match(
                    &facts.text.node_ids,
                    mutations.iter().map(|mutation| mutation.node_id),
                ) {
                    return decision(
                        Some(StyleInvalidationPlan::full_document()),
                        StyleInvalidationReason::ExactMutationDetailsUnavailable,
                        hits,
                        inline_style_changed,
                    );
                }
                for mutation in mutations {
                    let Some(before) = mutation.before else {
                        return decision(
                            Some(StyleInvalidationPlan::full_document()),
                            StyleInvalidationReason::ExactMutationDetailsUnavailable,
                            hits,
                            inline_style_changed,
                        );
                    };
                    if before == mutation.after
                        || crate::selectors::matching::text_is_document_whitespace(before)
                            == crate::selectors::matching::text_is_document_whitespace(
                                mutation.after,
                            )
                    {
                        continue;
                    }
                    let Some((parent_id, parent_namespace)) = mutation.parent_element else {
                        return decision(
                            Some(StyleInvalidationPlan::full_document()),
                            StyleInvalidationReason::ExactMutationDetailsUnavailable,
                            hits,
                            inline_style_changed,
                        );
                    };
                    let has_empty_dependency = match artifact.has_empty_dependency_for_namespace(
                        parent_namespace,
                        &mut dependency_budget,
                    ) {
                        Ok(has_dependency) => has_dependency,
                        Err(failure) => {
                            return decision(
                                Some(StyleInvalidationPlan::full_document()),
                                dependency_failure_reason(&failure),
                                hits,
                                inline_style_changed,
                            );
                        }
                    };
                    if has_empty_dependency {
                        if node_ids.try_reserve(1).is_err() {
                            return decision(
                                Some(StyleInvalidationPlan::full_document()),
                                StyleInvalidationReason::DependencyClassificationResourceUnavailable,
                                hits,
                                inline_style_changed,
                            );
                        }
                        node_ids.push(parent_id);
                        hits.empty = true;
                        reason = StyleInvalidationReason::EmptyDependencyMatched;
                    }
                }
            }

            decision(
                StyleInvalidationPlan::document_suffix(node_ids),
                reason,
                hits,
                inline_style_changed,
            )
        }
    }
}

fn decision(
    plan: Option<StyleInvalidationPlan>,
    reason: StyleInvalidationReason,
    dependency_hits: DependencyHitSummary,
    inline_style_changed: bool,
) -> StyleInvalidationDecision {
    StyleInvalidationDecision {
        plan,
        reason,
        dependency_hits,
        inline_style_changed,
    }
}

fn mutation_ids_match(expected: &[Id], actual: impl ExactSizeIterator<Item = Id>) -> bool {
    actual.len() == expected.len() && actual.zip(expected.iter().copied()).all(|(a, b)| a == b)
}

fn effective_unqualified_attribute_value<'a>(
    namespace: html::ElementNamespace,
    attributes: &'a [html::ParserCreatedAttribute],
    name: &str,
) -> Option<&'a str> {
    crate::dom_attributes::first_effective_unqualified_attribute(
        namespace,
        attributes.iter().map(|attribute| {
            crate::SelectorDomAttribute::new(
                attribute.namespace(),
                attribute.local_name(),
                attribute.value(),
            )
        }),
        name,
    )
    .map(crate::SelectorDomAttribute::value)
}

fn invalidation_reason_label(reason: StyleInvalidationReason) -> &'static str {
    match reason {
        StyleInvalidationReason::NoStyleEffect => "no-style-effect",
        StyleInvalidationReason::StylesheetSetChanged => "stylesheet-set-changed",
        StyleInvalidationReason::DocumentReplaced => "document-replaced",
        StyleInvalidationReason::StructuralMutationRequiresFullRebuild => {
            "structural-mutation-requires-full-rebuild"
        }
        StyleInvalidationReason::UnclassifiedMutation => "unclassified-mutation",
        StyleInvalidationReason::MissingOrIncompatibleDependencies => {
            "missing-or-incompatible-dependencies"
        }
        StyleInvalidationReason::DependencyEvaluationLimitExceeded => {
            "dependency-evaluation-limit-exceeded"
        }
        StyleInvalidationReason::DependencyClassificationResourceUnavailable => {
            "dependency-classification-resource-unavailable"
        }
        StyleInvalidationReason::ExactMutationDetailsUnavailable => {
            "exact-mutation-details-unavailable"
        }
        StyleInvalidationReason::HistoricalMutationTarget => "historical-mutation-target",
        StyleInvalidationReason::SelectorDependencyMatched => "selector-dependency-matched",
        StyleInvalidationReason::InlineCascadeInputChanged => "inline-cascade-input-changed",
        StyleInvalidationReason::EmptyDependencyMatched => "empty-dependency-matched",
    }
}

fn dependency_failure_reason(failure: &DependencyClassificationFailure) -> StyleInvalidationReason {
    match failure {
        DependencyClassificationFailure::EvaluationLimitExceeded { .. }
        | DependencyClassificationFailure::CounterExhausted { .. } => {
            StyleInvalidationReason::DependencyEvaluationLimitExceeded
        }
        DependencyClassificationFailure::ArtifactUnavailable => {
            StyleInvalidationReason::MissingOrIncompatibleDependencies
        }
    }
}

/// Combines pending CSS plans without exposing their semantic representation
/// to Browser/runtime. `None` is the sole representation of no invalidation.
pub fn merge_style_invalidation_plans(
    existing: Option<StyleInvalidationPlan>,
    incoming: Option<StyleInvalidationPlan>,
) -> Option<StyleInvalidationPlan> {
    match (existing, incoming) {
        (None, None) => None,
        (Some(plan), None) | (None, Some(plan)) => Some(plan),
        (Some(existing), Some(incoming)) => Some(merge_plans(existing, incoming)),
    }
}

fn merge_plans(
    existing: StyleInvalidationPlan,
    incoming: StyleInvalidationPlan,
) -> StyleInvalidationPlan {
    match (existing.kind, incoming.kind) {
        (StyleInvalidationPlanKind::FullDocument, _)
        | (_, StyleInvalidationPlanKind::FullDocument) => StyleInvalidationPlan::full_document(),
        (
            StyleInvalidationPlanKind::DocumentSuffix { mut node_ids },
            StyleInvalidationPlanKind::DocumentSuffix {
                node_ids: incoming_node_ids,
            },
        ) => {
            node_ids.extend(incoming_node_ids);
            StyleInvalidationPlan::document_suffix(node_ids)
                .expect("merged non-empty suffix invalidation")
        }
    }
}

fn canonicalize_node_ids(mut node_ids: Vec<Id>) -> Vec<Id> {
    node_ids.sort_by_key(|node_id| node_id.0);
    node_ids.dedup();
    node_ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dependency_artifact(
        source: &str,
        limits: &crate::StyleResolutionLimits,
    ) -> StyleDependencyArtifact {
        let sheet =
            crate::parse_stylesheet_with_options(source, &crate::ParseOptions::stylesheet());
        let input = crate::StylesheetCollectionInput::author(
            crate::StylesheetSourceId::in_memory_generation_index(1),
            crate::StylesheetOrder::new(0),
            &sheet,
            crate::StylesheetConditionInput::None,
        );
        let collection = crate::RuleCollection::try_new(&[input], limits).expect("collection");
        StyleDependencyArtifact::from_rule_collection(
            &collection,
            crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            limits,
        )
    }

    fn id(value: u32) -> Id {
        Id(value)
    }

    fn publication(builder: DomStyleChangeFactsBuilder) -> StyleChangeFacts {
        StyleChangeFacts::dom_publication(builder.build())
    }

    #[test]
    fn text_change_requires_full_document_style_invalidation() {
        let facts = publication(
            DomStyleChangeFacts::builder().text(ChangedStyleNodeFacts::changed(Vec::new())),
        );
        let plan = classify_style_invalidation(&facts).expect("text can change :empty matching");
        assert!(plan.invalidates_all_cached_style_artifacts());
        assert_eq!(plan.to_debug_snapshot(), "scope: full-document");
    }

    #[test]
    fn stylesheet_set_change_requires_full_document_style_invalidation() {
        let plan = classify_style_invalidation(&StyleChangeFacts::stylesheet_set_changed())
            .expect("stylesheet-set changes must authorize Style invalidation");

        assert!(plan.invalidates_all_cached_style_artifacts());
        assert_eq!(plan.to_debug_snapshot(), "scope: full-document");
    }

    #[test]
    fn attribute_suffix_ids_are_canonicalized() {
        let facts = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([
                id(4),
                id(2),
                id(4),
                id(1),
            ])),
        );
        let plan =
            classify_style_invalidation(&facts).expect("attribute change should invalidate style");

        assert_eq!(
            plan.to_debug_snapshot(),
            "scope: document-suffix node-ids: [1, 2, 4]"
        );
    }

    #[test]
    fn empty_attribute_identity_falls_back_to_full() {
        let facts = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed(Vec::new())),
        );
        let plan = classify_style_invalidation(&facts)
            .expect("unidentified attribute changes need a safe fallback");

        assert!(plan.invalidates_all_cached_style_artifacts());
    }

    #[test]
    fn pending_plans_merge_in_css() {
        let first_facts = publication(
            DomStyleChangeFacts::builder()
                .attributes(ChangedStyleNodeFacts::changed([id(3), id(1)])),
        );
        let second_facts = publication(
            DomStyleChangeFacts::builder()
                .attributes(ChangedStyleNodeFacts::changed([id(2), id(1)])),
        );
        let first = classify_style_invalidation(&first_facts);
        let second = classify_style_invalidation(&second_facts);
        let merged = merge_style_invalidation_plans(first, second).expect("merged plan");

        assert_eq!(
            merged.to_debug_snapshot(),
            "scope: document-suffix node-ids: [1, 2, 3]"
        );
    }

    #[test]
    fn full_plan_dominates_pending_suffix() {
        let suffix_facts = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([id(2)])),
        );
        let full_facts =
            publication(DomStyleChangeFacts::builder().tree_topology_or_order_operation());
        let suffix = classify_style_invalidation(&suffix_facts);
        let full = classify_style_invalidation(&full_facts);

        let merged = merge_style_invalidation_plans(suffix, full).expect("merged plan");
        assert!(merged.invalidates_all_cached_style_artifacts());
    }

    #[test]
    fn unavailable_active_dependency_metadata_forces_full_fallback() {
        let limits = crate::StyleResolutionLimits {
            max_selector_dependency_records_per_document: 0,
            ..crate::StyleResolutionLimits::default()
        };
        let artifact = dependency_artifact(".hot { color: red; }", &limits);
        let before = Vec::new();
        let after = vec![html::internal::unqualified_attribute("class", "hot")];
        let mutation = DomStyleAttributeMutation::new(
            id(7),
            html::ElementNamespace::Html,
            Some(&before),
            &after,
        );
        let change = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([id(7)])),
        );
        let decision = classify_style_invalidation_with_dependencies(
            StyleInvalidationInput::new(
                &change,
                Some(&artifact),
                crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            )
            .with_attribute_mutations(Some(&[mutation])),
        );
        assert!(
            decision
                .plan()
                .is_some_and(StyleInvalidationPlan::invalidates_all_cached_style_artifacts)
        );
        assert!(
            decision
                .to_debug_snapshot()
                .contains("reason: missing-or-incompatible-dependencies")
        );
    }

    #[test]
    fn dependency_classification_work_limit_forces_full_fallback() {
        let limits = crate::StyleResolutionLimits {
            max_selector_dependency_evaluations_per_publication: 0,
            ..crate::StyleResolutionLimits::default()
        };
        let artifact = dependency_artifact(".hot { color: red; }", &limits);
        assert!(artifact.to_debug_snapshot().contains("state: complete"));
        let before = Vec::new();
        let after = vec![html::internal::unqualified_attribute("class", "hot")];
        let mutation = DomStyleAttributeMutation::new(
            id(7),
            html::ElementNamespace::Html,
            Some(&before),
            &after,
        );
        let change = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([id(7)])),
        );
        let decision = classify_style_invalidation_with_dependencies(
            StyleInvalidationInput::new(
                &change,
                Some(&artifact),
                crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            )
            .with_attribute_mutations(Some(&[mutation])),
        );
        assert!(
            decision
                .plan()
                .is_some_and(StyleInvalidationPlan::invalidates_all_cached_style_artifacts)
        );
        assert!(
            decision
                .to_debug_snapshot()
                .contains("reason: dependency-evaluation-limit-exceeded")
        );
    }

    #[test]
    fn dependency_classification_resource_failure_has_a_distinct_debug_reason() {
        let decision = decision(
            Some(StyleInvalidationPlan::full_document()),
            StyleInvalidationReason::DependencyClassificationResourceUnavailable,
            DependencyHitSummary::default(),
            false,
        );

        assert_eq!(
            decision.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "af9-style-invalidation-decision\n",
                "reason: dependency-classification-resource-unavailable\n",
                "dependency-hits: id=false class=false attribute=false empty=false\n",
                "inline-style-changed: false\n",
                "selected-plan: scope: full-document\n",
            )
        );
    }

    #[test]
    fn inactive_unsupported_selector_does_not_poison_complete_dependencies() {
        let artifact = dependency_artifact(
            ".future:hover { color: blue; } .hot { color: red; }",
            &crate::StyleResolutionLimits::default(),
        );
        assert!(artifact.to_debug_snapshot().contains("state: complete"));
        assert!(
            artifact
                .to_debug_snapshot()
                .contains("inactive-unsupported=1")
        );
        let before = vec![html::internal::unqualified_attribute("title", "old")];
        let after = vec![html::internal::unqualified_attribute("title", "new")];
        let mutation = DomStyleAttributeMutation::new(
            id(7),
            html::ElementNamespace::Html,
            Some(&before),
            &after,
        );
        let change = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([id(7)])),
        );
        let decision = classify_style_invalidation_with_dependencies(
            StyleInvalidationInput::new(
                &change,
                Some(&artifact),
                crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            )
            .with_attribute_mutations(Some(&[mutation])),
        );
        assert!(decision.plan().is_none());
        assert_eq!(
            decision.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "af9-style-invalidation-decision\n",
                "reason: no-style-effect\n",
                "dependency-hits: id=false class=false attribute=false empty=false\n",
                "inline-style-changed: false\n",
                "selected-plan: none\n",
            )
        );
    }

    #[test]
    fn no_incoming_plan_does_not_clear_pending_plan() {
        let facts = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([id(2)])),
        );
        let suffix = classify_style_invalidation(&facts);
        let merged = merge_style_invalidation_plans(suffix.clone(), None);

        assert_eq!(merged, suffix);
    }

    #[test]
    fn text_full_plan_dominates_pending_suffix() {
        let suffix_facts = publication(
            DomStyleChangeFacts::builder().attributes(ChangedStyleNodeFacts::changed([id(2)])),
        );
        let text_facts = publication(
            DomStyleChangeFacts::builder().text(ChangedStyleNodeFacts::changed(Vec::new())),
        );
        let suffix = classify_style_invalidation(&suffix_facts);
        let text = classify_style_invalidation(&text_facts);

        let merged = merge_style_invalidation_plans(suffix, text).expect("merged plan");
        assert!(merged.invalidates_all_cached_style_artifacts());
    }

    #[test]
    fn mixed_attribute_and_text_facts_are_classified_as_one_publication() {
        let facts = publication(
            DomStyleChangeFacts::builder()
                .attributes(ChangedStyleNodeFacts::changed([id(9), id(9)]))
                .text(ChangedStyleNodeFacts::changed([id(4)])),
        );
        let plan = classify_style_invalidation(&facts).expect("text requires safe invalidation");
        assert_eq!(plan.to_debug_snapshot(), "scope: full-document");
    }

    #[test]
    fn allocation_and_template_association_alone_do_not_invent_style_work() {
        let facts = publication(
            DomStyleChangeFacts::builder()
                .ordinary_nodes_allocated()
                .template_contents_associated(),
        );
        assert_eq!(classify_style_invalidation(&facts), None);
    }

    #[test]
    fn unclassified_patch_falls_back_to_full_document() {
        let facts = publication(
            DomStyleChangeFacts::builder()
                .unclassified_patches(NonZeroUsize::new(2).expect("nonzero")),
        );
        let plan = classify_style_invalidation(&facts).expect("unknown patch needs safe fallback");
        assert!(plan.invalidates_all_cached_style_artifacts());
    }
}

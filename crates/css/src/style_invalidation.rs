//! CSS-owned style-input invalidation contracts.
//!
//! Browser/runtime reports what changed through [`StyleChangeFacts`]. CSS
//! classifies that fact into an opaque plan describing which retained style
//! results remain semantically reusable. The plan deliberately does not
//! describe the computation that will eventually execute; retained artifact
//! availability and identity validation remain runtime concerns.

use std::{fmt::Write, num::NonZeroUsize};

use html::internal::Id;

/// CSS-owned, invariant-safe facts for one changed-node mutation dimension.
///
/// `occurred` is deliberately distinct from `node_ids`: a valid mutation may
/// target only nodes that no longer survive in the published tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedStyleNodeFacts {
    occurred: bool,
    node_ids: Vec<Id>,
}

impl ChangedStyleNodeFacts {
    #[must_use]
    pub fn unchanged() -> Self {
        Self {
            occurred: false,
            node_ids: Vec::new(),
        }
    }

    #[must_use]
    pub fn changed(node_ids: impl IntoIterator<Item = Id>) -> Self {
        Self {
            occurred: true,
            node_ids: canonicalize_node_ids(node_ids.into_iter().collect()),
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
/// The current document-suffix proof is intentionally conservative. Its input
/// is only the mutation fact because the supported model has no reverse
/// selector dependencies. A future selector-dependency issue can extend this
/// CSS boundary without teaching Browser selector semantics.
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

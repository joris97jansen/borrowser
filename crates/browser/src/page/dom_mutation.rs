use std::{fmt::Write, num::NonZeroUsize};

use css::{ChangedStyleNodeFacts, DomStyleChangeFacts, StyleChangeFacts};
use html::{DomPatch, PatchKey, internal::Id};
#[cfg(test)]
use html::{ElementNamespace, ParserCreatedAttribute};

use crate::dom_store::{
    DomMutationPrecisionFailure, ExactDomMutationDetails, ExactStoreAttributeMutation,
    ExactStoreTextMutation, ResolvedMutationNodeIds,
};

/// Patch-layer facts collected before mutation targets are resolved against
/// the staged post-publication DOM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingDomMutationFacts {
    document_replaced: bool,
    ordinary_nodes_allocated: bool,
    tree_topology_or_order_operation: bool,
    template_contents_associated: bool,
    attribute_target_keys: Vec<PatchKey>,
    text_target_keys: Vec<PatchKey>,
    unclassified_patch_count: usize,
}

impl PendingDomMutationFacts {
    pub(crate) fn from_patches(patches: &[DomPatch], handle_changed: bool) -> Self {
        let mut facts = Self {
            document_replaced: handle_changed,
            ordinary_nodes_allocated: false,
            tree_topology_or_order_operation: false,
            template_contents_associated: false,
            attribute_target_keys: Vec::new(),
            text_target_keys: Vec::new(),
            unclassified_patch_count: 0,
        };
        for patch in patches {
            match patch {
                DomPatch::Clear | DomPatch::CreateDocument { .. } => {
                    facts.document_replaced = true;
                }
                DomPatch::CreateDocumentType { .. }
                | DomPatch::CreateElement { .. }
                | DomPatch::CreateText { .. }
                | DomPatch::CreateComment { .. }
                | DomPatch::CreateProcessingInstruction { .. } => {
                    facts.ordinary_nodes_allocated = true;
                }
                DomPatch::CreateTemplateContents { .. } => {
                    facts.template_contents_associated = true;
                }
                DomPatch::AppendChild { .. }
                | DomPatch::InsertBefore { .. }
                | DomPatch::RemoveNode { .. } => {
                    facts.tree_topology_or_order_operation = true;
                }
                DomPatch::SetAttributes { key, .. } => facts.attribute_target_keys.push(*key),
                DomPatch::SetText { key, .. } | DomPatch::AppendText { key, .. } => {
                    facts.text_target_keys.push(*key);
                }
                // `DomPatch` is non-exhaustive. A future operation is not
                // structurally classified merely because this Browser build
                // does not know its meaning.
                _ => {
                    facts.unclassified_patch_count = facts
                        .unclassified_patch_count
                        .checked_add(1)
                        .expect("a patch slice cannot contain more than usize::MAX entries");
                }
            }
        }
        facts
    }

    pub(crate) fn attribute_target_keys(&self) -> &[PatchKey] {
        &self.attribute_target_keys
    }

    pub(crate) fn text_target_keys(&self) -> &[PatchKey] {
        &self.text_target_keys
    }

    pub(crate) fn document_replaced(&self) -> bool {
        self.document_replaced
    }

    #[cfg(test)]
    fn ordinary_nodes_allocated(&self) -> bool {
        self.ordinary_nodes_allocated
    }

    #[cfg(test)]
    fn tree_topology_or_order_operation(&self) -> bool {
        self.tree_topology_or_order_operation
    }

    #[cfg(test)]
    fn template_contents_associated(&self) -> bool {
        self.template_contents_associated
    }

    #[cfg(test)]
    fn unclassified_patch_count(&self) -> usize {
        self.unclassified_patch_count
    }
}

/// Neutral identity facts for one mutation dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangedDomNodeFacts {
    changed: bool,
    live_node_ids: Vec<Id>,
    historical_target_count: usize,
}

impl ChangedDomNodeFacts {
    fn from_resolution(target_keys: &[PatchKey], resolution: ResolvedMutationNodeIds) -> Self {
        let changed = !target_keys.is_empty();
        let (live_node_ids, historical_target_count) = resolution.into_parts();
        debug_assert!(changed || (live_node_ids.is_empty() && historical_target_count == 0));
        Self {
            changed,
            live_node_ids,
            historical_target_count,
        }
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    #[cfg(test)]
    pub(crate) fn live_node_ids(&self) -> &[Id] {
        &self.live_node_ids
    }

    #[cfg(test)]
    pub(crate) fn historical_target_count(&self) -> usize {
        self.historical_target_count
    }

    #[cfg(test)]
    pub(crate) fn changed_for_tests(node_ids: Vec<Id>) -> Self {
        let mut live_node_ids = node_ids;
        live_node_ids.sort_by_key(|node_id| node_id.0);
        live_node_ids.dedup();
        Self {
            changed: true,
            live_node_ids,
            historical_target_count: 0,
        }
    }
}

/// Complete Browser/DOM facts for exactly one committed publication.
///
/// Topology remains deliberately coarse in AF4e. Attribute and text targets
/// retain surviving neutral DOM identities plus a count of valid historical
/// targets. AF9 layers exact committed-before/final-after details captured by
/// `DomStore` on top without weakening this publication-level truth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DomMutationFacts {
    document_replaced: bool,
    ordinary_nodes_allocated: bool,
    tree_topology_or_order_operation: bool,
    template_contents_associated: bool,
    attributes: ChangedDomNodeFacts,
    text: ChangedDomNodeFacts,
    exact_attributes: ExactDomMutationDetails<ExactStoreAttributeMutation>,
    exact_text: ExactDomMutationDetails<ExactStoreTextMutation>,
    unclassified_patch_count: usize,
}

impl DomMutationFacts {
    pub(crate) fn resolve(
        pending: PendingDomMutationFacts,
        attributes: ResolvedMutationNodeIds,
        text: ResolvedMutationNodeIds,
        exact_attributes: ExactDomMutationDetails<ExactStoreAttributeMutation>,
        exact_text: ExactDomMutationDetails<ExactStoreTextMutation>,
    ) -> Self {
        let attributes =
            ChangedDomNodeFacts::from_resolution(&pending.attribute_target_keys, attributes);
        let text = ChangedDomNodeFacts::from_resolution(&pending.text_target_keys, text);
        Self {
            document_replaced: pending.document_replaced,
            ordinary_nodes_allocated: pending.ordinary_nodes_allocated,
            tree_topology_or_order_operation: pending.tree_topology_or_order_operation,
            template_contents_associated: pending.template_contents_associated,
            attributes,
            text,
            exact_attributes,
            exact_text,
            unclassified_patch_count: pending.unclassified_patch_count,
        }
    }

    pub(crate) fn document_replaced(&self) -> bool {
        self.document_replaced
    }

    pub(crate) fn tree_topology_or_order_operation(&self) -> bool {
        self.tree_topology_or_order_operation
    }

    pub(crate) fn attributes(&self) -> &ChangedDomNodeFacts {
        &self.attributes
    }

    pub(crate) fn text(&self) -> &ChangedDomNodeFacts {
        &self.text
    }

    pub(crate) fn unclassified_patch_count(&self) -> usize {
        self.unclassified_patch_count
    }

    pub(crate) fn to_css_style_change_facts(&self) -> StyleChangeFacts {
        let mut builder = DomStyleChangeFacts::builder()
            .attributes(if self.attributes.changed {
                ChangedStyleNodeFacts::changed_with_historical_targets(
                    self.attributes.live_node_ids.iter().copied(),
                    self.attributes.historical_target_count,
                )
            } else {
                ChangedStyleNodeFacts::unchanged()
            })
            .text(if self.text.changed {
                ChangedStyleNodeFacts::changed_with_historical_targets(
                    self.text.live_node_ids.iter().copied(),
                    self.text.historical_target_count,
                )
            } else {
                ChangedStyleNodeFacts::unchanged()
            });
        if self.document_replaced {
            builder = builder.document_replaced();
        }
        if self.ordinary_nodes_allocated {
            builder = builder.ordinary_nodes_allocated();
        }
        if self.tree_topology_or_order_operation {
            builder = builder.tree_topology_or_order_operation();
        }
        if self.template_contents_associated {
            builder = builder.template_contents_associated();
        }
        if let Some(count) = NonZeroUsize::new(self.unclassified_patch_count) {
            builder = builder.unclassified_patches(count);
        }
        StyleChangeFacts::dom_publication(builder.build())
    }

    pub(crate) fn css_attribute_mutation_views(
        &self,
    ) -> Option<Vec<css::DomStyleAttributeMutation<'_>>> {
        let ExactDomMutationDetails::Complete(mutations) = &self.exact_attributes else {
            return None;
        };
        let mut views = Vec::new();
        views.try_reserve_exact(mutations.len()).ok()?;
        views.extend(mutations.iter().map(|mutation| {
            css::DomStyleAttributeMutation::new(
                mutation.node_id,
                mutation.element_namespace,
                mutation.before.as_deref(),
                &mutation.after,
            )
        }));
        Some(views)
    }

    pub(crate) fn css_text_mutation_views(&self) -> Option<Vec<css::DomStyleTextMutation<'_>>> {
        let ExactDomMutationDetails::Complete(mutations) = &self.exact_text else {
            return None;
        };
        let mut views = Vec::new();
        views.try_reserve_exact(mutations.len()).ok()?;
        views.extend(mutations.iter().map(|mutation| {
            css::DomStyleTextMutation::new(
                mutation.node_id,
                mutation.parent_element,
                mutation.before.as_deref(),
                &mutation.after,
            )
        }));
        Some(views)
    }

    pub(crate) fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 2").expect("write mutation facts");
        writeln!(&mut out, "dom-mutation-facts").expect("write mutation facts");
        writeln!(&mut out, "document-replaced: {}", self.document_replaced)
            .expect("write mutation facts");
        writeln!(
            &mut out,
            "ordinary-nodes-allocated: {}",
            self.ordinary_nodes_allocated
        )
        .expect("write mutation facts");
        writeln!(
            &mut out,
            "tree-topology-or-order-operation: {}",
            self.tree_topology_or_order_operation
        )
        .expect("write mutation facts");
        writeln!(
            &mut out,
            "template-contents-associated: {}",
            self.template_contents_associated
        )
        .expect("write mutation facts");
        append_changed_nodes(&mut out, "attributes", &self.attributes);
        append_exact_attribute_details(&mut out, &self.exact_attributes);
        append_changed_nodes(&mut out, "text", &self.text);
        append_exact_text_details(&mut out, &self.exact_text);
        writeln!(
            &mut out,
            "unclassified-patch-count: {}",
            self.unclassified_patch_count
        )
        .expect("write mutation facts");
        out
    }

    #[cfg(test)]
    pub(crate) fn document_replaced_for_tests() -> Self {
        Self {
            document_replaced: true,
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    pub(crate) fn attributes_changed_for_tests(node_ids: Vec<Id>) -> Self {
        Self {
            attributes: ChangedDomNodeFacts::changed_for_tests(node_ids),
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    pub(crate) fn attribute_transition_for_tests(
        node_id: Id,
        element_namespace: ElementNamespace,
        before: Vec<ParserCreatedAttribute>,
        after: Vec<ParserCreatedAttribute>,
    ) -> Self {
        Self {
            attributes: ChangedDomNodeFacts::changed_for_tests(vec![node_id]),
            exact_attributes: ExactDomMutationDetails::Complete(vec![
                ExactStoreAttributeMutation {
                    node_id,
                    element_namespace,
                    before: Some(before),
                    after,
                },
            ]),
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    pub(crate) fn text_changed_for_tests(node_ids: Vec<Id>) -> Self {
        Self {
            text: ChangedDomNodeFacts::changed_for_tests(node_ids),
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    pub(crate) fn attributes_and_text_changed_for_tests(
        attribute_node_ids: Vec<Id>,
        text_node_ids: Vec<Id>,
    ) -> Self {
        Self {
            attributes: ChangedDomNodeFacts::changed_for_tests(attribute_node_ids),
            text: ChangedDomNodeFacts::changed_for_tests(text_node_ids),
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    pub(crate) fn tree_changed_for_tests() -> Self {
        Self {
            tree_topology_or_order_operation: true,
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    pub(crate) fn unclassified_for_tests(count: usize) -> Self {
        Self {
            unclassified_patch_count: count,
            ..Self::unchanged_for_tests()
        }
    }

    #[cfg(test)]
    fn unchanged_for_tests() -> Self {
        Self {
            document_replaced: false,
            ordinary_nodes_allocated: false,
            tree_topology_or_order_operation: false,
            template_contents_associated: false,
            attributes: ChangedDomNodeFacts {
                changed: false,
                live_node_ids: Vec::new(),
                historical_target_count: 0,
            },
            text: ChangedDomNodeFacts {
                changed: false,
                live_node_ids: Vec::new(),
                historical_target_count: 0,
            },
            unclassified_patch_count: 0,
            exact_attributes: ExactDomMutationDetails::Complete(Vec::new()),
            exact_text: ExactDomMutationDetails::Complete(Vec::new()),
        }
    }
}

fn append_exact_attribute_details(
    out: &mut String,
    details: &ExactDomMutationDetails<ExactStoreAttributeMutation>,
) {
    match details {
        ExactDomMutationDetails::Complete(mutations) => {
            writeln!(out, "attributes-exact: complete count={}", mutations.len())
                .expect("write mutation facts");
            for mutation in mutations {
                writeln!(
                    out,
                    "  node={} namespace={} before={} after={} no-op={}",
                    mutation.node_id.0,
                    mutation.element_namespace.snapshot_name(),
                    mutation.before.as_ref().map_or(0, Vec::len),
                    mutation.after.len(),
                    mutation.before.as_deref() == Some(mutation.after.as_slice()),
                )
                .expect("write mutation facts");
            }
        }
        ExactDomMutationDetails::ConservativeUnavailable(failure) => {
            write!(
                out,
                "attributes-exact: conservative-unavailable kind={}",
                failure.stable_label()
            )
            .expect("write mutation facts");
            append_precision_failure(out, failure);
        }
    }
}

fn append_exact_text_details(
    out: &mut String,
    details: &ExactDomMutationDetails<ExactStoreTextMutation>,
) {
    match details {
        ExactDomMutationDetails::Complete(mutations) => {
            writeln!(out, "text-exact: complete count={}", mutations.len())
                .expect("write mutation facts");
            for mutation in mutations {
                writeln!(
                    out,
                    "  node={} parent={} before-bytes={} after-bytes={} no-op={}",
                    mutation.node_id.0,
                    mutation
                        .parent_element
                        .map_or_else(|| "none".to_string(), |(id, _)| id.0.to_string()),
                    mutation.before.as_ref().map_or(0, String::len),
                    mutation.after.len(),
                    mutation.before.as_deref() == Some(mutation.after.as_str()),
                )
                .expect("write mutation facts");
            }
        }
        ExactDomMutationDetails::ConservativeUnavailable(failure) => {
            write!(
                out,
                "text-exact: conservative-unavailable kind={}",
                failure.stable_label()
            )
            .expect("write mutation facts");
            append_precision_failure(out, failure);
        }
    }
}

fn append_precision_failure(out: &mut String, failure: &DomMutationPrecisionFailure) {
    match failure {
        DomMutationPrecisionFailure::DocumentIdentityChanged => {
            writeln!(out).expect("write mutation facts");
        }
        DomMutationPrecisionFailure::LimitExceeded {
            limit,
            configured,
            observed,
        } => {
            writeln!(
                out,
                " limit={} configured={} observed={}",
                limit.stable_label(),
                configured,
                observed
            )
            .expect("write mutation facts");
        }
        DomMutationPrecisionFailure::CounterExhausted { counter } => {
            writeln!(out, " counter={counter}").expect("write mutation facts");
        }
        DomMutationPrecisionFailure::Reservation { storage } => {
            writeln!(out, " storage={}", storage.stable_label()).expect("write mutation facts");
        }
    }
}

fn append_changed_nodes(out: &mut String, label: &str, facts: &ChangedDomNodeFacts) {
    write!(out, "{label}: changed={} live-node-ids=[", facts.changed)
        .expect("write mutation facts");
    for (index, node_id) in facts.live_node_ids.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write!(out, "{}", node_id.0).expect("write mutation facts");
    }
    writeln!(
        out,
        "] historical-target-count={}",
        facts.historical_target_count
    )
    .expect("write mutation facts");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_batch_preserves_all_simultaneous_dimensions() {
        let patches = vec![
            DomPatch::SetAttributes {
                key: PatchKey(4),
                attributes: Vec::new(),
            },
            DomPatch::SetText {
                key: PatchKey(8),
                text: "changed".into(),
            },
            DomPatch::AppendText {
                key: PatchKey(8),
                text: " again".into(),
            },
            DomPatch::RemoveNode { key: PatchKey(9) },
        ];
        let facts = PendingDomMutationFacts::from_patches(&patches, false);
        assert!(!facts.document_replaced());
        assert!(facts.tree_topology_or_order_operation());
        assert_eq!(facts.attribute_target_keys(), [PatchKey(4)]);
        assert_eq!(facts.text_target_keys(), [PatchKey(8), PatchKey(8)]);
        assert_eq!(facts.unclassified_patch_count(), 0);
    }

    #[test]
    fn allocation_is_not_topology_and_template_association_is_separate() {
        let patches = vec![
            DomPatch::CreateText {
                key: PatchKey(2),
                text: String::new(),
            },
            DomPatch::CreateTemplateContents {
                host: PatchKey(3),
                contents: PatchKey(4),
            },
        ];
        let facts = PendingDomMutationFacts::from_patches(&patches, false);
        assert!(facts.ordinary_nodes_allocated());
        assert!(facts.template_contents_associated());
        assert!(!facts.tree_topology_or_order_operation());
    }

    #[test]
    fn every_known_patch_variant_maps_to_its_exact_neutral_dimension() {
        let reset = PendingDomMutationFacts::from_patches(
            &[
                DomPatch::Clear,
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
            ],
            false,
        );
        assert!(reset.document_replaced());
        assert!(!reset.ordinary_nodes_allocated());

        let allocations = PendingDomMutationFacts::from_patches(
            &[
                DomPatch::CreateDocumentType {
                    key: PatchKey(2),
                    name: Some("html".into()),
                    public_id: None,
                    system_id: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(3),
                    name: html::internal::html_name("p"),
                    attributes: Vec::new(),
                },
                DomPatch::CreateText {
                    key: PatchKey(4),
                    text: String::new(),
                },
                DomPatch::CreateComment {
                    key: PatchKey(5),
                    text: String::new(),
                },
                DomPatch::CreateProcessingInstruction {
                    key: PatchKey(6),
                    target: "pi".into(),
                    data: String::new(),
                },
            ],
            false,
        );
        assert!(allocations.ordinary_nodes_allocated());
        assert!(!allocations.tree_topology_or_order_operation());

        let topology = PendingDomMutationFacts::from_patches(
            &[
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::InsertBefore {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                    before: PatchKey(3),
                },
                DomPatch::RemoveNode { key: PatchKey(2) },
            ],
            false,
        );
        assert!(topology.tree_topology_or_order_operation());
        assert!(!topology.ordinary_nodes_allocated());

        let mutation_targets = PendingDomMutationFacts::from_patches(
            &[
                DomPatch::SetAttributes {
                    key: PatchKey(3),
                    attributes: Vec::new(),
                },
                DomPatch::SetText {
                    key: PatchKey(4),
                    text: "a".into(),
                },
                DomPatch::AppendText {
                    key: PatchKey(4),
                    text: "b".into(),
                },
            ],
            false,
        );
        assert_eq!(mutation_targets.attribute_target_keys(), [PatchKey(3)]);
        assert_eq!(
            mutation_targets.text_target_keys(),
            [PatchKey(4), PatchKey(4)]
        );
        assert!(!mutation_targets.tree_topology_or_order_operation());
        assert_eq!(mutation_targets.unclassified_patch_count(), 0);
    }

    #[test]
    #[should_panic(
        expected = "a committed DOM publication must establish document mode before invalidation"
    )]
    fn publication_invalidation_never_synthesizes_a_matching_environment() {
        let mut page = crate::page::PageState::new();
        let _ = page.mark_dom_changed(DomMutationFacts::text_changed_for_tests(Vec::new()));
    }
}

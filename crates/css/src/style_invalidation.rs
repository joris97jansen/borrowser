//! CSS-owned style-input invalidation contracts.
//!
//! Browser/runtime reports what changed through [`StyleChangeFacts`]. CSS
//! classifies that fact into an opaque plan describing which retained style
//! results remain semantically reusable. The plan deliberately does not
//! describe the computation that will eventually execute; retained artifact
//! availability and identity validation remain runtime concerns.

use std::fmt::Write;

use html::internal::Id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleChangeFacts {
    DocumentReplaced,
    TreeStructureChanged,
    AttributesChanged { node_ids: Vec<Id> },
    TextChanged,
    StylesheetSetChanged,
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
pub fn classify_style_invalidation(change: StyleChangeFacts) -> Option<StyleInvalidationPlan> {
    match change {
        StyleChangeFacts::DocumentReplaced
        | StyleChangeFacts::TreeStructureChanged
        | StyleChangeFacts::StylesheetSetChanged => Some(StyleInvalidationPlan::full_document()),
        StyleChangeFacts::AttributesChanged { node_ids } if node_ids.is_empty() => {
            Some(StyleInvalidationPlan::full_document())
        }
        StyleChangeFacts::AttributesChanged { node_ids } => {
            StyleInvalidationPlan::document_suffix(node_ids)
        }
        StyleChangeFacts::TextChanged => None,
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

    #[test]
    fn no_style_change_is_none() {
        assert_eq!(
            classify_style_invalidation(StyleChangeFacts::TextChanged),
            None
        );
    }

    #[test]
    fn attribute_suffix_ids_are_canonicalized() {
        let plan = classify_style_invalidation(StyleChangeFacts::AttributesChanged {
            node_ids: vec![id(4), id(2), id(4), id(1)],
        })
        .expect("attribute change should invalidate style");

        assert_eq!(
            plan.to_debug_snapshot(),
            "scope: document-suffix node-ids: [1, 2, 4]"
        );
    }

    #[test]
    fn empty_attribute_identity_falls_back_to_full() {
        let plan = classify_style_invalidation(StyleChangeFacts::AttributesChanged {
            node_ids: Vec::new(),
        })
        .expect("unidentified attribute changes need a safe fallback");

        assert!(plan.invalidates_all_cached_style_artifacts());
    }

    #[test]
    fn pending_plans_merge_in_css() {
        let first = classify_style_invalidation(StyleChangeFacts::AttributesChanged {
            node_ids: vec![id(3), id(1)],
        });
        let second = classify_style_invalidation(StyleChangeFacts::AttributesChanged {
            node_ids: vec![id(2), id(1)],
        });
        let merged = merge_style_invalidation_plans(first, second).expect("merged plan");

        assert_eq!(
            merged.to_debug_snapshot(),
            "scope: document-suffix node-ids: [1, 2, 3]"
        );
    }

    #[test]
    fn full_plan_dominates_pending_suffix() {
        let suffix = classify_style_invalidation(StyleChangeFacts::AttributesChanged {
            node_ids: vec![id(2)],
        });
        let full = classify_style_invalidation(StyleChangeFacts::TreeStructureChanged);

        let merged = merge_style_invalidation_plans(suffix, full).expect("merged plan");
        assert!(merged.invalidates_all_cached_style_artifacts());
    }

    #[test]
    fn no_op_does_not_clear_pending_plan() {
        let suffix = classify_style_invalidation(StyleChangeFacts::AttributesChanged {
            node_ids: vec![id(2)],
        });
        let merged = merge_style_invalidation_plans(
            suffix.clone(),
            classify_style_invalidation(StyleChangeFacts::TextChanged),
        );

        assert_eq!(merged, suffix);
    }
}

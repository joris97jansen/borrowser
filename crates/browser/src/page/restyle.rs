use crate::rendering::RenderInvalidationEntryPoint;
use html::{DomPatch, internal::Id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestyleTrigger {
    DocumentReplaced,
    TreeMutated,
    AttributesChanged,
    TextMutated,
}

impl RestyleTrigger {
    pub(crate) fn from_patches(patches: &[DomPatch]) -> Option<Self> {
        let mut trigger = None;
        for patch in patches {
            let candidate = match patch {
                DomPatch::Clear | DomPatch::CreateDocument { .. } => Self::DocumentReplaced,
                DomPatch::SetAttributes { .. } => Self::AttributesChanged,
                DomPatch::SetText { .. } | DomPatch::AppendText { .. } => Self::TextMutated,
                DomPatch::CreateElement { .. }
                | DomPatch::CreateText { .. }
                | DomPatch::CreateComment { .. }
                | DomPatch::AppendChild { .. }
                | DomPatch::InsertBefore { .. }
                | DomPatch::RemoveNode { .. } => Self::TreeMutated,
                _ => Self::TreeMutated,
            };
            trigger = Some(match (trigger, candidate) {
                (Some(Self::DocumentReplaced), _) | (_, Self::DocumentReplaced) => {
                    Self::DocumentReplaced
                }
                (Some(Self::TreeMutated), _) | (_, Self::TreeMutated) => Self::TreeMutated,
                (Some(Self::AttributesChanged), _) | (_, Self::AttributesChanged) => {
                    Self::AttributesChanged
                }
                _ => Self::TextMutated,
            });
        }
        trigger
    }

    pub(super) fn render_invalidation_entry_point(self) -> RenderInvalidationEntryPoint {
        match self {
            Self::DocumentReplaced => RenderInvalidationEntryPoint::DocumentReplaced,
            Self::TreeMutated => RenderInvalidationEntryPoint::DomStructureChanged,
            Self::AttributesChanged => RenderInvalidationEntryPoint::DomAttributesChanged,
            Self::TextMutated => RenderInvalidationEntryPoint::DomTextChanged,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestyleHint {
    pub(super) trigger: RestyleTrigger,
    pub(super) attribute_dirty_nodes: Vec<Id>,
}

impl RestyleHint {
    pub(crate) fn document_replaced() -> Self {
        Self {
            trigger: RestyleTrigger::DocumentReplaced,
            attribute_dirty_nodes: Vec::new(),
        }
    }

    pub(crate) fn from_dom_patch_batch(
        patches: &[DomPatch],
        attribute_dirty_nodes: Vec<Id>,
    ) -> Option<Self> {
        let trigger = RestyleTrigger::from_patches(patches)?;

        Some(Self {
            trigger,
            attribute_dirty_nodes,
        })
    }

    #[cfg(test)]
    pub(crate) fn attributes_changed(attribute_dirty_nodes: Vec<Id>) -> Self {
        Self {
            trigger: RestyleTrigger::AttributesChanged,
            attribute_dirty_nodes,
        }
    }

    #[cfg(test)]
    pub(crate) fn text_mutated() -> Self {
        Self {
            trigger: RestyleTrigger::TextMutated,
            attribute_dirty_nodes: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn tree_mutated() -> Self {
        Self {
            trigger: RestyleTrigger::TreeMutated,
            attribute_dirty_nodes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StyleInvalidationScope {
    /// Conservative full restyle. Used for document replacement, structural
    /// mutations, stylesheet changes, and any partial-reuse proof failure.
    Full,
    /// Minimal U4 partial strategy: attribute changes preserve selector element
    /// order, so reuse the computed prefix before the earliest changed element
    /// and recompute that element plus the document-order suffix. The suffix is
    /// deliberately conservative because sibling selectors can affect following
    /// siblings and inheritance can affect descendants.
    ///
    /// This proof assumes the supported selector model has no selector that lets
    /// later or descendant elements affect an earlier ancestor or sibling, such
    /// as `:has()`. Adding that kind of selector must either widen this scope to
    /// `Full` or add selector-aware invalidation dependencies.
    AttributeSuffix { node_ids: Vec<Id> },
}

impl StyleInvalidationScope {
    pub(super) fn merge(self, next: Self) -> Self {
        match (self, next) {
            (Self::Full, _) | (_, Self::Full) => Self::Full,
            (
                Self::AttributeSuffix { mut node_ids },
                Self::AttributeSuffix {
                    node_ids: next_node_ids,
                },
            ) => {
                node_ids.extend(next_node_ids);
                node_ids.sort_by_key(|id| id.0);
                node_ids.dedup();
                Self::AttributeSuffix { node_ids }
            }
        }
    }
}

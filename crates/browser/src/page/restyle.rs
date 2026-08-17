use crate::rendering::CssStyleInvalidationSource;
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
                // Template contents are inert to selector/layout/paint matching;
                // the DOM generation still commits, but no style invalidation is
                // required when this is the only publication effect.
                DomPatch::CreateTemplateContents { .. } => continue,
                DomPatch::SetAttributes { .. } => Self::AttributesChanged,
                DomPatch::SetText { .. } | DomPatch::AppendText { .. } => Self::TextMutated,
                DomPatch::CreateElement { .. }
                | DomPatch::CreateText { .. }
                | DomPatch::CreateComment { .. }
                | DomPatch::CreateProcessingInstruction { .. }
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

    pub(super) fn css_style_invalidation_source(self) -> CssStyleInvalidationSource {
        match self {
            Self::DocumentReplaced => CssStyleInvalidationSource::DocumentReplaced,
            Self::TreeMutated => CssStyleInvalidationSource::DomStructureChanged,
            Self::AttributesChanged => CssStyleInvalidationSource::DomAttributesChanged,
            Self::TextMutated => CssStyleInvalidationSource::DomTextChanged,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestyleHint {
    pub(super) trigger: RestyleTrigger,
    pub(super) attribute_dirty_nodes: Vec<Id>,
}

impl RestyleHint {
    #[cfg(test)]
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

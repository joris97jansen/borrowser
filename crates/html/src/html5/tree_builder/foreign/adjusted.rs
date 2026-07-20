use crate::dom_patch::PatchKey;
use crate::{ExpandedElementName, ParserCreatedAttribute};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::html5::tree_builder) enum AdjustedCurrentNodeSource {
    StackCurrent,
    FragmentContext,
}

/// Semantic adjusted-current-node view. A future fragment context does not
/// need to be an open-stack entry or have a patch identity.
#[derive(Clone, Copy, Debug)]
pub(in crate::html5::tree_builder) struct AdjustedCurrentNode<'a> {
    pub(in crate::html5::tree_builder) key: Option<PatchKey>,
    pub(in crate::html5::tree_builder) expanded_name: &'a ExpandedElementName,
    pub(in crate::html5::tree_builder) attributes: &'a [ParserCreatedAttribute],
    pub(in crate::html5::tree_builder) source: AdjustedCurrentNodeSource,
}

impl<'a> AdjustedCurrentNode<'a> {
    pub(in crate::html5::tree_builder) fn from_stack_current(
        key: PatchKey,
        expanded_name: &'a ExpandedElementName,
        attributes: &'a [ParserCreatedAttribute],
    ) -> Self {
        Self {
            key: Some(key),
            expanded_name,
            attributes,
            source: AdjustedCurrentNodeSource::StackCurrent,
        }
    }

    #[allow(
        dead_code,
        reason = "fragment parsing is deliberately deferred after AE11"
    )]
    pub(in crate::html5::tree_builder) fn from_fragment_context(
        expanded_name: &'a ExpandedElementName,
        attributes: &'a [ParserCreatedAttribute],
    ) -> Self {
        Self {
            key: None,
            expanded_name,
            attributes,
            source: AdjustedCurrentNodeSource::FragmentContext,
        }
    }
}

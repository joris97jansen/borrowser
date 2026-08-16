use super::limits::{StyleResolutionError, StyleResolutionLimit};
use crate::selectors::{BoundedSelectorDomConstructionError, SelectorDomIndex};
use html::{ElementNode, Node};

pub(super) fn build_document_selector_dom_with_element_limit<'dom>(
    root: &'dom Node,
    maximum_elements: usize,
) -> Result<SelectorDomIndex<'dom>, StyleResolutionError> {
    SelectorDomIndex::try_from_document_with_element_limit(root, maximum_elements)
        .map_err(map_bounded_construction_error)
}

pub(super) fn preflight_document_selector_dom_with_element_limit(
    root: &Node,
    maximum_elements: usize,
) -> Result<(), StyleResolutionError> {
    SelectorDomIndex::preflight_document_with_element_limit(root, maximum_elements)
        .map_err(map_bounded_construction_error)
}

pub(super) fn build_element_subtree_selector_dom_with_element_limit<'dom>(
    root: &'dom ElementNode,
    maximum_elements: usize,
) -> Result<SelectorDomIndex<'dom>, StyleResolutionError> {
    SelectorDomIndex::try_from_element_subtree_with_element_limit(root, maximum_elements)
        .map_err(map_bounded_construction_error)
}

fn map_bounded_construction_error(
    error: BoundedSelectorDomConstructionError,
) -> StyleResolutionError {
    match error {
        BoundedSelectorDomConstructionError::Build(error) => {
            StyleResolutionError::SelectorDomBuild(error)
        }
        BoundedSelectorDomConstructionError::ElementLimitExceeded { limit, .. } => {
            StyleResolutionError::LimitExceeded {
                limit: StyleResolutionLimit::StyledElementsPerDocument,
                configured: limit,
            }
        }
    }
}

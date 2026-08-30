use std::marker::PhantomData;

use html::{ElementNamespace, ElementNode, Node, internal::Id};

use super::{
    BoundedSelectorDomConstructionError, SelectorDomBuildError, SelectorDomElementId,
    SelectorDomIndex, SelectorListMatchOutcome, SelectorMatchingContext,
    SelectorMatchingEnvironment, SelectorMatchingLimitError, SelectorMatchingLimits,
};
use crate::selectors::SelectorListParseResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleProjectionBuildError {
    SelectorDom(SelectorDomBuildError),
    ElementLimitExceeded { limit: usize, observed: usize },
}

impl StyleProjectionBuildError {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::SelectorDom(_) => "selector-dom-build",
            Self::ElementLimitExceeded { .. } => "element-limit-exceeded",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleProjectionKeyError {
    RootMismatch,
    MatchingEnvironmentMismatch,
    ProjectionShapeMismatch,
    SourceElementMissing,
    SelectorIdentityMismatch,
    ElementNamespaceMismatch,
    ElementLocalNameMismatch,
}

/// Opaque CSS-owned locator for validating an HTML element against a selector
/// projection. This is neither fixture identity nor a serializable engine ID.
pub struct StyleProjectionElementKey<'dom> {
    root: *const Node,
    source: *const ElementNode,
    source_node: Id,
    selector_element: SelectorDomElementId,
    document_order: usize,
    namespace: ElementNamespace,
    local_name: &'dom str,
    element_count: usize,
    environment: SelectorMatchingEnvironment,
    _dom: PhantomData<&'dom Node>,
}

/// One bounded selector projection and its host-language matching environment.
/// Compatible projections over the same immutable DOM may validate the same
/// key; raw selector element numbers alone are never accepted as provenance.
pub struct StyleProjection<'dom> {
    root: &'dom Node,
    index: SelectorDomIndex<'dom>,
    environment: SelectorMatchingEnvironment,
}

impl<'dom> StyleProjection<'dom> {
    pub fn try_from_document_with_element_limit(
        root: &'dom Node,
        environment: SelectorMatchingEnvironment,
        maximum_elements: usize,
    ) -> Result<Self, StyleProjectionBuildError> {
        let index = SelectorDomIndex::try_from_document_with_element_limit(root, maximum_elements)
            .map_err(|error| match error {
                BoundedSelectorDomConstructionError::Build(error) => {
                    StyleProjectionBuildError::SelectorDom(error)
                }
                BoundedSelectorDomConstructionError::ElementLimitExceeded { limit, observed } => {
                    StyleProjectionBuildError::ElementLimitExceeded { limit, observed }
                }
            })?;
        Ok(Self {
            root,
            index,
            environment,
        })
    }

    pub fn environment(&self) -> SelectorMatchingEnvironment {
        self.environment
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn key_for_element(
        &self,
        element: &'dom ElementNode,
    ) -> Option<StyleProjectionElementKey<'dom>> {
        let selector_element = self.index.element_for_source(element)?;
        let document_order = selector_element.get() as usize - 1;
        Some(StyleProjectionElementKey {
            root: self.root as *const Node,
            source: element as *const ElementNode,
            source_node: element.id(),
            selector_element,
            document_order,
            namespace: element.namespace(),
            local_name: element.name(),
            element_count: self.index.len(),
            environment: self.environment,
            _dom: PhantomData,
        })
    }

    pub fn validate_key(
        &self,
        key: &StyleProjectionElementKey<'dom>,
    ) -> Result<(), StyleProjectionKeyError> {
        if !std::ptr::eq(self.root, key.root) {
            return Err(StyleProjectionKeyError::RootMismatch);
        }
        if self.environment != key.environment {
            return Err(StyleProjectionKeyError::MatchingEnvironmentMismatch);
        }
        if self.index.len() != key.element_count {
            return Err(StyleProjectionKeyError::ProjectionShapeMismatch);
        }
        let mapped = self
            .index
            .elements()
            .find(|element| std::ptr::eq(self.index.source_element(*element), key.source))
            .ok_or(StyleProjectionKeyError::SourceElementMissing)?;
        if self.index.source_element(mapped).id() != key.source_node {
            return Err(StyleProjectionKeyError::SourceElementMissing);
        }
        if mapped != key.selector_element || mapped.get() as usize - 1 != key.document_order {
            return Err(StyleProjectionKeyError::SelectorIdentityMismatch);
        }
        let source = self.index.source_element(mapped);
        if source.namespace() != key.namespace {
            return Err(StyleProjectionKeyError::ElementNamespaceMismatch);
        }
        if source.name() != key.local_name {
            return Err(StyleProjectionKeyError::ElementLocalNameMismatch);
        }
        Ok(())
    }

    pub fn match_selector_list_checked(
        &self,
        key: &StyleProjectionElementKey<'dom>,
        selectors: &SelectorListParseResult,
        limits: SelectorMatchingLimits,
    ) -> Result<SelectorListMatchOutcome, StyleProjectionMatchError> {
        self.validate_key(key)
            .map_err(StyleProjectionMatchError::ProjectionKey)?;
        SelectorMatchingContext::with_limits(&self.index, self.environment, limits)
            .match_selector_list_checked(key.selector_element, selectors)
            .map_err(StyleProjectionMatchError::Matching)
    }

    pub(crate) fn index(&self) -> &SelectorDomIndex<'dom> {
        &self.index
    }

    pub(crate) fn root(&self) -> &'dom Node {
        self.root
    }

    pub(crate) fn selector_element_for_validated_key(
        &self,
        key: &StyleProjectionElementKey<'dom>,
    ) -> Result<SelectorDomElementId, StyleProjectionKeyError> {
        self.validate_key(key)?;
        Ok(key.selector_element)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleProjectionMatchError {
    ProjectionKey(StyleProjectionKeyError),
    Matching(SelectorMatchingLimitError),
}

impl StyleProjectionMatchError {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::ProjectionKey(_) => "projection-key",
            Self::Matching(_) => "matching-resource-limit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use html::DocumentMode;

    fn document() -> Node {
        html::parse_document(
            "<!doctype html><html><body><div class=target></div></body></html>",
            html::HtmlParseOptions::default(),
        )
        .expect("document")
        .document
    }

    #[test]
    fn compatible_projection_revalidates_every_provenance_fact() {
        let root = document();
        let environment = SelectorMatchingEnvironment::new(DocumentMode::NoQuirks);
        let first = StyleProjection::try_from_document_with_element_limit(&root, environment, 16)
            .expect("first projection");
        let target_id = first.index.elements().last().expect("target selector id");
        let target = first.index.source_element(target_id);
        let key = first.key_for_element(target).expect("target key");

        let compatible =
            StyleProjection::try_from_document_with_element_limit(&root, environment, 16)
                .expect("compatible projection");
        assert_eq!(compatible.validate_key(&key), Ok(()));

        let incompatible_environment = StyleProjection::try_from_document_with_element_limit(
            &root,
            SelectorMatchingEnvironment::new(DocumentMode::Quirks),
            16,
        )
        .expect("alternate environment projection");
        assert_eq!(
            incompatible_environment.validate_key(&key),
            Err(StyleProjectionKeyError::MatchingEnvironmentMismatch)
        );

        let other_root = document();
        let other =
            StyleProjection::try_from_document_with_element_limit(&other_root, environment, 16)
                .expect("other root projection");
        assert_eq!(
            other.validate_key(&key),
            Err(StyleProjectionKeyError::RootMismatch)
        );
    }
}

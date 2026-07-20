//! Output models for document-level computed styles.

use crate::selectors::SelectorDomElementId;

use super::super::style::ComputedStyle;

/// Computed style for one DOM element in a document style pass.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedElementStyle {
    pub(in crate::computed) selector_element_id: SelectorDomElementId,
    pub(in crate::computed) element_namespace: html::ElementNamespace,
    pub(in crate::computed) element_name: String,
    pub(in crate::computed) style: ComputedStyle,
}

impl ComputedElementStyle {
    pub(super) fn new(
        selector_element_id: SelectorDomElementId,
        element_namespace: html::ElementNamespace,
        element_name: String,
        style: ComputedStyle,
    ) -> Self {
        Self {
            selector_element_id,
            element_namespace,
            element_name,
            style,
        }
    }

    pub fn selector_element_id(&self) -> SelectorDomElementId {
        self.selector_element_id
    }

    pub fn element_name(&self) -> &str {
        &self.element_name
    }

    pub fn element_namespace(&self) -> html::ElementNamespace {
        self.element_namespace
    }

    pub fn style(&self) -> &ComputedStyle {
        &self.style
    }
}

/// Document-order computed-style output for the element set selector matching
/// can address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedDocumentStyle {
    pub(in crate::computed) entries: Vec<ComputedElementStyle>,
}

impl ComputedDocumentStyle {
    pub(super) fn new(entries: Vec<ComputedElementStyle>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[ComputedElementStyle] {
        &self.entries
    }

    pub fn get(&self, element: SelectorDomElementId) -> Option<&ComputedElementStyle> {
        self.entries
            .iter()
            .find(|entry| entry.selector_element_id == element)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComputedStyleReuseStats {
    pub hits: usize,
    pub misses: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedDocumentStyleWithStats {
    pub computed: ComputedDocumentStyle,
    pub reuse_stats: ComputedStyleReuseStats,
}

use super::contract::ResolvedStyle;
use crate::selectors::{SelectorDomElementId, SelectorMatchingEnvironment};
use std::fmt::Write;

/// Total specified-value/defaulting source resolution for one DOM element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedElementStyle {
    selector_element_id: SelectorDomElementId,
    element_namespace: html::ElementNamespace,
    element_name: String,
    style: ResolvedStyle,
}

impl ResolvedElementStyle {
    pub(super) fn new(
        selector_element_id: SelectorDomElementId,
        element_namespace: html::ElementNamespace,
        element_name: String,
        style: ResolvedStyle,
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

    pub fn style(&self) -> &ResolvedStyle {
        &self.style
    }
}

/// Document-order resolved-style output for the element set selector matching
/// can address.
///
/// This is the structured source-resolution result for the current runtime
/// integration path. It is independent of `html::Node::style` mutation; the
/// legacy bridge projects from this object only after cascade has already
/// resolved winners, symbolic inheritance, and defaults. The result remains
/// bound to the immutable
/// matching environment used for selector evaluation so incremental reuse
/// cannot cross document-mode semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedDocumentStyle {
    matching_environment: SelectorMatchingEnvironment,
    entries: Vec<ResolvedElementStyle>,
}

impl ResolvedDocumentStyle {
    pub(super) fn new(
        matching_environment: SelectorMatchingEnvironment,
        entries: Vec<ResolvedElementStyle>,
    ) -> Self {
        Self {
            matching_environment,
            entries,
        }
    }

    pub fn matching_environment(&self) -> SelectorMatchingEnvironment {
        self.matching_environment
    }

    pub fn entries(&self) -> &[ResolvedElementStyle] {
        &self.entries
    }

    pub fn get(&self, element: SelectorDomElementId) -> Option<&ResolvedElementStyle> {
        self.entries
            .iter()
            .find(|entry| entry.selector_element_id == element)
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 3").expect("write snapshot");
        writeln!(&mut out, "resolved-document-style").expect("write snapshot");
        for (index, entry) in self.entries.iter().enumerate() {
            writeln!(
                &mut out,
                "element[{index}]: selector-id={} namespace={} name=\"{}\"",
                entry.selector_element_id.get(),
                entry.element_namespace.snapshot_name(),
                entry.element_name
            )
            .expect("write snapshot");
            for line in entry.style.to_debug_snapshot().lines().skip(1) {
                writeln!(&mut out, "  {line}").expect("write snapshot");
            }
        }
        out
    }
}

use crate::{
    model,
    selectors::{SelectorDomElementIter, SelectorDomIndex, SelectorMatchingContext},
};

use html::{Node, internal::Id};

use super::{
    document::{
        ComputedDocumentStyle, ComputedElementStyle, ComputedStyleResolutionError,
        compute_document_styles,
    },
    style::ComputedStyle,
};

/// A node in the style tree: pairs a DOM node with its computed style
/// and the styled children.
///
/// This forms a parallel tree to the DOM:
/// - Same shape (for elements we care about)
/// - Holds computed, inherited CSS values
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub node_id: Id,
    pub style: ComputedStyle,
    pub children: Vec<StyledNode<'a>>,
}

/// Builds a styled tree from stylesheets through the structured
/// cascade-to-computed pipeline without mutating `Node::style`.
pub fn build_style_tree_with_stylesheets<'a>(
    root: &'a html::Node,
    sheets: &[model::StylesheetParse],
) -> Result<StyledNode<'a>, ComputedStyleResolutionError> {
    let computed_styles = compute_document_styles(root, sheets)?;
    build_style_tree_from_computed_styles(root, &computed_styles)
}

/// Builds a styled tree from a precomputed document-style result.
pub fn build_style_tree_from_computed_styles<'a>(
    root: &'a html::Node,
    computed_styles: &ComputedDocumentStyle,
) -> Result<StyledNode<'a>, ComputedStyleResolutionError> {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut element_ids = index.elements();
    let mut entries = ComputedElementStyleCursor::new(computed_styles.entries());
    let styled = build_style_tree_from_computed_entries(
        root,
        None,
        &context,
        &mut element_ids,
        &mut entries,
    )?;
    if let Some(missing_element) = element_ids.next() {
        return Err(ComputedStyleResolutionError::MissingComputedElementStyle {
            element_index: entries.next_index(),
            element_name: context.element_name(missing_element).to_string(),
        });
    }
    if let Some(extra) = entries.next_entry() {
        return Err(ComputedStyleResolutionError::ExtraComputedElementStyle {
            element: extra.selector_element_id(),
        });
    }

    Ok(styled)
}

fn build_style_tree_from_computed_entries<'a, 'b>(
    node: &'a Node,
    parent_style: Option<&ComputedStyle>,
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element_ids: &mut SelectorDomElementIter,
    entries: &mut ComputedElementStyleCursor<'b>,
) -> Result<StyledNode<'a>, ComputedStyleResolutionError> {
    match node {
        Node::Document { children, .. } => {
            let base = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree_from_computed_entries(
                    child,
                    Some(&base),
                    context,
                    element_ids,
                    entries,
                )?);
            }

            Ok(StyledNode {
                node,
                node_id: node.id(),
                style: base,
                children: styled_children,
            })
        }

        Node::Element { name, children, .. } => {
            let element_index = entries.next_index();
            let expected_selector_id = element_ids.next().ok_or_else(|| {
                ComputedStyleResolutionError::MissingComputedElementStyle {
                    element_index,
                    element_name: name.to_string(),
                }
            })?;
            let entry = entries.next_entry().ok_or_else(|| {
                ComputedStyleResolutionError::MissingComputedElementStyle {
                    element_index,
                    element_name: name.to_string(),
                }
            })?;
            if entry.selector_element_id() != expected_selector_id {
                return Err(
                    ComputedStyleResolutionError::ComputedElementIdentityMismatch {
                        element_index,
                        expected: expected_selector_id,
                        actual: entry.selector_element_id(),
                    },
                );
            }

            let expected_name = context.element_name(expected_selector_id);
            if expected_name != name.as_ref() || entry.element_name() != expected_name {
                return Err(ComputedStyleResolutionError::ComputedElementNameMismatch {
                    element_index,
                    expected: expected_name.to_string(),
                    actual: entry.element_name().to_string(),
                });
            }

            let computed = *entry.style();
            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree_from_computed_entries(
                    child,
                    Some(&computed),
                    context,
                    element_ids,
                    entries,
                )?);
            }

            Ok(StyledNode {
                node,
                node_id: node.id(),
                style: computed,
                children: styled_children,
            })
        }

        Node::Text { .. } | Node::Comment { .. } => {
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            Ok(StyledNode {
                node,
                node_id: node.id(),
                style: inherited,
                children: Vec::new(),
            })
        }
    }
}

struct ComputedElementStyleCursor<'a> {
    entries: &'a [ComputedElementStyle],
    next_index: usize,
}

impl<'a> ComputedElementStyleCursor<'a> {
    fn new(entries: &'a [ComputedElementStyle]) -> Self {
        Self {
            entries,
            next_index: 0,
        }
    }

    fn next_index(&self) -> usize {
        self.next_index
    }

    fn next_entry(&mut self) -> Option<&'a ComputedElementStyle> {
        let entry = self.entries.get(self.next_index)?;
        self.next_index += 1;
        Some(entry)
    }
}

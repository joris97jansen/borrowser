use super::contract::ResolvedStyle;
use super::document::ResolvedElementStyle;
use super::integration::resolve_document_styles;
use crate::model;
use html::Node;

/// Legacy DOM-attached style bridge.
///
/// Cascade itself is no longer driven by this mutation path. The bridge first
/// resolves the structured document style output, then projects authored winner
/// values back into `Node::Element::style` for the pre-computed-values runtime
/// path that still consumes string declarations.
pub fn attach_styles(dom: &mut Node, sheets: &[model::StylesheetParse]) {
    let resolved_styles = resolve_document_styles(dom, sheets);
    let mut entries = resolved_styles.entries().iter();
    project_resolved_styles_to_dom(dom, &mut entries);
    debug_assert!(
        entries.next().is_none(),
        "resolved document style must contain exactly one entry per element"
    );
}

fn project_resolved_styles_to_dom<'a>(
    node: &mut Node,
    entries: &mut std::slice::Iter<'a, ResolvedElementStyle>,
) {
    match node {
        Node::Document { children, .. } => {
            for child in children {
                project_resolved_styles_to_dom(child, entries);
            }
        }
        Node::Element {
            style, children, ..
        } => {
            let resolved = entries
                .next()
                .expect("resolved document style missing element entry");
            project_resolved_style_to_legacy_vector(resolved.style(), style);
            for child in children {
                project_resolved_styles_to_dom(child, entries);
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn project_resolved_style_to_legacy_vector(
    resolved_style: &ResolvedStyle,
    target: &mut Vec<(String, String)>,
) {
    target.clear();
    for entry in resolved_style.entries() {
        let Some(winner) = entry.winner() else {
            continue;
        };
        let Some(value) = winner.value.to_css_text() else {
            continue;
        };
        target.push((entry.property().name().to_string(), value));
    }
}

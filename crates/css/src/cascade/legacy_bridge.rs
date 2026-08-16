use super::contract::ResolvedStyle;
use super::document::ResolvedElementStyle;
use super::integration::{
    StyleResolutionLimits, try_resolve_document_styles_with_limits,
    try_resolve_element_subtree_styles_with_limits,
};
use crate::model;
use crate::selectors::SelectorMatchingEnvironment;
use html::Node;

/// Legacy DOM-attached style bridge.
///
/// Cascade itself is no longer driven by this mutation path. The bridge first
/// resolves the structured document style output, then projects authored winner
/// values back into `Node::Element::style` for the pre-computed-values runtime
/// path that still consumes string declarations.
///
/// This is a compatibility path, not the authoritative resolved-style API. It
/// deliberately preserves the historical element-rooted bridge by using the
/// explicit selector element-subtree projection; that root is never treated as
/// a document element. If style resolution fails, the bridge clears any legacy
/// projected style vectors and returns without projecting a partial or
/// fabricated resolved-style result.
pub fn attach_styles(
    dom: &mut Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
) {
    let limits = StyleResolutionLimits::default();
    let resolution = match &*dom {
        Node::Document { .. } => {
            try_resolve_document_styles_with_limits(dom, matching_environment, sheets, &limits)
        }
        Node::Element { element } => try_resolve_element_subtree_styles_with_limits(
            element,
            matching_environment,
            sheets,
            &limits,
        ),
        _ => {
            #[cfg(debug_assertions)]
            eprintln!("legacy attach_styles received a non-document, non-element root");
            clear_legacy_styles(dom);
            return;
        }
    };
    let resolved_styles = match resolution {
        Ok(resolved_styles) => resolved_styles,
        Err(_error) => {
            #[cfg(debug_assertions)]
            eprintln!("legacy attach_styles degraded style resolution failure: {_error}");
            clear_legacy_styles(dom);
            return;
        }
    };
    let mut entries = resolved_styles.entries().iter();
    if !project_resolved_styles_to_dom(dom, &mut entries) {
        #[cfg(debug_assertions)]
        eprintln!("legacy attach_styles degraded resolved-style projection invariant failure");
        clear_legacy_styles(dom);
        return;
    }
    debug_assert!(
        entries.next().is_none(),
        "resolved document style must contain exactly one entry per element"
    );
}

fn project_resolved_styles_to_dom<'a>(
    node: &mut Node,
    entries: &mut std::slice::Iter<'a, ResolvedElementStyle>,
) -> bool {
    match node {
        Node::Document { children, .. } => {
            for child in children {
                if !project_resolved_styles_to_dom(child, entries) {
                    return false;
                }
            }
            true
        }
        Node::Element { element } => {
            let Some(resolved) = entries.next() else {
                return false;
            };
            project_resolved_style_to_legacy_vector(resolved.style(), element.style_mut());
            for child in element.children_mut() {
                if !project_resolved_styles_to_dom(child, entries) {
                    return false;
                }
            }
            true
        }
        Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. }
        | Node::DocumentType { .. } => true,
    }
}

fn clear_legacy_styles(node: &mut Node) {
    match node {
        Node::Document { children, .. } => {
            for child in children {
                clear_legacy_styles(child);
            }
        }
        Node::Element { element } => {
            element.style_mut().clear();
            for child in element.children_mut() {
                clear_legacy_styles(child);
            }
        }
        Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. }
        | Node::DocumentType { .. } => {}
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

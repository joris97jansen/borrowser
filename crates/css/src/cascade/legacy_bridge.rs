use super::contract::ResolvedStyle;
use super::document::ResolvedElementStyle;
use super::integration::{StyleResolutionLimits, try_resolve_document_styles_with_limits};
use crate::model;
use html::Node;

/// Legacy DOM-attached style bridge.
///
/// Cascade itself is no longer driven by this mutation path. The bridge first
/// resolves the structured document style output, then projects authored winner
/// values back into `Node::Element::style` for the pre-computed-values runtime
/// path that still consumes string declarations.
///
/// This is a compatibility path, not the authoritative resolved-style API. If
/// document style resolution hits a hardening limit, the bridge clears any
/// legacy projected style vectors and returns without projecting a partial or
/// fabricated resolved-style result.
pub fn attach_styles(dom: &mut Node, sheets: &[model::StylesheetParse]) {
    let resolved_styles = match try_resolve_document_styles_with_limits(
        dom,
        sheets,
        &StyleResolutionLimits::default(),
    ) {
        Ok(resolved_styles) => resolved_styles,
        Err(error) => {
            #[cfg(debug_assertions)]
            eprintln!("legacy attach_styles degraded style resolution failure: {error}");
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
        Node::Element {
            style, children, ..
        } => {
            let Some(resolved) = entries.next() else {
                return false;
            };
            project_resolved_style_to_legacy_vector(resolved.style(), style);
            for child in children {
                if !project_resolved_styles_to_dom(child, entries) {
                    return false;
                }
            }
            true
        }
        Node::Text { .. } | Node::Comment { .. } => true,
    }
}

fn clear_legacy_styles(node: &mut Node) {
    match node {
        Node::Document { children, .. } => {
            for child in children {
                clear_legacy_styles(child);
            }
        }
        Node::Element {
            style, children, ..
        } => {
            style.clear();
            for child in children {
                clear_legacy_styles(child);
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

//! Authored HTML occurrences, independent of selected loading associations.
use super::DocumentAddressState;
use addressing::{ResourceAddress, ResourceKind, UrlDiagnostic, UrlFailure};
use html::{ElementNamespace, Node, internal::Id};

/// IDs refer only to the originating committed document. This ephemeral
/// inventory neither deduplicates addresses nor represents issued requests.
pub(crate) struct HtmlResourceReference {
    pub(crate) element: Id,
    pub(crate) attribute: &'static str,
    pub(crate) kind: ResourceKind,
    pub(crate) outcome: HtmlReferenceOutcome,
}

#[derive(Debug)]
pub(crate) enum HtmlReferenceOutcome {
    Resolved {
        address: ResourceAddress,
        diagnostics: Vec<UrlDiagnostic>,
    },
    /// HTML inventory outcome, not a URL parse failure or effective load result.
    EmptyReference,
    Failed(UrlFailure),
}

pub(super) fn collect_html_resource_references(
    dom: &Node,
    addresses: &DocumentAddressState,
) -> Vec<HtmlResourceReference> {
    let mut records = Vec::new();
    // Carry only the immediate parent's media classification. Ordinary children
    // exclude template contents; foreign nodes still contribute their children.
    let mut stack = vec![(dom, false)];
    while let Some((node, parent_is_media)) = stack.pop() {
        let element = match node {
            Node::Document { children, .. } => {
                stack.extend(children.iter().rev().map(|child| (child, false)));
                continue;
            }
            Node::Element { element } => element,
            _ => continue,
        };
        let is_html = element.namespace() == ElementNamespace::Html;
        let is_media = is_html && matches!(element.name(), "audio" | "video");
        stack.extend(
            element
                .children()
                .iter()
                .rev()
                .map(|child| (child, is_media)),
        );
        if !is_html {
            continue;
        }
        let (attribute, kind) = match element.name() {
            "link" if node.attr("rel").is_some_and(has_stylesheet_token) => {
                ("href", ResourceKind::Stylesheet)
            }
            "img" => ("src", ResourceKind::Image),
            "source" if parent_is_media => ("src", ResourceKind::Media),
            "a" => ("href", ResourceKind::Hyperlink),
            "form" => ("action", ResourceKind::FormAction),
            "iframe" => ("src", ResourceKind::Document),
            "script" if is_javascript_reference(node) => ("src", ResourceKind::Script),
            _ => continue,
        };
        let Some(value) = node.attr(attribute) else {
            continue;
        };
        let outcome = if value.is_empty() && kind == ResourceKind::FormAction {
            HtmlReferenceOutcome::Resolved {
                address: ResourceAddress::new(addresses.document_url().clone(), kind),
                diagnostics: Vec::new(),
            }
        } else if value.is_empty() && kind != ResourceKind::Hyperlink {
            HtmlReferenceOutcome::EmptyReference
        } else {
            // No universal trimming: preserve URL-parser diagnostics and the
            // distinction between exactly empty and nonempty whitespace values.
            match addresses.active_base_url().resolve(value) {
                Ok(parsed) => HtmlReferenceOutcome::Resolved {
                    address: ResourceAddress::new(parsed.url, kind),
                    diagnostics: parsed.diagnostics,
                },
                Err(failure) => HtmlReferenceOutcome::Failed(failure),
            }
        };
        records.push(HtmlResourceReference {
            element: node.id(),
            attribute,
            kind,
            outcome,
        });
    }
    records
}

fn html_ascii_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

fn has_stylesheet_token(rel: &str) -> bool {
    rel.split(html_ascii_whitespace)
        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
}

/// Only resource-purpose classification, not script preparation or execution.
fn is_javascript_reference(node: &Node) -> bool {
    if let Some(script_type) = node.attr("type") {
        script_type.is_empty()
            || javascript_mime_essence(script_type.trim_matches(html_ascii_whitespace))
            || script_type.eq_ignore_ascii_case("module")
    } else {
        node.attr("language").is_none_or(|language| {
            language.is_empty() || javascript_mime_essence(&format!("text/{language}"))
        })
    }
}

fn javascript_mime_essence(value: &str) -> bool {
    // MIME Sniffing's closed JavaScript essence-match list. Parameters are not
    // removed, and this does not introduce a general MIME parser or sniffer.
    [
        "application/ecmascript",
        "application/javascript",
        "application/x-ecmascript",
        "application/x-javascript",
        "text/ecmascript",
        "text/javascript",
        "text/javascript1.0",
        "text/javascript1.1",
        "text/javascript1.2",
        "text/javascript1.3",
        "text/javascript1.4",
        "text/javascript1.5",
        "text/jscript",
        "text/livescript",
        "text/x-ecmascript",
        "text/x-javascript",
    ]
    .iter()
    .any(|essence| value.eq_ignore_ascii_case(essence))
}

#[cfg(test)]
mod tests;

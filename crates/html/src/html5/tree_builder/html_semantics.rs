//! Shared HTML-namespace tree-builder semantic categories.

use crate::html5::shared::AtomTable;
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::resolve::resolve_atom;
use crate::html5::tree_builder::stack::OpenElement;
use crate::names::ElementNamespace;

// WHATWG HTML `source` revision 85b40db7c40436be8d459e8f4ca2120e823c34f0,
// "The stack of open elements" / "Special". HTML namespace only.
// Keep sorted for deterministic allocation-free binary search.
const HTML_SPECIAL_ELEMENT_NAMES: [&str; 83] = [
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "keygen",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "search",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
];

pub(in crate::html5::tree_builder) fn is_special_element(
    element: OpenElement,
    atoms: &AtomTable,
) -> Result<bool, TreeBuilderError> {
    let resolved = resolve_atom(atoms, element.name())?;
    Ok(match element.namespace() {
        ElementNamespace::Html => HTML_SPECIAL_ELEMENT_NAMES.binary_search(&resolved).is_ok(),
        ElementNamespace::MathMl => matches!(
            resolved,
            "mi" | "mo" | "mn" | "ms" | "mtext" | "annotation-xml"
        ),
        ElementNamespace::Svg => matches!(resolved, "foreignObject" | "desc" | "title"),
    })
}

#[cfg(test)]
fn is_special_html_element(
    name: crate::html5::shared::AtomId,
    atoms: &AtomTable,
) -> Result<bool, TreeBuilderError> {
    let resolved = resolve_atom(atoms, name)?;
    Ok(HTML_SPECIAL_ELEMENT_NAMES.binary_search(&resolved).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{HTML_SPECIAL_ELEMENT_NAMES, is_special_element, is_special_html_element};
    use crate::ElementNamespace;
    use crate::dom_patch::PatchKey;
    use crate::html5::shared::{AtomId, DocumentParseContext, EngineInvariantError};
    use crate::html5::tree_builder::stack::OpenElement;

    // Independent pinned test oracle. Do not derive this from the production
    // table: equality catches missing, extra, and reordered production entries.
    const WHATWG_85B40DB_HTML_SPECIAL_ELEMENTS: [&str; 83] = [
        "address",
        "applet",
        "area",
        "article",
        "aside",
        "base",
        "basefont",
        "bgsound",
        "blockquote",
        "body",
        "br",
        "button",
        "caption",
        "center",
        "col",
        "colgroup",
        "dd",
        "details",
        "dir",
        "div",
        "dl",
        "dt",
        "embed",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "frame",
        "frameset",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hgroup",
        "hr",
        "html",
        "iframe",
        "img",
        "input",
        "keygen",
        "li",
        "link",
        "listing",
        "main",
        "marquee",
        "menu",
        "meta",
        "nav",
        "noembed",
        "noframes",
        "noscript",
        "object",
        "ol",
        "p",
        "param",
        "plaintext",
        "pre",
        "script",
        "search",
        "section",
        "select",
        "source",
        "style",
        "summary",
        "table",
        "tbody",
        "td",
        "template",
        "textarea",
        "tfoot",
        "th",
        "thead",
        "title",
        "tr",
        "track",
        "ul",
        "wbr",
        "xmp",
    ];

    #[test]
    fn production_special_category_matches_pinned_independent_oracle() {
        assert_eq!(HTML_SPECIAL_ELEMENT_NAMES.len(), 83);
        assert_eq!(WHATWG_85B40DB_HTML_SPECIAL_ELEMENTS.len(), 83);
        assert_eq!(
            HTML_SPECIAL_ELEMENT_NAMES,
            WHATWG_85B40DB_HTML_SPECIAL_ELEMENTS
        );
        assert!(
            HTML_SPECIAL_ELEMENT_NAMES
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "production special-category table must be strictly sorted and duplicate-free"
        );
        assert!(
            WHATWG_85B40DB_HTML_SPECIAL_ELEMENTS
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "pinned test oracle must be strictly sorted and duplicate-free"
        );
    }

    #[test]
    fn foreign_special_elements_require_the_complete_expanded_name() {
        let mut ctx = DocumentParseContext::new();

        for (index, local) in ["mi", "mo", "mn", "ms", "mtext", "annotation-xml"]
            .into_iter()
            .enumerate()
        {
            let atom = ctx.atoms.intern_exact(local).expect("MathML special atom");
            assert!(
                is_special_element(
                    OpenElement::new_foreign(
                        PatchKey(100 + index as u32),
                        ElementNamespace::MathMl,
                        atom,
                    ),
                    &ctx.atoms,
                )
                .expect("MathML special classification"),
                "MathML {local} must be special"
            );
            assert!(
                !is_special_element(
                    OpenElement::new_html(PatchKey(200 + index as u32), atom),
                    &ctx.atoms,
                )
                .expect("HTML lookalike classification"),
                "HTML {local} must not inherit MathML special semantics"
            );
            assert!(
                !is_special_element(
                    OpenElement::new_foreign(
                        PatchKey(300 + index as u32),
                        ElementNamespace::Svg,
                        atom,
                    ),
                    &ctx.atoms,
                )
                .expect("SVG lookalike classification"),
                "SVG {local} must not inherit MathML special semantics"
            );
        }

        for (index, local) in ["foreignObject", "desc", "title"].into_iter().enumerate() {
            let atom = ctx.atoms.intern_exact(local).expect("SVG special atom");
            assert!(
                is_special_element(
                    OpenElement::new_foreign(
                        PatchKey(400 + index as u32),
                        ElementNamespace::Svg,
                        atom,
                    ),
                    &ctx.atoms,
                )
                .expect("SVG special classification"),
                "SVG {local} must be special"
            );
            assert!(
                !is_special_element(
                    OpenElement::new_foreign(
                        PatchKey(500 + index as u32),
                        ElementNamespace::MathMl,
                        atom,
                    ),
                    &ctx.atoms,
                )
                .expect("MathML lookalike classification"),
                "MathML {local} must not inherit SVG special semantics"
            );
        }

        let title = ctx.atoms.intern_exact("title").expect("title atom");
        assert!(
            is_special_element(OpenElement::new_html(PatchKey(600), title), &ctx.atoms)
                .expect("HTML title classification"),
            "HTML title remains special through the independent HTML taxonomy"
        );
        for (key, local) in [(601, "foreignObject"), (602, "desc")] {
            let atom = ctx.atoms.intern_exact(local).expect("SVG lookalike atom");
            assert!(
                !is_special_element(OpenElement::new_html(PatchKey(key), atom), &ctx.atoms)
                    .expect("HTML lookalike classification"),
                "HTML {local} must not inherit SVG special semantics"
            );
        }
    }

    #[test]
    fn classifier_accepts_every_pinned_name_and_rejects_representative_others() {
        let mut ctx = DocumentParseContext::new();
        for name in WHATWG_85B40DB_HTML_SPECIAL_ELEMENTS {
            let atom = ctx.atoms.intern_ascii_folded(name).expect("atom");
            assert!(
                is_special_html_element(atom, &ctx.atoms).expect("valid atom"),
                "{name} must be special"
            );
        }
        for name in ["option", "optgroup", "span", "x-custom"] {
            let atom = ctx.atoms.intern_ascii_folded(name).expect("atom");
            assert!(
                !is_special_html_element(atom, &ctx.atoms).expect("valid atom"),
                "{name} must not be special"
            );
        }
        for name in ["keygen", "noscript", "object"] {
            let atom = ctx.atoms.intern_ascii_folded(name).expect("atom");
            assert!(is_special_html_element(atom, &ctx.atoms).expect("valid atom"));
        }
    }

    #[test]
    fn classifier_rejects_an_invalid_atom_with_the_exact_invariant_error() {
        let ctx = DocumentParseContext::new();
        assert!(matches!(
            is_special_html_element(
                AtomId::for_test(ctx.atoms.id() as u32, u32::MAX),
                &ctx.atoms,
            ),
            Err(EngineInvariantError)
        ));
    }
}

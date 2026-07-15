//! Shared HTML-namespace tree-builder semantic categories.

use crate::html5::shared::{AtomId, AtomTable};
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::resolve::resolve_atom;

// WHATWG HTML `source` revision 88ae68cb961651f0f92c5d2046049f53ecdfc6cf,
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

pub(in crate::html5::tree_builder) fn is_special_html_element(
    name: AtomId,
    atoms: &AtomTable,
) -> Result<bool, TreeBuilderError> {
    let resolved = resolve_atom(atoms, name)?;
    Ok(HTML_SPECIAL_ELEMENT_NAMES.binary_search(&resolved).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{HTML_SPECIAL_ELEMENT_NAMES, is_special_html_element};
    use crate::html5::shared::{AtomId, DocumentParseContext, EngineInvariantError};

    // Independent pinned test oracle. Do not derive this from the production
    // table: equality catches missing, extra, and reordered production entries.
    const WHATWG_88AE68C_HTML_SPECIAL_ELEMENTS: [&str; 83] = [
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
        assert_eq!(WHATWG_88AE68C_HTML_SPECIAL_ELEMENTS.len(), 83);
        assert_eq!(
            HTML_SPECIAL_ELEMENT_NAMES,
            WHATWG_88AE68C_HTML_SPECIAL_ELEMENTS
        );
        assert!(
            HTML_SPECIAL_ELEMENT_NAMES
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "production special-category table must be strictly sorted and duplicate-free"
        );
        assert!(
            WHATWG_88AE68C_HTML_SPECIAL_ELEMENTS
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "pinned test oracle must be strictly sorted and duplicate-free"
        );
    }

    #[test]
    fn classifier_accepts_every_pinned_name_and_rejects_representative_others() {
        let mut ctx = DocumentParseContext::new();
        for name in WHATWG_88AE68C_HTML_SPECIAL_ELEMENTS {
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
            is_special_html_element(AtomId(u32::MAX), &ctx.atoms),
            Err(EngineInvariantError)
        ));
    }
}

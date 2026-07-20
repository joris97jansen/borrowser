pub(crate) mod patch_apply;

use crate::names::NameInterner;
use crate::{ElementNamespace, ExpandedElementName, ParserCreatedAttribute};

/// Test-only explicit HTML construction. Production construction APIs never
/// infer a namespace.
pub(crate) fn html_name(local_name: &str) -> ExpandedElementName {
    let mut names = NameInterner::new();
    let atom = names
        .intern_ascii_folded(local_name)
        .expect("test local-name interner exhausted");
    names
        .expanded_name(ElementNamespace::Html, atom)
        .expect("test local name missing from interner")
}

/// Test-only DOM-string attribute construction. `None` is accepted only as a
/// compact source-syntax fixture spelling and normalizes immediately to "".
pub(crate) fn html_attribute(local_name: &str, value: Option<&str>) -> ParserCreatedAttribute {
    let mut names = NameInterner::new();
    let atom = names
        .intern_ascii_folded(local_name)
        .expect("test attribute-name interner exhausted");
    ParserCreatedAttribute::unqualified(&names, atom, value.unwrap_or_default().to_string())
        .expect("test attribute name missing from interner")
}

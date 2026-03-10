use crate::html5::shared::EngineInvariantError;
use crate::html5::shared::{AtomId, AtomTable, Attribute, AttributeValue, TextValue};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::TreeBuilderError;

pub(in crate::html5::tree_builder) fn resolve_atom(
    atoms: &AtomTable,
    id: AtomId,
) -> Result<&str, TreeBuilderError> {
    atoms.resolve(id).ok_or(EngineInvariantError)
}

pub(in crate::html5::tree_builder) fn resolve_atom_arc(
    atoms: &AtomTable,
    id: AtomId,
) -> Result<std::sync::Arc<str>, TreeBuilderError> {
    atoms.resolve_arc(id).ok_or(EngineInvariantError)
}

pub(in crate::html5::tree_builder) fn resolve_attribute_value(
    attribute: &Attribute,
    text: &dyn TextResolver,
) -> Result<Option<String>, TreeBuilderError> {
    match &attribute.value {
        None => Ok(None),
        Some(AttributeValue::Owned(value)) => Ok(Some(value.clone())),
        Some(AttributeValue::Span(span)) => text
            .resolve_span(*span)
            .map(|value| Some(value.to_string()))
            .map_err(|_| EngineInvariantError),
    }
}

pub(in crate::html5::tree_builder) fn resolve_text_value(
    value: &TextValue,
    text: &dyn TextResolver,
) -> Result<String, TreeBuilderError> {
    match value {
        TextValue::Owned(value) => Ok(value.clone()),
        TextValue::Span(span) => text
            .resolve_span(*span)
            .map(|value| value.to_string())
            .map_err(|_| EngineInvariantError),
    }
}

pub(in crate::html5::tree_builder) fn is_html_whitespace_text(
    value: &TextValue,
    text: &dyn TextResolver,
) -> Result<bool, TreeBuilderError> {
    match value {
        TextValue::Owned(value) => Ok(is_html_whitespace_str(value)),
        TextValue::Span(span) => text
            .resolve_span(*span)
            .map(is_html_whitespace_str)
            .map_err(|_| EngineInvariantError),
    }
}

#[inline]
pub(in crate::html5::tree_builder) fn is_html_whitespace_str(value: &str) -> bool {
    value
        .as_bytes()
        .iter()
        .copied()
        .all(is_html_whitespace_byte)
}

#[inline]
pub(in crate::html5::tree_builder) fn is_html_whitespace_byte(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

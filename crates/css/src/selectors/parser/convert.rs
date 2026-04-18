use super::segment::SegmentParseError;
use super::spans::token_text_span;
use super::{
    CssInput, CssTokenText, InvalidSelectorReason, SelectorIdent, SelectorString,
    SelectorStructureError,
};

pub(super) fn selector_ident_from_text(
    input: &CssInput,
    text: &CssTokenText,
) -> Result<SelectorIdent, SegmentParseError> {
    let value = text
        .resolve(input)
        .ok_or_else(|| SegmentParseError::Invalid {
            span: token_text_span(text),
            reason: InvalidSelectorReason::UnexpectedComponentValue,
        })?;

    SelectorIdent::new(value.into_owned(), token_text_span(text)).map_err(|error| {
        SegmentParseError::Invalid {
            span: token_text_span(text),
            reason: map_structure_error(error),
        }
    })
}

pub(super) fn selector_string_from_text(
    input: &CssInput,
    text: &CssTokenText,
) -> Result<SelectorString, SegmentParseError> {
    let value = text
        .resolve(input)
        .ok_or_else(|| SegmentParseError::Invalid {
            span: token_text_span(text),
            reason: InvalidSelectorReason::UnexpectedComponentValue,
        })?;

    Ok(SelectorString::new(
        value.into_owned(),
        token_text_span(text),
    ))
}

pub(super) fn map_structure_error(error: SelectorStructureError) -> InvalidSelectorReason {
    match error {
        SelectorStructureError::EmptySelectorList => InvalidSelectorReason::EmptySelectorList,
        SelectorStructureError::EmptyCompoundSelector => {
            InvalidSelectorReason::EmptyCompoundSelector
        }
        SelectorStructureError::EmptyIdentifier
        | SelectorStructureError::MixedInputIds
        | SelectorStructureError::NonMonotonicSpans
        | SelectorStructureError::PayloadSpanOutsideNode => {
            InvalidSelectorReason::UnexpectedComponentValue
        }
    }
}

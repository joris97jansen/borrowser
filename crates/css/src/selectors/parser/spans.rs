use super::{CssComponentValue, CssSpan, CssTokenText};

pub(super) fn component_value_span(value: &CssComponentValue) -> CssSpan {
    match value {
        CssComponentValue::PreservedToken(token) => token.span,
        CssComponentValue::SimpleBlock(block) => block.span,
        CssComponentValue::Function(function) => function.span,
    }
}

pub(super) fn component_list_span(values: &[CssComponentValue]) -> Option<CssSpan> {
    let start = values.first().map(component_value_span)?;
    let end = values.last().map(component_value_span)?;
    span_from_bounds(start, end)
}

pub(super) fn span_from_bounds(start: CssSpan, end: CssSpan) -> Option<CssSpan> {
    if start.input_id != end.input_id || end.end < start.start {
        return None;
    }

    CssSpan::new(start.input_id, start.start, end.end)
}

pub(super) fn token_text_span(text: &CssTokenText) -> Option<CssSpan> {
    match text {
        CssTokenText::Span(span) => Some(*span),
        CssTokenText::Owned(_) => None,
    }
}

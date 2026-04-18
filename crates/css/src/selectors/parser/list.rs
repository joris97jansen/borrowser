use super::convert::map_structure_error;
use super::segment::{SegmentParseResult, SegmentParser};
use super::spans::component_list_span;
use super::trivia::is_trivia_component;
use super::{
    CssComponentValue, CssInput, CssToken, CssTokenKind, InvalidSelectorList,
    InvalidSelectorReason, SelectorList, SelectorListParseResult, UnsupportedSelectorList,
};

pub fn parse_selector_list(
    input: &CssInput,
    values: &[CssComponentValue],
) -> SelectorListParseResult {
    let overall_span = component_list_span(values);
    let has_any_significant = values.iter().any(|value| !is_trivia_component(value));

    if !has_any_significant {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            InvalidSelectorReason::EmptySelectorList,
        ));
    }

    let mut selectors = Vec::new();
    let mut unsupported = Vec::new();
    let mut segment_start = 0usize;

    for comma_index in top_level_comma_indices(values).chain(std::iter::once(values.len())) {
        let segment = &values[segment_start..comma_index];
        match parse_selector_segment(input, segment, overall_span) {
            SegmentParseResult::Parsed(selector) => selectors.push(selector),
            SegmentParseResult::Unsupported(features) => unsupported.extend(features),
            SegmentParseResult::Invalid { span, reason } => {
                return SelectorListParseResult::Invalid(InvalidSelectorList::new(span, reason));
            }
        }
        segment_start = comma_index.saturating_add(1);
    }

    if !unsupported.is_empty() {
        return SelectorListParseResult::Unsupported(UnsupportedSelectorList::from_features(
            overall_span,
            unsupported,
        ));
    }

    match SelectorList::new(overall_span, selectors) {
        Ok(list) => SelectorListParseResult::Parsed(list),
        Err(error) => SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            map_structure_error(error),
        )),
    }
}

fn parse_selector_segment(
    input: &CssInput,
    values: &[CssComponentValue],
    fallback_span: Option<super::CssSpan>,
) -> SegmentParseResult {
    SegmentParser::new(input, values).parse(fallback_span)
}

fn top_level_comma_indices(values: &[CssComponentValue]) -> impl Iterator<Item = usize> + '_ {
    values.iter().enumerate().filter_map(|(index, value)| {
        matches!(
            value,
            CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comma,
                ..
            })
        )
        .then_some(index)
    })
}

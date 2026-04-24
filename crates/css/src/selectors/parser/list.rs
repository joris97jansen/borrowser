use super::convert::map_structure_error;
use super::segment::{SegmentParseResult, SegmentParser};
use super::spans::component_list_span;
use super::trivia::is_trivia_component;
use super::{
    CssComponentValue, CssInput, CssToken, CssTokenKind, InvalidSelectorList,
    InvalidSelectorReason, SelectorList, SelectorListParseResult, UnsupportedSelectorList,
};
use crate::syntax::SyntaxLimits;

pub fn parse_selector_list(
    input: &CssInput,
    values: &[CssComponentValue],
) -> SelectorListParseResult {
    parse_selector_list_with_limits(input, values, &SyntaxLimits::default())
}

pub fn parse_selector_list_with_limits(
    input: &CssInput,
    values: &[CssComponentValue],
    limits: &SyntaxLimits,
) -> SelectorListParseResult {
    let overall_span = component_list_span(values);
    let has_any_significant = values.iter().any(|value| !is_trivia_component(value));

    if !has_any_significant {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            InvalidSelectorReason::EmptySelectorList,
        ));
    }

    let significant_count = values
        .iter()
        .filter(|value| !is_trivia_component(value))
        .take(limits.max_selector_component_values.saturating_add(1))
        .count();
    if significant_count > limits.max_selector_component_values {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            InvalidSelectorReason::ResourceLimitExceeded,
        ));
    }

    let mut selectors = Vec::new();
    let mut unsupported = Vec::new();
    let mut segment_start = 0usize;
    let comma_indices: Vec<_> = top_level_comma_indices(values).collect();
    let selector_count = comma_indices.len().saturating_add(1);
    if selector_count > limits.max_selectors_per_rule {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            InvalidSelectorReason::ResourceLimitExceeded,
        ));
    }

    for comma_index in comma_indices
        .into_iter()
        .chain(std::iter::once(values.len()))
    {
        let segment = &values[segment_start..comma_index];
        match parse_selector_segment(input, segment, overall_span, limits) {
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
    limits: &SyntaxLimits,
) -> SegmentParseResult {
    SegmentParser::new(input, values, limits).parse(fallback_span)
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

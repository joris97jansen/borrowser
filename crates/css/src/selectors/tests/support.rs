use super::super::{
    AttributeMatchSelector, AttributeMatcher, AttributeSelector, AttributeValue, ClassSelector,
    Combinator, CombinedSelector, ComplexSelector, CompoundSelector, IdSelector,
    InvalidSelectorList, SelectorIdent, SelectorList, SelectorListParseResult, SelectorString,
    SubclassSelector, TypeSelector, UnsupportedSelectorList, parse_selector_list,
    parse_selector_list_with_limits,
};
use crate::syntax::{
    CssInput, CssRule, CssSpan, ParseOptions, SyntaxLimits, parse_stylesheet_with_options,
};

pub(super) fn span(input: &CssInput, start: usize, end: usize) -> CssSpan {
    input.span(start, end).expect("valid span")
}

pub(super) fn ident(input: &CssInput, start: usize, end: usize, text: &str) -> SelectorIdent {
    SelectorIdent::new(text, Some(span(input, start, end))).expect("selector ident")
}

pub(super) fn string(input: &CssInput, start: usize, end: usize, value: &str) -> SelectorString {
    SelectorString::new(value, Some(span(input, start, end)))
}

pub(super) fn sample_selector_list(input: &CssInput) -> SelectorList {
    let head = CompoundSelector::new(
        span(input, 0, 12),
        Some(
            TypeSelector::named(span(input, 0, 7), ident(input, 0, 7, "article"))
                .expect("named type selector"),
        ),
        vec![SubclassSelector::Class(
            ClassSelector::new(span(input, 7, 12), ident(input, 8, 12, "card"))
                .expect("class selector"),
        )],
    )
    .expect("head compound");

    let tail_compound = CompoundSelector::new(
        span(input, 15, 41),
        Some(
            TypeSelector::named(span(input, 15, 17), ident(input, 15, 17, "h1"))
                .expect("tail named type selector"),
        ),
        vec![
            SubclassSelector::Id(
                IdSelector::new(span(input, 17, 22), ident(input, 18, 22, "hero"))
                    .expect("id selector"),
            ),
            SubclassSelector::Attribute(AttributeSelector::Match(
                AttributeMatchSelector::new(
                    span(input, 22, 41),
                    ident(input, 23, 32, "data-kind"),
                    AttributeMatcher::Exact,
                    AttributeValue::string(string(input, 33, 40, "promo")),
                )
                .expect("attribute selector"),
            )),
        ],
    )
    .expect("tail compound");

    SelectorList::new(
        Some(span(input, 0, 41)),
        vec![
            ComplexSelector::new(
                span(input, 0, 41),
                head,
                vec![
                    CombinedSelector::new(span(input, 13, 41), Combinator::Child, tail_compound)
                        .expect("combined selector"),
                ],
            )
            .expect("complex selector"),
        ],
    )
    .expect("selector list")
}

pub(super) fn parse_selector_result(source: &str) -> SelectorListParseResult {
    let stylesheet = format!("{source} {{ color: red; }}");
    let parse = parse_stylesheet_with_options(&stylesheet, &ParseOptions::stylesheet());
    let rule = parse.stylesheet.rules.first().expect("style rule");
    let CssRule::Qualified(rule) = rule else {
        panic!("expected qualified rule");
    };
    parse_selector_list(&parse.input, &rule.prelude)
}

pub(super) fn parse_selector_result_with_limits(
    source: &str,
    limits: &SyntaxLimits,
) -> SelectorListParseResult {
    let stylesheet = format!("{source} {{ color: red; }}");
    let parse = parse_stylesheet_with_options(&stylesheet, &ParseOptions::stylesheet());
    let rule = parse.stylesheet.rules.first().expect("style rule");
    let CssRule::Qualified(rule) = rule else {
        panic!("expected qualified rule");
    };
    parse_selector_list_with_limits(&parse.input, &rule.prelude, limits)
}

pub(super) fn parsed_selector_list(source: &str) -> SelectorList {
    let result = parse_selector_result(source);
    let Some(list) = result.parsed() else {
        panic!("expected parsed selector list for {source:?}");
    };
    list.clone()
}

pub(super) fn invalid_selector(source: &str) -> InvalidSelectorList {
    let result = parse_selector_result(source);
    let Some(list) = result.invalid() else {
        panic!("expected invalid selector parse result for {source:?}");
    };
    list.clone()
}

pub(super) fn unsupported_selector(source: &str) -> UnsupportedSelectorList {
    let result = parse_selector_result(source);
    let Some(list) = result.unsupported() else {
        panic!("expected unsupported selector parse result for {source:?}");
    };
    list.clone()
}

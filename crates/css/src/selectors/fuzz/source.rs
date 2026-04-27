use crate::selectors::{
    InvalidSelectorList, InvalidSelectorReason, SelectorListParseResult,
    parse_selector_list_with_limits,
};
use crate::syntax::{CssRule, ParseOptions, SyntaxLimits, parse_stylesheet_with_options};

pub(super) fn parse_selector_source(
    source: &str,
    limits: &SyntaxLimits,
) -> SelectorListParseResult {
    let stylesheet_source = format!("{source} {{ color: red; }}");
    let parse_options = ParseOptions {
        limits: limits.clone(),
        ..ParseOptions::stylesheet()
    };

    let parse = parse_stylesheet_with_options(&stylesheet_source, &parse_options);

    let Some(rule) = parse.stylesheet.rules.first() else {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            None,
            InvalidSelectorReason::EmptySelectorList,
        ));
    };

    let CssRule::Qualified(rule) = rule else {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            None,
            InvalidSelectorReason::EmptySelectorList,
        ));
    };

    parse_selector_list_with_limits(&parse.input, &rule.prelude, limits)
}

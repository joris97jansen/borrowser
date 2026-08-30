use super::{
    InvalidSelectorList, InvalidSelectorReason, SelectorListParseResult,
    parse_selector_list_with_limits,
};
use crate::syntax::{ParseOptions, SyntaxLimits, parse_component_value_list_structured};

/// Parses an authored selector list without fabricating a stylesheet rule.
///
/// This is the canonical source-text carrier for selector-only consumers. It
/// shares the syntax component-value parser and authoritative selector parser
/// used by model stylesheet parsing, while invoking neither stylesheet-rule
/// parsing nor any DOM/cascade phase.
pub fn parse_selector_source_with_limits(
    source: &str,
    limits: &SyntaxLimits,
) -> SelectorListParseResult {
    let options = ParseOptions {
        limits: limits.clone(),
        ..ParseOptions::stylesheet()
    };
    let (input, values, stats) = parse_component_value_list_structured(source, &options);
    if stats.hit_limit {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            input.span(0, input.len_bytes()),
            InvalidSelectorReason::ResourceLimitExceeded,
        ));
    }
    parse_selector_list_with_limits(&input, &values, limits)
}

pub fn parse_selector_source(source: &str) -> SelectorListParseResult {
    parse_selector_source_with_limits(source, &SyntaxLimits::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_selector_source_never_requires_or_fabricates_a_stylesheet_rule() {
        let parsed = parse_selector_source("article.featured, #hero");
        let SelectorListParseResult::Parsed(list) = parsed else {
            panic!("selector list")
        };
        assert_eq!(list.selectors().len(), 2);
        assert_eq!(
            list.selectors()[0].specificity(),
            crate::Specificity::new(0, 1, 1)
        );
        assert_eq!(
            list.selectors()[1].specificity(),
            crate::Specificity::new(1, 0, 0)
        );

        assert!(matches!(
            parse_selector_source("a:hover"),
            SelectorListParseResult::Unsupported(_)
        ));
        assert!(matches!(
            parse_selector_source("a,,b"),
            SelectorListParseResult::Invalid(_)
        ));
    }

    #[test]
    fn direct_selector_source_reports_resource_exhaustion_as_typed_invalidity() {
        let limits = SyntaxLimits {
            max_selectors_per_rule: 1,
            ..SyntaxLimits::default()
        };
        assert!(matches!(
            parse_selector_source_with_limits("a, b", &limits),
            SelectorListParseResult::Invalid(ref invalid)
                if invalid.reason() == InvalidSelectorReason::ResourceLimitExceeded
        ));
    }
}

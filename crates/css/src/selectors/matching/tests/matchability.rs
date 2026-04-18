use super::super::SelectorMatchability;
use super::support::parsed_div_selector_result;
use crate::selectors::{InvalidSelectorReason, UnsupportedSelectorFeature};

#[test]
fn parse_results_expose_matchability_without_collapsing_invalidity() {
    let parsed = parsed_div_selector_result();
    let unsupported = crate::selectors::SelectorListParseResult::Unsupported(
        crate::selectors::UnsupportedSelectorList::from_features(
            None,
            [UnsupportedSelectorFeature::PseudoClass],
        ),
    );
    let invalid = crate::selectors::SelectorListParseResult::Invalid(
        crate::selectors::InvalidSelectorList::new(None, InvalidSelectorReason::EmptySelectorList),
    );

    assert_eq!(parsed.matchability(), SelectorMatchability::Parsed);
    assert_eq!(
        unsupported.matchability(),
        SelectorMatchability::Unsupported
    );
    assert_eq!(invalid.matchability(), SelectorMatchability::Invalid);
}

#[test]
fn selector_matchability_helpers_expose_explicit_states() {
    assert!(SelectorMatchability::Parsed.is_parsed());
    assert!(!SelectorMatchability::Parsed.is_unsupported());
    assert!(!SelectorMatchability::Parsed.is_invalid());

    assert!(SelectorMatchability::Unsupported.is_unsupported());
    assert!(!SelectorMatchability::Unsupported.is_parsed());
    assert!(!SelectorMatchability::Unsupported.is_invalid());

    assert!(SelectorMatchability::Invalid.is_invalid());
    assert!(!SelectorMatchability::Invalid.is_parsed());
    assert!(!SelectorMatchability::Invalid.is_unsupported());
}

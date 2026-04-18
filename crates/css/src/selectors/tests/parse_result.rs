use super::super::{
    InvalidSelectorList, InvalidSelectorReason, SelectorListParseResult,
    UnsupportedSelectorFeature, UnsupportedSelectorHandling, UnsupportedSelectorList,
};
use super::support::unsupported_selector;
use crate::syntax::CssInput;

#[test]
fn parse_result_states_are_explicit_and_snapshot_stable() {
    let unsupported_input = CssInput::from(":hover");
    let unsupported = SelectorListParseResult::Unsupported(UnsupportedSelectorList::from_features(
        unsupported_input.span(0, 6),
        [
            UnsupportedSelectorFeature::PseudoClass,
            UnsupportedSelectorFeature::ForgivingSelectorList,
            UnsupportedSelectorFeature::PseudoClass,
        ],
    ));
    assert!(unsupported.parsed().is_none());
    assert!(unsupported.unsupported().is_some());
    assert!(unsupported.invalid().is_none());
    assert_eq!(
        unsupported
            .unsupported()
            .expect("unsupported result")
            .handling(),
        UnsupportedSelectorHandling::PreserveAsUnsupported
    );
    assert_eq!(
        unsupported.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..6\n",
            "feature[0]: pseudo-class\n",
            "feature[1]: forgiving-selector-list\n",
        )
    );

    let invalid_input = CssInput::from("> div");
    let invalid = SelectorListParseResult::Invalid(InvalidSelectorList::new(
        invalid_input.span(0, 1),
        InvalidSelectorReason::LeadingCombinator,
    ));
    assert!(invalid.parsed().is_none());
    assert!(invalid.unsupported().is_none());
    assert!(invalid.invalid().is_some());
    assert_eq!(
        invalid.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..1\n",
            "reason: leading-combinator\n",
        )
    );
}

#[test]
fn unsupported_feature_lists_are_deduplicated_in_first_encounter_order() {
    let input = CssInput::from(":hover:focus");
    let list = UnsupportedSelectorList::from_features(
        input.span(0, 12),
        [
            UnsupportedSelectorFeature::PseudoClass,
            UnsupportedSelectorFeature::FunctionalPseudoClass,
            UnsupportedSelectorFeature::PseudoClass,
            UnsupportedSelectorFeature::PseudoElement,
            UnsupportedSelectorFeature::FunctionalPseudoClass,
        ],
    );

    assert_eq!(
        list.features(),
        &[
            UnsupportedSelectorFeature::PseudoClass,
            UnsupportedSelectorFeature::FunctionalPseudoClass,
            UnsupportedSelectorFeature::PseudoElement,
        ]
    );
}

#[test]
fn unsupported_selector_lists_expose_explicit_handling_strategy() {
    let list = unsupported_selector("a:is(.x)");

    assert_eq!(
        list.handling(),
        UnsupportedSelectorHandling::PreserveAsUnsupported
    );
}

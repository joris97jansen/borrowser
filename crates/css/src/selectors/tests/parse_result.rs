use super::super::diagnostics::{
    SelectorDiagnosticClass, SelectorDiagnosticDetail, SelectorDiagnosticLevel,
};
use super::super::{
    InvalidSelectorList, InvalidSelectorReason, SelectorListParseResult,
    UnsupportedSelectorFeature, UnsupportedSelectorHandling, UnsupportedSelectorList,
};
use super::support::unsupported_selector;
use crate::syntax::CssInput;

#[test]
fn parse_result_states_are_explicit_and_snapshot_stable() {
    let unsupported_input = CssInput::from(":hover");
    let unsupported = SelectorListParseResult::Unsupported(
        UnsupportedSelectorList::from_features(
            unsupported_input.span(0, 6),
            [
                UnsupportedSelectorFeature::PseudoClass,
                UnsupportedSelectorFeature::ForgivingSelectorList,
                UnsupportedSelectorFeature::PseudoClass,
            ],
        )
        .expect("unsupported feature list must be non-empty"),
    );
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
    )
    .expect("unsupported feature list must be non-empty");

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
fn unsupported_feature_lists_cannot_be_constructed_empty() {
    assert!(UnsupportedSelectorList::from_features(None, []).is_none());
}

#[test]
fn unsupported_selector_lists_expose_explicit_handling_strategy() {
    let list = unsupported_selector("a:is(.x)");

    assert_eq!(
        list.handling(),
        UnsupportedSelectorHandling::PreserveAsUnsupported
    );
}

#[test]
fn selector_diagnostics_normalize_invalid_reasons_at_the_selector_boundary() {
    let cases = [
        (
            InvalidSelectorReason::EmptySelectorList,
            SelectorDiagnosticClass::EmptySelectorList,
            SelectorDiagnosticLevel::Error,
        ),
        (
            InvalidSelectorReason::ResourceLimitExceeded,
            SelectorDiagnosticClass::LimitExceeded,
            SelectorDiagnosticLevel::Error,
        ),
        (
            InvalidSelectorReason::InvariantViolation,
            SelectorDiagnosticClass::InvariantViolation,
            SelectorDiagnosticLevel::Error,
        ),
        (
            InvalidSelectorReason::MissingAttributeValue,
            SelectorDiagnosticClass::InvalidSelector,
            SelectorDiagnosticLevel::Error,
        ),
    ];

    for (reason, expected_class, expected_level) in cases {
        let result = SelectorListParseResult::Invalid(InvalidSelectorList::new(None, reason));
        let diagnostic = result.diagnostic().expect("invalid selector diagnostic");

        assert_eq!(diagnostic.class(), expected_class);
        assert_eq!(diagnostic.level(), expected_level);
        assert!(match (reason, diagnostic.detail()) {
            (
                InvalidSelectorReason::EmptySelectorList,
                SelectorDiagnosticDetail::EmptySelectorList,
            ) => true,
            (reason, SelectorDiagnosticDetail::Invalid(actual)) => *actual == reason,
            (
                InvalidSelectorReason::InvariantViolation,
                SelectorDiagnosticDetail::InvariantViolation,
            ) => true,
            (
                InvalidSelectorReason::ResourceLimitExceeded,
                SelectorDiagnosticDetail::LimitExceeded,
            ) => true,
            _ => false,
        });
    }
}

#[test]
fn selector_diagnostics_normalize_unsupported_features_at_the_selector_boundary() {
    let result = SelectorListParseResult::Unsupported(
        UnsupportedSelectorList::from_features(
            None,
            [
                UnsupportedSelectorFeature::FunctionalPseudoClass,
                UnsupportedSelectorFeature::ForgivingSelectorList,
            ],
        )
        .expect("unsupported feature list must be non-empty"),
    );
    let diagnostic = result
        .diagnostic()
        .expect("unsupported selector diagnostic");

    assert_eq!(
        diagnostic.class(),
        SelectorDiagnosticClass::UnsupportedSelector
    );
    assert_eq!(diagnostic.level(), SelectorDiagnosticLevel::Warning);
    assert!(matches!(
        diagnostic.detail(),
        SelectorDiagnosticDetail::Unsupported(features)
            if *features == [
                UnsupportedSelectorFeature::FunctionalPseudoClass,
                UnsupportedSelectorFeature::ForgivingSelectorList,
            ]
    ));
}

#[test]
fn selector_diagnostic_messages_use_canonical_selector_labels() {
    let invalid = SelectorListParseResult::Invalid(InvalidSelectorList::new(
        None,
        InvalidSelectorReason::MissingAttributeValue,
    ));
    assert_eq!(
        invalid
            .diagnostic()
            .expect("invalid selector diagnostic")
            .stable_message(),
        "invalid selector: missing-attribute-value"
    );

    let unsupported = SelectorListParseResult::Unsupported(
        UnsupportedSelectorList::from_features(
            None,
            [UnsupportedSelectorFeature::FunctionalPseudoClass],
        )
        .expect("unsupported feature list must be non-empty"),
    );
    assert_eq!(
        unsupported
            .diagnostic()
            .expect("unsupported selector diagnostic")
            .stable_message(),
        "unsupported selector feature(s): functional-pseudo-class"
    );

    let invariant = SelectorListParseResult::Invalid(InvalidSelectorList::new(
        None,
        InvalidSelectorReason::InvariantViolation,
    ));
    assert_eq!(
        invariant
            .diagnostic()
            .expect("invariant selector diagnostic")
            .stable_message(),
        "selector invariant violation: invariant-violation"
    );

    let resource_limit = SelectorListParseResult::Invalid(InvalidSelectorList::new(
        None,
        InvalidSelectorReason::ResourceLimitExceeded,
    ));
    assert_eq!(
        resource_limit
            .diagnostic()
            .expect("resource-limit selector diagnostic")
            .stable_message(),
        "selector resource limit exceeded: resource-limit-exceeded"
    );
}

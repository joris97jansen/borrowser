use super::{
    AttributeMatchSelector, AttributeMatcher, AttributeSelector, AttributeValue, ClassSelector,
    Combinator, CombinedSelector, ComplexSelector, CompoundSelector, IdSelector,
    InvalidSelectorList, InvalidSelectorReason, NamedTypeSelector, SelectorIdent, SelectorList,
    SelectorListParseResult, SelectorString, Specificity, SubclassSelector, TypeSelector,
    UnsupportedSelectorFeature, UnsupportedSelectorList,
};
use crate::syntax::CssInput;

fn sample_selector_list(input: &CssInput) -> SelectorList {
    SelectorList {
        span: input.span(0, 41),
        selectors: vec![ComplexSelector {
            span: input.span(0, 41).expect("selector span"),
            head: CompoundSelector {
                span: input.span(0, 12).expect("head compound span"),
                type_selector: Some(TypeSelector::Named(NamedTypeSelector {
                    span: input.span(0, 7).expect("type span"),
                    name: SelectorIdent {
                        span: input.span(0, 7),
                        text: "article".to_string(),
                    },
                })),
                subclasses: vec![SubclassSelector::Class(ClassSelector {
                    span: input.span(7, 12).expect("class span"),
                    name: SelectorIdent {
                        span: input.span(8, 12),
                        text: "card".to_string(),
                    },
                })],
            },
            tail: vec![CombinedSelector {
                span: input.span(12, 41).expect("combined span"),
                combinator: Combinator::Child,
                selector: CompoundSelector {
                    span: input.span(15, 41).expect("tail compound span"),
                    type_selector: Some(TypeSelector::Named(NamedTypeSelector {
                        span: input.span(15, 17).expect("tail type span"),
                        name: SelectorIdent {
                            span: input.span(15, 17),
                            text: "h1".to_string(),
                        },
                    })),
                    subclasses: vec![
                        SubclassSelector::Id(IdSelector {
                            span: input.span(17, 22).expect("id span"),
                            name: SelectorIdent {
                                span: input.span(18, 22),
                                text: "hero".to_string(),
                            },
                        }),
                        SubclassSelector::Attribute(AttributeSelector::Match(
                            AttributeMatchSelector {
                                span: input.span(22, 41).expect("attribute span"),
                                name: SelectorIdent {
                                    span: input.span(23, 32),
                                    text: "data-kind".to_string(),
                                },
                                matcher: AttributeMatcher::Exact,
                                value: AttributeValue::String(SelectorString {
                                    span: input.span(33, 40),
                                    value: "promo".to_string(),
                                }),
                            },
                        )),
                    ],
                },
            }],
        }],
    }
}

#[test]
fn selector_list_snapshot_is_stable() {
    let input = CssInput::from("article.card > h1#hero[data-kind=\"promo\"]");
    let list = sample_selector_list(&input);

    assert_eq!(
        list.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-list\n",
            "span: @0..41\n",
            "selector[0] @0..41 specificity=(1,2,2)\n",
            "  compound[0] @0..12 specificity=(0,1,1)\n",
            "    - type(\"article\") node=@0..7 name=@0..7\n",
            "    - class(\"card\") node=@7..12 name=@8..12\n",
            "  combined[0] child @12..41\n",
            "    compound @15..41 specificity=(1,1,1)\n",
            "      - type(\"h1\") node=@15..17 name=@15..17\n",
            "      - id(\"hero\") node=@17..22 name=@18..22\n",
            "      - attribute-match(name=\"data-kind\", name_span=@23..32, matcher=exact, value=string(\"promo\", span=@33..40)) node=@22..41\n",
        )
    );
}

#[test]
fn specificity_counts_supported_selector_components() {
    let input = CssInput::from("article.card > h1#hero[data-kind=\"promo\"]");
    let list = sample_selector_list(&input);
    let selector = &list.selectors[0];

    assert_eq!(
        selector.head.specificity(),
        Specificity {
            ids: 0,
            classes: 1,
            types: 1,
        }
    );
    assert_eq!(
        selector.tail[0].selector.specificity(),
        Specificity {
            ids: 1,
            classes: 1,
            types: 1,
        }
    );
    assert_eq!(
        selector.specificity(),
        Specificity {
            ids: 1,
            classes: 2,
            types: 2,
        }
    );
}

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
    let invalid = SelectorListParseResult::Invalid(InvalidSelectorList {
        span: invalid_input.span(0, 1),
        reason: InvalidSelectorReason::LeadingCombinator,
    });
    assert!(invalid.parsed().is_none());
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

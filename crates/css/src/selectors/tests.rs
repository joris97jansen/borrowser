use super::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, ClassSelector, Combinator, CombinedSelector, ComplexSelector, CompoundSelector,
    IdSelector, InvalidSelectorList, InvalidSelectorReason, SelectorIdent, SelectorList,
    SelectorListParseResult, SelectorString, SelectorStructureError, Specificity, SubclassSelector,
    TypeSelector, UnsupportedSelectorFeature, UnsupportedSelectorList, parse_selector_list,
};
use crate::syntax::{CssInput, CssRule, CssSpan, ParseOptions, parse_stylesheet_with_options};

fn span(input: &CssInput, start: usize, end: usize) -> CssSpan {
    input.span(start, end).expect("valid span")
}

fn ident(input: &CssInput, start: usize, end: usize, text: &str) -> SelectorIdent {
    SelectorIdent::new(text, Some(span(input, start, end))).expect("selector ident")
}

fn string(input: &CssInput, start: usize, end: usize, value: &str) -> SelectorString {
    SelectorString::new(value, Some(span(input, start, end)))
}

fn sample_selector_list(input: &CssInput) -> SelectorList {
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

fn parse_selector_result(source: &str) -> SelectorListParseResult {
    let stylesheet = format!("{source} {{ color: red; }}");
    let parse = parse_stylesheet_with_options(&stylesheet, &ParseOptions::stylesheet());
    let rule = parse.stylesheet.rules.first().expect("style rule");
    let CssRule::Qualified(rule) = rule else {
        panic!("expected qualified rule");
    };
    parse_selector_list(&parse.input, &rule.prelude)
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
            "  combined[0] child @13..41\n",
            "    compound @15..41 specificity=(1,1,1)\n",
            "      - type(\"h1\") node=@15..17 name=@15..17\n",
            "      - id(\"hero\") node=@17..22 name=@18..22\n",
            "      - attribute-match(name=\"data-kind\", name_span=@23..32, matcher=exact, value=string(\"promo\", span=@33..40)) node=@22..41\n",
        )
    );
}

#[test]
fn selector_ir_construction_exposes_structure_accessors() {
    let input = CssInput::from("article.card > h1#hero[data-kind=\"promo\"]");
    let list = sample_selector_list(&input);

    assert_eq!(list.len(), 1);
    assert_eq!(list.span(), Some(span(&input, 0, 41)));

    let selector = list.iter().next().expect("selector");
    assert_eq!(selector.span(), span(&input, 0, 41));
    assert_eq!(selector.tail().len(), 1);

    let head = selector.head();
    assert_eq!(head.span(), span(&input, 0, 12));
    assert_eq!(head.subclasses().len(), 1);
    assert!(matches!(head.type_selector(), Some(TypeSelector::Named(_))));

    let combined = &selector.tail()[0];
    assert_eq!(combined.combinator(), Combinator::Child);
    assert_eq!(combined.span(), span(&input, 13, 41));
    assert_eq!(combined.selector().span(), span(&input, 15, 41));
}

#[test]
fn specificity_counts_supported_selector_components() {
    let input = CssInput::from("article.card > h1#hero[data-kind=\"promo\"]");
    let list = sample_selector_list(&input);
    let selector = list.iter().next().expect("selector");

    assert_eq!(selector.head().specificity(), Specificity::new(0, 1, 1));
    assert_eq!(
        selector.tail()[0].selector().specificity(),
        Specificity::new(1, 1, 1)
    );
    assert_eq!(selector.specificity(), Specificity::new(1, 2, 2));
    assert_eq!(selector.specificity().ids(), 1);
    assert_eq!(selector.specificity().classes(), 2);
    assert_eq!(selector.specificity().types(), 2);
}

#[test]
fn specificity_is_exposed_for_supported_simple_selector_kinds() {
    let universal_input = CssInput::from("*");
    let universal = TypeSelector::universal(span(&universal_input, 0, 1));
    assert_eq!(universal.specificity(), Specificity::ZERO);

    let type_input = CssInput::from("article");
    let named = TypeSelector::named(span(&type_input, 0, 7), ident(&type_input, 0, 7, "article"))
        .expect("named type selector");
    assert_eq!(named.specificity(), Specificity::TYPE);

    let id_input = CssInput::from("#hero");
    let id = IdSelector::new(span(&id_input, 0, 5), ident(&id_input, 1, 5, "hero"))
        .expect("id selector");
    assert_eq!(id.specificity(), Specificity::ID);
    assert_eq!(
        SubclassSelector::Id(id.clone()).specificity(),
        Specificity::ID
    );

    let class_input = CssInput::from(".card");
    let class = ClassSelector::new(span(&class_input, 0, 5), ident(&class_input, 1, 5, "card"))
        .expect("class selector");
    assert_eq!(class.specificity(), Specificity::CLASS);
    assert_eq!(
        SubclassSelector::Class(class.clone()).specificity(),
        Specificity::CLASS
    );

    let exists_input = CssInput::from("[data-kind]");
    let exists = AttributeExistsSelector::new(
        span(&exists_input, 0, 11),
        ident(&exists_input, 1, 10, "data-kind"),
    )
    .expect("attribute exists selector");
    assert_eq!(exists.specificity(), Specificity::CLASS);
    assert_eq!(
        AttributeSelector::Exists(exists.clone()).specificity(),
        Specificity::CLASS
    );

    let match_input = CssInput::from("[data-kind=\"promo\"]");
    let matched = AttributeMatchSelector::new(
        span(&match_input, 0, 19),
        ident(&match_input, 1, 10, "data-kind"),
        AttributeMatcher::Exact,
        AttributeValue::string(string(&match_input, 11, 18, "promo")),
    )
    .expect("attribute match selector");
    assert_eq!(matched.specificity(), Specificity::CLASS);
    assert_eq!(
        AttributeSelector::Match(matched).specificity(),
        Specificity::CLASS
    );
}

#[test]
fn specificity_saturates_deterministically() {
    let saturated = Specificity::new(u16::MAX, u16::MAX - 1, u16::MAX) + Specificity::new(1, 2, 1);
    assert_eq!(saturated, Specificity::new(u16::MAX, u16::MAX, u16::MAX));

    let mut accum = Specificity::new(u16::MAX - 1, 0, u16::MAX - 1);
    accum += Specificity::new(5, u16::MAX, 5);
    assert_eq!(accum, Specificity::new(u16::MAX, u16::MAX, u16::MAX));
}

#[test]
fn parser_derives_specificity_from_selector_ir() {
    let result = parse_selector_result("*#hero.card[data-kind] > section.notice");
    let SelectorListParseResult::Parsed(list) = result else {
        panic!("expected supported selector list to parse");
    };

    let selector = list.iter().next().expect("parsed selector");
    assert_eq!(selector.head().specificity(), Specificity::new(1, 2, 0));
    assert_eq!(
        selector.tail()[0].selector().specificity(),
        Specificity::new(0, 1, 1)
    );
    assert_eq!(selector.specificity(), Specificity::new(1, 3, 1));
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
    let invalid = SelectorListParseResult::Invalid(InvalidSelectorList::new(
        invalid_input.span(0, 1),
        InvalidSelectorReason::LeadingCombinator,
    ));
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

#[test]
fn selector_ir_rejects_empty_selector_lists_and_compounds() {
    let input = CssInput::from("div");

    assert_eq!(
        SelectorList::new(None, Vec::new()),
        Err(SelectorStructureError::EmptySelectorList)
    );
    assert_eq!(
        CompoundSelector::new(span(&input, 0, 0), None, Vec::new()),
        Err(SelectorStructureError::EmptyCompoundSelector)
    );
}

#[test]
fn selector_ir_rejects_empty_identifier_payloads() {
    assert_eq!(
        SelectorIdent::new("", None),
        Err(SelectorStructureError::EmptyIdentifier)
    );
}

#[test]
fn selector_ir_rejects_payload_spans_outside_node_spans() {
    let input = CssInput::from("div.class");

    let err = ClassSelector::new(span(&input, 3, 9), ident(&input, 0, 3, "div"))
        .expect_err("payload span should be rejected");
    assert_eq!(err, SelectorStructureError::PayloadSpanOutsideNode);
}

#[test]
fn selector_ir_rejects_non_monotonic_compound_and_selector_spans() {
    let input = CssInput::from("div#id.class");
    let id = SubclassSelector::Id(
        IdSelector::new(span(&input, 3, 6), ident(&input, 4, 6, "id")).expect("id selector"),
    );
    let class = SubclassSelector::Class(
        ClassSelector::new(span(&input, 6, 12), ident(&input, 7, 12, "class"))
            .expect("class selector"),
    );

    let compound = CompoundSelector::new(
        span(&input, 0, 12),
        Some(TypeSelector::named(span(&input, 0, 3), ident(&input, 0, 3, "div")).expect("type")),
        vec![class.clone(), id.clone()],
    );
    assert_eq!(compound, Err(SelectorStructureError::NonMonotonicSpans));

    let input = CssInput::from("a b c");
    let head = CompoundSelector::new(
        span(&input, 0, 1),
        Some(TypeSelector::named(span(&input, 0, 1), ident(&input, 0, 1, "a")).expect("type")),
        vec![],
    )
    .expect("head");
    let first = CombinedSelector::new(
        span(&input, 1, 3),
        Combinator::Descendant,
        CompoundSelector::new(
            span(&input, 2, 3),
            Some(TypeSelector::named(span(&input, 2, 3), ident(&input, 2, 3, "b")).expect("b")),
            vec![],
        )
        .expect("first compound"),
    )
    .expect("first combined");
    let second = CombinedSelector::new(
        span(&input, 3, 5),
        Combinator::Descendant,
        CompoundSelector::new(
            span(&input, 4, 5),
            Some(TypeSelector::named(span(&input, 4, 5), ident(&input, 4, 5, "c")).expect("c")),
            vec![],
        )
        .expect("second compound"),
    )
    .expect("second combined");

    assert_eq!(
        ComplexSelector::new(span(&input, 0, 5), head, vec![second, first]),
        Err(SelectorStructureError::NonMonotonicSpans)
    );
}

#[test]
fn selector_lists_preserve_order_and_all_supported_combinators() {
    let input = CssInput::from("div span, ul > li + a ~ em");

    let first = ComplexSelector::new(
        span(&input, 0, 8),
        CompoundSelector::new(
            span(&input, 0, 3),
            Some(
                TypeSelector::named(span(&input, 0, 3), ident(&input, 0, 3, "div")).expect("type"),
            ),
            vec![],
        )
        .expect("first head"),
        vec![
            CombinedSelector::new(
                span(&input, 3, 8),
                Combinator::Descendant,
                CompoundSelector::new(
                    span(&input, 4, 8),
                    Some(
                        TypeSelector::named(span(&input, 4, 8), ident(&input, 4, 8, "span"))
                            .expect("descendant type"),
                    ),
                    vec![],
                )
                .expect("descendant compound"),
            )
            .expect("descendant combined"),
        ],
    )
    .expect("first selector");

    let second = ComplexSelector::new(
        span(&input, 10, 25),
        CompoundSelector::new(
            span(&input, 10, 12),
            Some(
                TypeSelector::named(span(&input, 10, 12), ident(&input, 10, 12, "ul")).expect("ul"),
            ),
            vec![],
        )
        .expect("second head"),
        vec![
            CombinedSelector::new(
                span(&input, 12, 17),
                Combinator::Child,
                CompoundSelector::new(
                    span(&input, 15, 17),
                    Some(
                        TypeSelector::named(span(&input, 15, 17), ident(&input, 15, 17, "li"))
                            .expect("li"),
                    ),
                    vec![],
                )
                .expect("li compound"),
            )
            .expect("child combined"),
            CombinedSelector::new(
                span(&input, 17, 21),
                Combinator::NextSibling,
                CompoundSelector::new(
                    span(&input, 20, 21),
                    Some(
                        TypeSelector::named(span(&input, 20, 21), ident(&input, 20, 21, "a"))
                            .expect("a"),
                    ),
                    vec![],
                )
                .expect("a compound"),
            )
            .expect("next sibling combined"),
            CombinedSelector::new(
                span(&input, 21, 25),
                Combinator::SubsequentSibling,
                CompoundSelector::new(
                    span(&input, 24, 25),
                    Some(
                        TypeSelector::named(span(&input, 24, 25), ident(&input, 24, 25, "em"))
                            .expect("em"),
                    ),
                    vec![],
                )
                .expect("em compound"),
            )
            .expect("subsequent sibling combined"),
        ],
    )
    .expect("second selector");

    let list = SelectorList::new(Some(span(&input, 0, 25)), vec![first, second]).expect("list");

    assert_eq!(list.len(), 2);
    assert_eq!(
        list.selectors()[0].tail()[0].combinator(),
        Combinator::Descendant
    );
    assert_eq!(
        list.selectors()[1].tail()[0].combinator(),
        Combinator::Child
    );
    assert_eq!(
        list.selectors()[1].tail()[1].combinator(),
        Combinator::NextSibling
    );
    assert_eq!(
        list.selectors()[1].tail()[2].combinator(),
        Combinator::SubsequentSibling
    );
}

#[test]
fn attribute_selectors_cover_exists_and_match_forms() {
    let input = CssInput::from("[data-kind][lang|=\"en\"]");
    let exists = AttributeSelector::Exists(
        AttributeExistsSelector::new(span(&input, 0, 11), ident(&input, 1, 10, "data-kind"))
            .expect("exists selector"),
    );
    let matcher = AttributeSelector::Match(
        AttributeMatchSelector::new(
            span(&input, 11, 23),
            ident(&input, 12, 16, "lang"),
            AttributeMatcher::DashMatch,
            AttributeValue::string(string(&input, 18, 22, "en")),
        )
        .expect("match selector"),
    );

    assert_eq!(exists.span(), span(&input, 0, 11));
    assert_eq!(matcher.span(), span(&input, 11, 23));
}

#[test]
fn parser_builds_ir_for_supported_selector_lists() {
    let result = parse_selector_result("article.card > h1#hero[data-kind=\"promo\"], aside");

    assert_eq!(
        result.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: parsed\n",
            "span: @0..49\n",
            "selector[0] @0..41 specificity=(1,2,2)\n",
            "  compound[0] @0..12 specificity=(0,1,1)\n",
            "    - type(\"article\") node=@0..7 name=@0..7\n",
            "    - class(\"card\") node=@7..12 name=@8..12\n",
            "  combined[0] child @13..41\n",
            "    compound @15..41 specificity=(1,1,1)\n",
            "      - type(\"h1\") node=@15..17 name=@15..17\n",
            "      - id(\"hero\") node=@17..22 name=@18..22\n",
            "      - attribute-match(name=\"data-kind\", name_span=@23..32, matcher=exact, value=string(\"promo\", span=@34..39)) node=@22..41\n",
            "selector[1] @43..48 specificity=(0,0,1)\n",
            "  compound[0] @43..48 specificity=(0,0,1)\n",
            "    - type(\"aside\") node=@43..48 name=@43..48\n",
        )
    );
}

#[test]
fn parser_distinguishes_comments_from_descendant_whitespace() {
    let compact = parse_selector_result("div/**/.card");
    let descendant = parse_selector_result("div /* gap */ .card");

    let SelectorListParseResult::Parsed(compact) = compact else {
        panic!("expected compact selector to parse");
    };
    let compact_selector = compact.iter().next().expect("compact selector");
    assert!(compact_selector.tail().is_empty());
    assert_eq!(compact_selector.head().subclasses().len(), 1);

    let SelectorListParseResult::Parsed(descendant) = descendant else {
        panic!("expected descendant selector to parse");
    };
    let descendant_selector = descendant.iter().next().expect("descendant selector");
    assert_eq!(descendant_selector.tail().len(), 1);
    assert_eq!(
        descendant_selector.tail()[0].combinator(),
        Combinator::Descendant
    );
}

#[test]
fn parser_reports_invalid_selector_shapes_deterministically() {
    let leading = parse_selector_result("> div");
    let trailing = parse_selector_result("div >");
    let repeated = parse_selector_result("div > + span");

    assert_eq!(
        leading.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..1\n",
            "reason: leading-combinator\n",
        )
    );
    assert_eq!(
        trailing.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @4..5\n",
            "reason: trailing-combinator\n",
        )
    );
    assert_eq!(
        repeated.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @6..7\n",
            "reason: repeated-combinator\n",
        )
    );
}

#[test]
fn parser_reports_unsupported_selector_features_without_string_splitting() {
    let pseudo = parse_selector_result("a:is(.x, .y)");
    let namespace = parse_selector_result("svg|a");
    let case_modifier = parse_selector_result("[lang=\"en\" i]");

    assert_eq!(
        pseudo.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..13\n",
            "feature[0]: functional-pseudo-class\n",
            "feature[1]: forgiving-selector-list\n",
        )
    );
    assert_eq!(
        namespace.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..6\n",
            "feature[0]: namespace\n",
        )
    );
    assert_eq!(
        case_modifier.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..14\n",
            "feature[0]: attribute-case-modifier\n",
        )
    );
}

#[test]
fn parser_applies_selector_list_failure_policy_deterministically() {
    let unsupported = parse_selector_result("div, a:is(.x)");
    let invalid = parse_selector_result("a:is(.x), > span");

    assert_eq!(
        unsupported.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..14\n",
            "feature[0]: functional-pseudo-class\n",
            "feature[1]: forgiving-selector-list\n",
        )
    );
    assert_eq!(
        invalid.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @10..11\n",
            "reason: leading-combinator\n",
        )
    );
}

#[test]
fn parser_rejects_empty_selector_list_segments() {
    let trailing = parse_selector_result("div,");
    let leading = parse_selector_result(",div");
    let repeated = parse_selector_result("div,,span");

    assert_eq!(
        trailing.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @4..5\n",
            "reason: empty-compound-selector\n",
        )
    );
    assert_eq!(
        leading.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..5\n",
            "reason: empty-compound-selector\n",
        )
    );
    assert_eq!(
        repeated.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..10\n",
            "reason: empty-compound-selector\n",
        )
    );
}

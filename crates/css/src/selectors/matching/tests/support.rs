use super::super::SelectorDomIndex;
use crate::selectors::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, ClassSelector, ComplexSelector, CompoundSelector, IdSelector, SelectorIdent,
    SelectorList, SelectorListParseResult, SelectorString, TypeSelector,
};
use crate::syntax::{CssInput, CssRule, CssSpan, ParseOptions, parse_stylesheet_with_options};
use html::Node;
use std::sync::Arc;

pub(super) fn doc(children: Vec<Node>) -> Node {
    Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children,
    }
}

pub(super) fn element(
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    Node::Element {
        id: html::internal::Id::INVALID,
        name: Arc::<str>::from(name),
        attributes: attributes
            .into_iter()
            .map(|(name, value)| (Arc::<str>::from(name), value.map(str::to_string)))
            .collect(),
        style: Vec::new(),
        children,
    }
}

pub(super) fn text(value: &str) -> Node {
    Node::Text {
        id: html::internal::Id::INVALID,
        text: value.to_string(),
    }
}

pub(super) fn comment(value: &str) -> Node {
    Node::Comment {
        id: html::internal::Id::INVALID,
        text: value.to_string(),
    }
}

pub(super) fn parsed_div_selector_result() -> SelectorListParseResult {
    let input = CssInput::from("div");
    let span = input.span(0, 3).expect("span");
    let named = TypeSelector::named(
        span,
        SelectorIdent::new("div", Some(span)).expect("selector ident"),
    )
    .expect("named type selector");
    let compound = CompoundSelector::new(span, Some(named), Vec::new()).expect("compound selector");
    let complex = ComplexSelector::new(span, compound, Vec::new()).expect("complex selector");
    let list = SelectorList::new(Some(span), vec![complex]).expect("selector list");
    SelectorListParseResult::Parsed(list)
}

pub(super) fn parse_selector_result(source: &str) -> SelectorListParseResult {
    let stylesheet = format!("{source} {{ color: red; }}");
    let parse = parse_stylesheet_with_options(&stylesheet, &ParseOptions::stylesheet());
    let rule = parse.stylesheet.rules.first().expect("style rule");
    let CssRule::Qualified(rule) = rule else {
        panic!("expected qualified rule");
    };
    crate::selectors::parse_selector_list(&parse.input, &rule.prelude)
}

pub(super) fn assert_matching_debug_snapshot(dom: Node, selector_source: &str, expected: &str) {
    let index = SelectorDomIndex::from_root(&dom);
    let selectors = parse_selector_result(selector_source);

    assert_eq!(index.to_matching_debug_snapshot(&selectors), expected);
}

pub(super) fn parsed_single_selector(source: &str) -> ComplexSelector {
    parse_selector_result(source)
        .parsed()
        .expect("parsed selector list")
        .selectors()
        .first()
        .expect("selector entry")
        .clone()
}

pub(super) fn dummy_span(marker: &str) -> CssSpan {
    let input = CssInput::from(marker);
    input.span(0, marker.len()).expect("dummy span")
}

pub(super) fn selector_ident(text: &str) -> SelectorIdent {
    SelectorIdent::new(text, None).expect("selector ident")
}

pub(super) fn selector_string(value: &str) -> SelectorString {
    SelectorString::new(value, None)
}

pub(super) fn universal_type_selector() -> TypeSelector {
    TypeSelector::universal(dummy_span("*"))
}

pub(super) fn named_type_selector(name: &str) -> TypeSelector {
    TypeSelector::named(dummy_span("t"), selector_ident(name)).expect("named type selector")
}

pub(super) fn id_selector(name: &str) -> IdSelector {
    IdSelector::new(dummy_span("#"), selector_ident(name)).expect("id selector")
}

pub(super) fn class_selector(name: &str) -> ClassSelector {
    ClassSelector::new(dummy_span("."), selector_ident(name)).expect("class selector")
}

pub(super) fn attribute_exists_selector(name: &str) -> AttributeSelector {
    AttributeSelector::Exists(
        AttributeExistsSelector::new(dummy_span("[]"), selector_ident(name))
            .expect("attribute exists selector"),
    )
}

pub(super) fn attribute_match_selector(
    name: &str,
    matcher: AttributeMatcher,
    value: AttributeValue,
) -> AttributeSelector {
    AttributeSelector::Match(
        AttributeMatchSelector::new(dummy_span("[]"), selector_ident(name), matcher, value)
            .expect("attribute match selector"),
    )
}

pub(super) fn ident_value(value: &str) -> AttributeValue {
    AttributeValue::ident(selector_ident(value))
}

pub(super) fn string_value(value: &str) -> AttributeValue {
    AttributeValue::string(selector_string(value))
}

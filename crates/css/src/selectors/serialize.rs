use super::{
    AttributeMatcher, AttributeSelector, AttributeValue, Combinator, ComplexSelector,
    InvalidSelectorReason, SelectorList, SelectorListParseResult, Specificity, SubclassSelector,
    TypeSelector, UnsupportedSelectorFeature,
};
use crate::syntax::CssSpan;
use std::fmt::Write;

const SNAPSHOT_VERSION: u32 = 1;

pub fn serialize_selector_list_for_snapshot(list: &SelectorList) -> String {
    let mut out = String::new();
    write_snapshot_header(&mut out, "selector-list");
    write_selector_list_snapshot_body(&mut out, list, 0);
    out
}

pub fn serialize_selector_parse_result_for_snapshot(result: &SelectorListParseResult) -> String {
    let mut out = String::new();
    write_snapshot_header(&mut out, "selector-parse");
    write_selector_parse_result_snapshot_body(&mut out, result, 0);
    out
}

pub(crate) fn write_selector_list_snapshot_body(
    out: &mut String,
    list: &SelectorList,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    writeln!(out, "{indent_str}span: {}", span_label(list.span)).expect("write selector list span");
    for (selector_index, selector) in list.selectors.iter().enumerate() {
        write_selector(out, selector, selector_index, indent);
    }
}

pub(crate) fn write_selector_parse_result_snapshot_body(
    out: &mut String,
    result: &SelectorListParseResult,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    match result {
        SelectorListParseResult::Parsed(list) => {
            writeln!(out, "{indent_str}result: parsed").expect("write parsed result");
            write_selector_list_snapshot_body(out, list, indent);
        }
        SelectorListParseResult::Unsupported(list) => {
            writeln!(out, "{indent_str}result: unsupported").expect("write unsupported result");
            writeln!(out, "{indent_str}span: {}", span_label(list.span()))
                .expect("write unsupported span");
            for (feature_index, feature) in list.features().iter().enumerate() {
                writeln!(
                    out,
                    "{indent_str}feature[{feature_index}]: {}",
                    unsupported_feature_label(*feature)
                )
                .expect("write unsupported feature");
            }
        }
        SelectorListParseResult::Invalid(list) => {
            writeln!(out, "{indent_str}result: invalid").expect("write invalid result");
            writeln!(out, "{indent_str}span: {}", span_label(list.span))
                .expect("write invalid span");
            writeln!(
                out,
                "{indent_str}reason: {}",
                invalid_reason_label(list.reason)
            )
            .expect("write invalid reason");
        }
    }
}

fn write_snapshot_header(out: &mut String, kind: &str) {
    writeln!(out, "version: {SNAPSHOT_VERSION}").expect("write snapshot version");
    writeln!(out, "{kind}").expect("write snapshot kind");
}

fn write_selector(out: &mut String, selector: &ComplexSelector, index: usize, indent: usize) {
    let indent_str = " ".repeat(indent);
    writeln!(
        out,
        "{indent_str}selector[{index}] @{}..{} specificity={}",
        selector.span.start,
        selector.span.end,
        specificity_label(selector.specificity())
    )
    .expect("write selector");

    write_compound(out, &selector.head, Some(0), indent + 2);

    for (combined_index, combined) in selector.tail.iter().enumerate() {
        writeln!(
            out,
            "{}  combined[{combined_index}] {} @{}..{}",
            indent_str,
            combinator_label(combined.combinator),
            combined.span.start,
            combined.span.end
        )
        .expect("write combined selector");
        write_compound(out, &combined.selector, None, indent + 4);
    }
}

fn write_compound(
    out: &mut String,
    selector: &super::CompoundSelector,
    index: Option<usize>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    match index {
        Some(index) => writeln!(
            out,
            "{indent_str}compound[{index}] @{}..{} specificity={}",
            selector.span.start,
            selector.span.end,
            specificity_label(selector.specificity())
        ),
        None => writeln!(
            out,
            "{indent_str}compound @{}..{} specificity={}",
            selector.span.start,
            selector.span.end,
            specificity_label(selector.specificity())
        ),
    }
    .expect("write compound selector");

    if let Some(type_selector) = &selector.type_selector {
        writeln!(
            out,
            "{}  - {}",
            indent_str,
            type_selector_snapshot(type_selector)
        )
        .expect("write type selector");
    }

    for subclass in &selector.subclasses {
        writeln!(
            out,
            "{}  - {}",
            indent_str,
            subclass_selector_snapshot(subclass)
        )
        .expect("write subclass selector");
    }
}

fn type_selector_snapshot(selector: &TypeSelector) -> String {
    match selector {
        TypeSelector::Universal(selector) => {
            format!("universal(*) node={}", span_label(Some(selector.span)))
        }
        TypeSelector::Named(selector) => format!(
            "type({}) node={} name={}",
            quoted(&selector.name.text),
            span_label(Some(selector.span)),
            span_label(selector.name.span),
        ),
    }
}

fn subclass_selector_snapshot(selector: &SubclassSelector) -> String {
    match selector {
        SubclassSelector::Id(selector) => format!(
            "id({}) node={} name={}",
            quoted(&selector.name.text),
            span_label(Some(selector.span)),
            span_label(selector.name.span),
        ),
        SubclassSelector::Class(selector) => format!(
            "class({}) node={} name={}",
            quoted(&selector.name.text),
            span_label(Some(selector.span)),
            span_label(selector.name.span),
        ),
        SubclassSelector::Attribute(selector) => attribute_selector_snapshot(selector),
    }
}

fn attribute_selector_snapshot(selector: &AttributeSelector) -> String {
    match selector {
        AttributeSelector::Exists(selector) => format!(
            "attribute-exists(name={}, name_span={}) node={}",
            quoted(&selector.name.text),
            span_label(selector.name.span),
            span_label(Some(selector.span))
        ),
        AttributeSelector::Match(selector) => format!(
            "attribute-match(name={}, name_span={}, matcher={}, value={}) node={}",
            quoted(&selector.name.text),
            span_label(selector.name.span),
            attribute_matcher_label(selector.matcher),
            attribute_value_snapshot(&selector.value),
            span_label(Some(selector.span))
        ),
    }
}

fn attribute_value_snapshot(value: &AttributeValue) -> String {
    match value {
        AttributeValue::Ident(value) => {
            format!(
                "ident({}, span={})",
                quoted(&value.text),
                span_label(value.span)
            )
        }
        AttributeValue::String(value) => {
            format!(
                "string({}, span={})",
                quoted(&value.value),
                span_label(value.span)
            )
        }
    }
}

fn specificity_label(specificity: Specificity) -> String {
    format!(
        "({},{},{})",
        specificity.ids, specificity.classes, specificity.types
    )
}

fn combinator_label(combinator: Combinator) -> &'static str {
    match combinator {
        Combinator::Descendant => "descendant",
        Combinator::Child => "child",
        Combinator::NextSibling => "next-sibling",
        Combinator::SubsequentSibling => "subsequent-sibling",
    }
}

fn attribute_matcher_label(matcher: AttributeMatcher) -> &'static str {
    match matcher {
        AttributeMatcher::Exact => "exact",
        AttributeMatcher::Includes => "includes",
        AttributeMatcher::DashMatch => "dash-match",
        AttributeMatcher::Prefix => "prefix",
        AttributeMatcher::Suffix => "suffix",
        AttributeMatcher::Substring => "substring",
    }
}

fn unsupported_feature_label(feature: UnsupportedSelectorFeature) -> &'static str {
    match feature {
        UnsupportedSelectorFeature::Namespace => "namespace",
        UnsupportedSelectorFeature::AttributeCaseModifier => "attribute-case-modifier",
        UnsupportedSelectorFeature::PseudoClass => "pseudo-class",
        UnsupportedSelectorFeature::FunctionalPseudoClass => "functional-pseudo-class",
        UnsupportedSelectorFeature::PseudoElement => "pseudo-element",
        UnsupportedSelectorFeature::RelativeSelector => "relative-selector",
        UnsupportedSelectorFeature::NestingSelector => "nesting-selector",
        UnsupportedSelectorFeature::ColumnCombinator => "column-combinator",
        UnsupportedSelectorFeature::ForgivingSelectorList => "forgiving-selector-list",
    }
}

fn invalid_reason_label(reason: InvalidSelectorReason) -> &'static str {
    match reason {
        InvalidSelectorReason::EmptySelectorList => "empty-selector-list",
        InvalidSelectorReason::EmptyCompoundSelector => "empty-compound-selector",
        InvalidSelectorReason::LeadingCombinator => "leading-combinator",
        InvalidSelectorReason::TrailingCombinator => "trailing-combinator",
        InvalidSelectorReason::RepeatedCombinator => "repeated-combinator",
        InvalidSelectorReason::MultipleTypeSelectors => "multiple-type-selectors",
        InvalidSelectorReason::MissingAttributeName => "missing-attribute-name",
        InvalidSelectorReason::MissingAttributeValue => "missing-attribute-value",
        InvalidSelectorReason::UnexpectedComponentValue => "unexpected-component-value",
    }
}

fn span_label(span: Option<CssSpan>) -> String {
    match span {
        Some(span) => format!("@{}..{}", span.start, span.end),
        None => "@<none>".to_string(),
    }
}

fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

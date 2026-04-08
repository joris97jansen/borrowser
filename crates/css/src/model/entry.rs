use super::{
    AtRule, AtRuleBlock, DeclarationBlock, PreservedBlock, PreservedComponentList, Rule, StyleRule,
    Stylesheet, StylesheetParse,
};
use crate::syntax::{
    self, CssComponentValue, CssInput, CssParseOrigin, CssRule, CssTokenText, ParseOptions,
};

/// Parse a stylesheet directly into the engine-facing rule model using default
/// stylesheet parse options.
pub fn parse_stylesheet(input: &str) -> Stylesheet {
    parse_stylesheet_with_options(input, &ParseOptions::stylesheet()).stylesheet
}

/// Parse a stylesheet into the engine-facing rule model.
///
/// The model is constructed from structured syntax-layer output. This entry
/// point does not use compatibility adapters or raw-string reparsing.
pub fn parse_stylesheet_with_options(input: &str, options: &ParseOptions) -> StylesheetParse {
    let parse = syntax::parse_stylesheet_with_options(input, options);
    let stylesheet = build_stylesheet(&parse.input, &parse.stylesheet, options.origin);

    StylesheetParse {
        input: parse.input,
        stylesheet,
        diagnostics: parse.diagnostics,
        stats: parse.stats,
    }
}

fn build_stylesheet(
    input: &CssInput,
    stylesheet: &syntax::CssStylesheet,
    origin: CssParseOrigin,
) -> Stylesheet {
    Stylesheet {
        origin,
        rules: stylesheet
            .rules
            .iter()
            .map(|rule| build_rule(input, rule))
            .collect(),
    }
}

fn build_rule(input: &CssInput, rule: &CssRule) -> Rule {
    match rule {
        CssRule::Qualified(rule) => Rule::Style(StyleRule {
            span: rule.span,
            selector_source: PreservedComponentList {
                span: component_list_span(&rule.prelude),
                values: rule.prelude.clone(),
            },
            declarations: DeclarationBlock {
                span: rule.block.span,
                declarations: rule.block.declarations.clone(),
            },
        }),
        CssRule::At(rule) => Rule::At(AtRule {
            span: rule.span,
            name: canonical_name(input, &rule.name),
            prelude: PreservedComponentList {
                span: component_list_span(&rule.prelude),
                values: rule.prelude.clone(),
            },
            block: rule.block.as_ref().map(|block| {
                AtRuleBlock::Preserved(PreservedBlock {
                    span: block.span,
                    kind: block.kind,
                    values: block.value.clone(),
                })
            }),
        }),
    }
}

fn canonical_name(input: &CssInput, text: &CssTokenText) -> Option<String> {
    text.resolve(input)
        .map(|text| text.into_owned().to_ascii_lowercase())
}

fn component_list_span(values: &[CssComponentValue]) -> Option<crate::syntax::CssSpan> {
    let first = values.first()?;
    let last = values.last()?;
    let first_span = component_value_span(first);
    let last_span = component_value_span(last);

    assert_eq!(
        first_span.input_id, last_span.input_id,
        "component list span invariant violated: component values belong to different inputs"
    );
    assert!(
        last_span.end >= first_span.start,
        "component list span invariant violated: component values are not monotonic"
    );

    crate::syntax::CssSpan::new(first_span.input_id, first_span.start, last_span.end)
}

fn component_value_span(value: &CssComponentValue) -> crate::syntax::CssSpan {
    match value {
        CssComponentValue::PreservedToken(token) => token.span,
        CssComponentValue::SimpleBlock(block) => block.span,
        CssComponentValue::Function(function) => function.span,
    }
}

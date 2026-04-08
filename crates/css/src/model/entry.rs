use super::{
    AtRule, AtRuleBlock, Declaration, DeclarationBlock, DeclarationValue, ImportantAnnotation,
    PreservedBlock, PreservedComponentList, PropertyName, PropertyNameKind, Rule, StyleRule,
    Stylesheet, StylesheetParse,
};
use crate::syntax::{
    self, CssComponentValue, CssInput, CssParseOrigin, CssRule, CssTokenKind, CssTokenText,
    ParseOptions,
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
            declarations: build_declaration_block(input, &rule.block),
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

fn build_declaration_block(
    input: &CssInput,
    block: &syntax::CssDeclarationBlock,
) -> DeclarationBlock {
    DeclarationBlock {
        span: block.span,
        declarations: block
            .declarations
            .iter()
            .map(|declaration| build_declaration(input, declaration))
            .collect(),
    }
}

fn build_declaration(input: &CssInput, declaration: &syntax::CssDeclaration) -> Declaration {
    let (value_values, important) = split_important_annotation(input, &declaration.value);

    Declaration {
        span: declaration.span,
        name: build_property_name(input, &declaration.name),
        value: DeclarationValue {
            span: declaration_value_span(declaration.value_span, &value_values),
            values: value_values,
        },
        important,
    }
}

fn build_property_name(input: &CssInput, text: &CssTokenText) -> PropertyName {
    let span = token_text_span(text);

    match text.resolve(input).map(|text| text.into_owned()) {
        Some(name) if name.starts_with("--") => PropertyName {
            span,
            kind: PropertyNameKind::Custom,
            text: Some(name),
        },
        Some(name) => PropertyName {
            span,
            kind: PropertyNameKind::Standard,
            text: Some(name.to_ascii_lowercase()),
        },
        None => PropertyName {
            span,
            kind: PropertyNameKind::Invalid,
            text: None,
        },
    }
}

fn token_text_span(text: &CssTokenText) -> Option<crate::syntax::CssSpan> {
    match text {
        CssTokenText::Span(span) => Some(*span),
        CssTokenText::Owned(_) => None,
    }
}

fn split_important_annotation(
    input: &CssInput,
    values: &[CssComponentValue],
) -> (Vec<CssComponentValue>, Option<ImportantAnnotation>) {
    let Some(important_index) = last_non_trivia_index(values) else {
        return (values.to_vec(), None);
    };
    if !is_important_ident(input, &values[important_index]) {
        return (values.to_vec(), None);
    }
    let important_span = component_value_span(&values[important_index]);

    let Some(bang_index) = last_non_trivia_index(&values[..important_index]) else {
        return (values.to_vec(), None);
    };
    if !is_bang_delim(&values[bang_index]) {
        return (values.to_vec(), None);
    }
    let bang_span = component_value_span(&values[bang_index]);

    assert_eq!(
        bang_span.input_id, important_span.input_id,
        "important annotation invariant violated: annotation tokens belong to different inputs"
    );
    assert!(
        important_span.end >= bang_span.start,
        "important annotation invariant violated: annotation tokens are not monotonic"
    );

    let span = crate::syntax::CssSpan::new(bang_span.input_id, bang_span.start, important_span.end)
        .expect("important annotation span");

    (
        values[..bang_index].to_vec(),
        Some(ImportantAnnotation { span }),
    )
}

fn declaration_value_span(
    original_value_span: crate::syntax::CssSpan,
    values: &[CssComponentValue],
) -> crate::syntax::CssSpan {
    component_list_span(values).unwrap_or_else(|| {
        crate::syntax::CssSpan::new(
            original_value_span.input_id,
            original_value_span.start,
            original_value_span.start,
        )
        .expect("empty declaration value span")
    })
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

fn is_trivia(value: &CssComponentValue) -> bool {
    matches!(
        value,
        CssComponentValue::PreservedToken(token)
            if matches!(token.kind, CssTokenKind::Whitespace | CssTokenKind::Comment(_))
    )
}

fn last_non_trivia_index(values: &[CssComponentValue]) -> Option<usize> {
    values.iter().rposition(|value| !is_trivia(value))
}

fn is_important_ident(input: &CssInput, value: &CssComponentValue) -> bool {
    match value {
        CssComponentValue::PreservedToken(token) => match &token.kind {
            CssTokenKind::Ident(text) => text
                .resolve(input)
                .map(|name| name.eq_ignore_ascii_case("important"))
                .unwrap_or(false),
            _ => false,
        },
        _ => false,
    }
}

fn is_bang_delim(value: &CssComponentValue) -> bool {
    matches!(
        value,
        CssComponentValue::PreservedToken(token)
            if matches!(token.kind, CssTokenKind::Delim('!'))
    )
}

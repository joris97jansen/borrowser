use super::{
    AtRule, AtRuleBlock, Declaration, DeclarationBlock, DeclarationListParse, DeclarationValue,
    ImportantAnnotation, PreservedBlock, PreservedComponentList, PropertyName, PropertyNameKind,
    Rule, StyleRule, Stylesheet, StylesheetParse, ValueBlock, ValueComponent, ValueFunction,
    ValueSymbol, ValueText, ValueToken,
};
use crate::selectors::parse_selector_list_with_limits;
use crate::syntax::{
    self, CssComponentValue, CssInput, CssParseOrigin, CssRule, CssToken, CssTokenKind,
    CssTokenText, ParseOptions,
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
    let stylesheet = build_stylesheet(&parse.input, &parse.stylesheet, options.origin, options);

    StylesheetParse {
        input: parse.input,
        stylesheet,
        diagnostics: parse.diagnostics,
        stats: parse.stats,
    }
}

pub(crate) fn parse_declaration_list_with_options(
    input: &str,
    options: &ParseOptions,
) -> DeclarationListParse {
    let syntax::StructuredDeclarationListParse {
        input,
        declarations,
        diagnostics,
        stats,
    } = syntax::parse_declaration_list_structured(input, 0, options);
    let declarations = declarations
        .iter()
        .map(|declaration| build_declaration(&input, declaration))
        .collect();

    DeclarationListParse {
        input,
        declarations,
        diagnostics,
        stats,
    }
}

fn build_stylesheet(
    input: &CssInput,
    stylesheet: &syntax::CssStylesheet,
    origin: CssParseOrigin,
    options: &ParseOptions,
) -> Stylesheet {
    Stylesheet {
        origin,
        span: input.span(0, input.len_bytes()),
        rules: stylesheet
            .rules
            .iter()
            .map(|rule| build_rule(input, rule, options))
            .collect(),
    }
}

fn build_rule(input: &CssInput, rule: &CssRule, options: &ParseOptions) -> Rule {
    match rule {
        CssRule::Qualified(rule) => Rule::Style(StyleRule {
            span: rule.span,
            selectors: parse_selector_list_with_limits(input, &rule.prelude, &options.limits),
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
            components: build_value_components(input, &value_values),
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

    if bang_span.input_id != important_span.input_id || important_span.end < bang_span.start {
        return (values.to_vec(), None);
    }

    let Some(span) =
        crate::syntax::CssSpan::new(bang_span.input_id, bang_span.start, important_span.end)
    else {
        return (values.to_vec(), None);
    };

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
        .unwrap_or(original_value_span)
    })
}

fn build_value_components(input: &CssInput, values: &[CssComponentValue]) -> Vec<ValueComponent> {
    values
        .iter()
        .filter_map(|value| build_value_component(input, value))
        .collect()
}

fn build_value_component(input: &CssInput, value: &CssComponentValue) -> Option<ValueComponent> {
    match value {
        CssComponentValue::PreservedToken(token) => {
            build_value_token(input, token).map(ValueComponent::Token)
        }
        CssComponentValue::SimpleBlock(block) => Some(ValueComponent::SimpleBlock(ValueBlock {
            span: block.span,
            kind: block.kind,
            components: build_value_components(input, &block.value),
        })),
        CssComponentValue::Function(function) => Some(ValueComponent::Function(ValueFunction {
            span: function.span,
            name: build_value_text(input, &function.name),
            components: build_value_components(input, &function.value),
        })),
    }
}

fn build_value_token(input: &CssInput, token: &CssToken) -> Option<ValueToken> {
    Some(match &token.kind {
        CssTokenKind::Whitespace => ValueToken::Whitespace { span: token.span },
        CssTokenKind::Comment(text) => ValueToken::Comment {
            span: token.span,
            text: build_value_text(input, text),
        },
        CssTokenKind::Ident(text) => ValueToken::Ident {
            span: token.span,
            text: build_value_text(input, text),
        },
        // Structured declaration values should never contain raw function
        // tokens; the syntax layer is expected to wrap them as
        // `CssComponentValue::Function`. If invariant recovery still surfaces a
        // raw function token here, normalize it into a plain ident-shaped token
        // so model conversion stays deterministic and non-panicking.
        CssTokenKind::Function(text) => ValueToken::Ident {
            span: token.span,
            text: build_value_text(input, text),
        },
        CssTokenKind::AtKeyword(text) => ValueToken::AtKeyword {
            span: token.span,
            text: build_value_text(input, text),
        },
        CssTokenKind::Hash { value, kind } => ValueToken::Hash {
            span: token.span,
            kind: *kind,
            text: build_value_text(input, value),
        },
        CssTokenKind::String(text) => ValueToken::String {
            span: token.span,
            text: build_value_text(input, text),
        },
        CssTokenKind::BadString => ValueToken::BadString { span: token.span },
        CssTokenKind::Url(text) => ValueToken::Url {
            span: token.span,
            text: build_value_text(input, text),
        },
        CssTokenKind::BadUrl => ValueToken::BadUrl { span: token.span },
        CssTokenKind::Delim(value) => ValueToken::Delim {
            span: token.span,
            value: *value,
        },
        CssTokenKind::Number(number) => ValueToken::Number {
            span: token.span,
            kind: number.kind,
            text: build_value_text(input, &number.repr),
        },
        CssTokenKind::Percentage(number) => ValueToken::Percentage {
            span: token.span,
            kind: number.kind,
            text: build_value_text(input, &number.repr),
        },
        CssTokenKind::Dimension(dimension) => ValueToken::Dimension {
            span: token.span,
            kind: dimension.number.kind,
            number: build_value_text(input, &dimension.number.repr),
            unit: build_value_text(input, &dimension.unit),
        },
        CssTokenKind::UnicodeRange(range) => ValueToken::UnicodeRange {
            span: token.span,
            range: *range,
        },
        CssTokenKind::Colon => build_symbol_token(token.span, ValueSymbol::Colon),
        CssTokenKind::Semicolon => build_symbol_token(token.span, ValueSymbol::Semicolon),
        CssTokenKind::Comma => build_symbol_token(token.span, ValueSymbol::Comma),
        CssTokenKind::LeftSquareBracket => {
            build_symbol_token(token.span, ValueSymbol::LeftSquareBracket)
        }
        CssTokenKind::RightSquareBracket => {
            build_symbol_token(token.span, ValueSymbol::RightSquareBracket)
        }
        CssTokenKind::LeftParenthesis => {
            build_symbol_token(token.span, ValueSymbol::LeftParenthesis)
        }
        CssTokenKind::RightParenthesis => {
            build_symbol_token(token.span, ValueSymbol::RightParenthesis)
        }
        CssTokenKind::LeftCurlyBracket => {
            build_symbol_token(token.span, ValueSymbol::LeftCurlyBracket)
        }
        CssTokenKind::RightCurlyBracket => {
            build_symbol_token(token.span, ValueSymbol::RightCurlyBracket)
        }
        CssTokenKind::IncludeMatch => build_symbol_token(token.span, ValueSymbol::IncludeMatch),
        CssTokenKind::DashMatch => build_symbol_token(token.span, ValueSymbol::DashMatch),
        CssTokenKind::PrefixMatch => build_symbol_token(token.span, ValueSymbol::PrefixMatch),
        CssTokenKind::SuffixMatch => build_symbol_token(token.span, ValueSymbol::SuffixMatch),
        CssTokenKind::SubstringMatch => build_symbol_token(token.span, ValueSymbol::SubstringMatch),
        CssTokenKind::Column => build_symbol_token(token.span, ValueSymbol::Column),
        CssTokenKind::Cdo => build_symbol_token(token.span, ValueSymbol::Cdo),
        CssTokenKind::Cdc => build_symbol_token(token.span, ValueSymbol::Cdc),
        CssTokenKind::Eof => return None,
    })
}

fn build_symbol_token(span: crate::syntax::CssSpan, kind: ValueSymbol) -> ValueToken {
    ValueToken::Symbol { span, kind }
}

fn build_value_text(input: &CssInput, text: &CssTokenText) -> ValueText {
    ValueText {
        span: token_text_span(text),
        text: text.resolve(input).map(|text| text.into_owned()),
    }
}

fn component_list_span(values: &[CssComponentValue]) -> Option<crate::syntax::CssSpan> {
    let first = values.first()?;
    let last = values.last()?;
    let first_span = component_value_span(first);
    let last_span = component_value_span(last);

    if first_span.input_id != last_span.input_id || last_span.end < first_span.start {
        return None;
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{CssInput, CssToken, CssTokenKind, CssTokenText};

    #[test]
    fn build_value_components_ignores_eof_sentinel_tokens() {
        let input = CssInput::from("");
        let values = vec![CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Eof,
            input.span(0, 0).expect("eof span"),
        ))];

        assert!(build_value_components(&input, &values).is_empty());
    }

    #[test]
    fn split_important_annotation_ignores_non_monotonic_annotation_spans() {
        let input = CssInput::from("!important?!");
        let values = vec![
            CssComponentValue::PreservedToken(CssToken::new(
                CssTokenKind::Delim('!'),
                input.span(11, 12).expect("bang span"),
            )),
            CssComponentValue::PreservedToken(CssToken::new(
                CssTokenKind::Ident(CssTokenText::Span(
                    input.span(1, 10).expect("important payload span"),
                )),
                input.span(1, 10).expect("important token span"),
            )),
        ];

        let (preserved, annotation) = split_important_annotation(&input, &values);

        assert!(annotation.is_none());
        assert_eq!(preserved, values);
    }
}

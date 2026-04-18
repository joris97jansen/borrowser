use crate::model::{DeclarationValue, ValueComponent, ValueSymbol, ValueText, ValueToken};

/// CSS text serialization for authored declaration values.
///
/// This module owns the cascade-local serializer used for debug surfaces and
/// specified-value handoff. It does not own debug snapshot formatting.
pub(crate) fn serialize_declaration_value_for_css(value: &DeclarationValue) -> Option<String> {
    let mut out = String::new();
    for component in &value.components {
        append_value_component(&mut out, component)?;
    }
    Some(out)
}

fn append_value_component(out: &mut String, component: &ValueComponent) -> Option<()> {
    match component {
        ValueComponent::Token(token) => append_value_token(out, token),
        ValueComponent::SimpleBlock(block) => {
            let (open, close) = match block.kind {
                crate::syntax::CssBlockKind::Curly => ('{', '}'),
                crate::syntax::CssBlockKind::Square => ('[', ']'),
                crate::syntax::CssBlockKind::Parenthesis => ('(', ')'),
            };
            out.push(open);
            for component in &block.components {
                append_value_component(out, component)?;
            }
            out.push(close);
            Some(())
        }
        ValueComponent::Function(function) => {
            out.push_str(function.name.text.as_deref()?);
            out.push('(');
            for component in &function.components {
                append_value_component(out, component)?;
            }
            out.push(')');
            Some(())
        }
    }
}

fn append_value_token(out: &mut String, token: &ValueToken) -> Option<()> {
    match token {
        ValueToken::Whitespace { .. } | ValueToken::Comment { .. } => {
            push_ascii_space(out);
            Some(())
        }
        ValueToken::Ident { text, .. } => append_text(out, text),
        ValueToken::AtKeyword { text, .. } => {
            out.push('@');
            append_text(out, text)
        }
        ValueToken::Hash { text, .. } => {
            out.push('#');
            append_text(out, text)
        }
        ValueToken::String { text, .. } => {
            out.push('"');
            append_quoted_text(out, text)?;
            out.push('"');
            Some(())
        }
        ValueToken::BadString { .. } | ValueToken::BadUrl { .. } => None,
        ValueToken::Url { text, .. } => {
            out.push_str("url(");
            append_text(out, text)?;
            out.push(')');
            Some(())
        }
        ValueToken::Delim { value, .. } => {
            out.push(*value);
            Some(())
        }
        ValueToken::Number { text, .. } => append_text(out, text),
        ValueToken::Percentage { text, .. } => {
            append_text(out, text)?;
            out.push('%');
            Some(())
        }
        ValueToken::Dimension { number, unit, .. } => {
            append_text(out, number)?;
            append_text(out, unit)
        }
        ValueToken::UnicodeRange { range, .. } => {
            out.push_str(&format!("U+{:X}-{:X}", range.start(), range.end()));
            Some(())
        }
        ValueToken::Symbol { kind, .. } => {
            out.push_str(match kind {
                ValueSymbol::Colon => ":",
                ValueSymbol::Semicolon => ";",
                ValueSymbol::Comma => ",",
                ValueSymbol::LeftSquareBracket => "[",
                ValueSymbol::RightSquareBracket => "]",
                ValueSymbol::LeftParenthesis => "(",
                ValueSymbol::RightParenthesis => ")",
                ValueSymbol::LeftCurlyBracket => "{",
                ValueSymbol::RightCurlyBracket => "}",
                ValueSymbol::IncludeMatch => "~=",
                ValueSymbol::DashMatch => "|=",
                ValueSymbol::PrefixMatch => "^=",
                ValueSymbol::SuffixMatch => "$=",
                ValueSymbol::SubstringMatch => "*=",
                ValueSymbol::Column => "||",
                ValueSymbol::Cdo => "<!--",
                ValueSymbol::Cdc => "-->",
            });
            Some(())
        }
    }
}

fn append_text(out: &mut String, text: &ValueText) -> Option<()> {
    out.push_str(text.text.as_deref()?);
    Some(())
}

fn append_quoted_text(out: &mut String, text: &ValueText) -> Option<()> {
    for ch in text.text.as_deref()?.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    Some(())
}

fn push_ascii_space(out: &mut String) {
    if !out.chars().last().is_some_and(char::is_whitespace) {
        out.push(' ');
    }
}

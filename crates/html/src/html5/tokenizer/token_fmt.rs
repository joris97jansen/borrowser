//! Deterministic token formatting for golden/WPT tests.
//!
//! This module provides a stable, allocation-only formatting surface for token
//! snapshots. It intentionally preserves tokenizer attribute encounter order.

use crate::html5::shared::{AtomId, AtomTable, Attribute, AttributeValue, TextValue, Token};
use crate::html5::tokenizer::{TextResolveError, TextResolver};
use std::borrow::Cow;

#[derive(Debug)]
pub enum TokenFmtError {
    UnknownAtomId {
        id: AtomId,
    },
    InvalidSpan {
        span: crate::html5::shared::TextSpan,
    },
}

impl std::fmt::Display for TokenFmtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenFmtError::UnknownAtomId { id } => write!(f, "unknown atom id: {id:?}"),
            TokenFmtError::InvalidSpan { span } => {
                write!(f, "invalid span: {}..{}", span.start, span.end)
            }
        }
    }
}

impl std::error::Error for TokenFmtError {}

/// Formatter context used to derive deterministic test strings from tokens.
pub struct TokenFmt<'a> {
    atoms: &'a AtomTable,
    resolver: &'a dyn TextResolver,
}

impl<'a> TokenFmt<'a> {
    pub fn new(atoms: &'a AtomTable, resolver: &'a dyn TextResolver) -> Self {
        Self { atoms, resolver }
    }

    pub fn format_token(&self, token: &Token) -> Result<String, TokenFmtError> {
        token.to_test_string(self)
    }

    pub fn resolve_atom(&self, id: AtomId) -> Result<&str, TokenFmtError> {
        self.atoms
            .resolve(id)
            .ok_or(TokenFmtError::UnknownAtomId { id })
    }

    pub fn resolve_attr_value<'v>(
        &'v self,
        value: &'v AttributeValue,
    ) -> Result<Cow<'v, str>, TokenFmtError> {
        match value {
            AttributeValue::Span(span) => self
                .resolver
                .resolve_span(*span)
                .map(Cow::Borrowed)
                .map_err(map_resolve_error),
            AttributeValue::Owned(text) => Ok(Cow::Borrowed(text.as_str())),
        }
    }

    pub fn resolve_text_value<'v>(
        &'v self,
        value: &'v TextValue,
    ) -> Result<Cow<'v, str>, TokenFmtError> {
        match value {
            TextValue::Span(span) => self
                .resolver
                .resolve_span(*span)
                .map(Cow::Borrowed)
                .map_err(map_resolve_error),
            TextValue::Owned(text) => Ok(Cow::Borrowed(text.as_str())),
        }
    }
}

/// Extension trait for deterministic token snapshot formatting.
pub trait TokenTestFormatExt {
    fn to_test_string(&self, fmt: &TokenFmt<'_>) -> Result<String, TokenFmtError>;
}

impl TokenTestFormatExt for Token {
    fn to_test_string(&self, fmt: &TokenFmt<'_>) -> Result<String, TokenFmtError> {
        match self {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                let name = match name {
                    None => "null".to_string(),
                    Some(id) => fmt.resolve_atom(*id)?.to_string(),
                };
                let public_id = public_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                let system_id = system_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                Ok(format!(
                    "DOCTYPE name={name} public_id={public_id} system_id={system_id} force_quirks={force_quirks}"
                ))
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let name = fmt.resolve_atom(*name)?;
                let mut out = String::new();
                out.push_str("START name=");
                out.push_str(name);
                out.push_str(" attrs=[");
                for (i, attr) in attrs.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push_str(&format_attr(attr, fmt)?);
                }
                out.push_str("] self_closing=");
                out.push_str(if *self_closing { "true" } else { "false" });
                Ok(out)
            }
            Token::EndTag { name } => Ok(format!("END name={}", fmt.resolve_atom(*name)?)),
            Token::Comment { text } => Ok(format!(
                "COMMENT text=\"{}\"",
                escape_text(&fmt.resolve_text_value(text)?)
            )),
            Token::Text { text } => Ok(format!(
                "CHAR text=\"{}\"",
                escape_text(&fmt.resolve_text_value(text)?)
            )),
            Token::Eof => Ok("EOF".to_string()),
        }
    }
}

fn format_attr(attr: &Attribute, fmt: &TokenFmt<'_>) -> Result<String, TokenFmtError> {
    let name = fmt.resolve_atom(attr.name)?;
    match &attr.value {
        None => Ok(name.to_string()),
        Some(value) => Ok(format!(
            "{name}=\"{}\"",
            escape_text(&fmt.resolve_attr_value(value)?)
        )),
    }
}

fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' || ch == '\u{7f}' => {
                use std::fmt::Write;
                let _ = write!(&mut out, "\\u{{{:02X}}}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn map_resolve_error(err: TextResolveError) -> TokenFmtError {
    match err {
        TextResolveError::InvalidSpan { span } => TokenFmtError::InvalidSpan { span },
    }
}

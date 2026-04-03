//! CSS syntax token model.
//!
//! The token definitions here are parser-neutral and intended for later
//! tokenizer/parser stages. They model lexical structure only and do not encode
//! selector matching, cascade, or computed-style semantics.

use super::input::{CssInput, CssSpan};
use std::borrow::Cow;
use std::fmt::Write;

/// Token text payload that may refer back to source input or store owned text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssTokenText {
    Span(CssSpan),
    Owned(String),
}

impl CssTokenText {
    /// Resolve the token payload against its owning source input.
    ///
    /// Returns `None` if a span-backed payload is resolved against the wrong
    /// `CssInput`.
    pub fn resolve<'a>(&'a self, input: &'a CssInput) -> Option<Cow<'a, str>> {
        match self {
            Self::Span(span) => Some(Cow::Borrowed(input.slice(*span)?)),
            Self::Owned(text) => Some(Cow::Borrowed(text.as_str())),
        }
    }
}

/// Lexical classification for hash tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssHashKind {
    Id,
    Unrestricted,
}

/// Numeric lexical classification for number-like tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssNumericKind {
    Integer,
    Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssNumber {
    /// Lexical source text for the number.
    ///
    /// This is intentionally not a parsed numeric-value object yet.
    pub repr: CssTokenText,
    pub kind: CssNumericKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDimension {
    pub number: CssNumber,
    pub unit: CssTokenText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssUnicodeRange {
    start: u32,
    end: u32,
}

impl CssUnicodeRange {
    pub const MAX_CODE_POINT: u32 = 0x10_FFFF;

    pub fn new(start: u32, end: u32) -> Option<Self> {
        if start <= end && end <= Self::MAX_CODE_POINT {
            Some(Self { start, end })
        } else {
            None
        }
    }

    pub fn start(self) -> u32 {
        self.start
    }

    pub fn end(self) -> u32 {
        self.end
    }
}

/// Core CSS lexical tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssTokenKind {
    Whitespace,
    Comment(CssTokenText),
    Ident(CssTokenText),
    Function(CssTokenText),
    AtKeyword(CssTokenText),
    Hash {
        value: CssTokenText,
        kind: CssHashKind,
    },
    String(CssTokenText),
    BadString,
    Url(CssTokenText),
    BadUrl,
    Delim(char),
    Number(CssNumber),
    Percentage(CssNumber),
    Dimension(CssDimension),
    UnicodeRange(CssUnicodeRange),
    Colon,
    Semicolon,
    Comma,
    LeftSquareBracket,
    RightSquareBracket,
    LeftParenthesis,
    RightParenthesis,
    LeftCurlyBracket,
    RightCurlyBracket,
    IncludeMatch,
    DashMatch,
    PrefixMatch,
    SuffixMatch,
    SubstringMatch,
    Column,
    Cdo,
    Cdc,
    Eof,
}

/// One lexical token with a source span in the decoded CSS input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssToken {
    pub kind: CssTokenKind,
    pub span: CssSpan,
}

impl CssToken {
    pub fn new(kind: CssTokenKind, span: CssSpan) -> Self {
        Self { kind, span }
    }
}

pub fn serialize_tokens_for_snapshot(input: &CssInput, tokens: &[CssToken]) -> String {
    let mut out = String::new();
    writeln!(&mut out, "tokens").expect("write tokens header");
    for (index, token) in tokens.iter().enumerate() {
        writeln!(
            &mut out,
            "token[{index}] {} @{}..{}",
            token_kind_snapshot(input, &token.kind),
            token.span.start,
            token.span.end,
        )
        .expect("write token snapshot");
    }
    out
}

fn token_kind_snapshot(input: &CssInput, kind: &CssTokenKind) -> String {
    match kind {
        CssTokenKind::Whitespace => "whitespace".to_string(),
        CssTokenKind::Comment(text) => format!("comment({})", quoted_text(input, text)),
        CssTokenKind::Ident(text) => format!("ident({})", quoted_text(input, text)),
        CssTokenKind::Function(text) => format!("function({})", quoted_text(input, text)),
        CssTokenKind::AtKeyword(text) => format!("at-keyword({})", quoted_text(input, text)),
        CssTokenKind::Hash { value, kind } => format!(
            "hash(kind={}, value={})",
            match kind {
                CssHashKind::Id => "id",
                CssHashKind::Unrestricted => "unrestricted",
            },
            quoted_text(input, value)
        ),
        CssTokenKind::String(text) => format!("string({})", quoted_text(input, text)),
        CssTokenKind::BadString => "bad-string".to_string(),
        CssTokenKind::Url(text) => format!("url({})", quoted_text(input, text)),
        CssTokenKind::BadUrl => "bad-url".to_string(),
        CssTokenKind::Delim(ch) => format!("delim({})", quoted_char(*ch)),
        CssTokenKind::Number(number) => format!(
            "number(kind={}, value={})",
            numeric_kind_label(number.kind),
            quoted_text(input, &number.repr)
        ),
        CssTokenKind::Percentage(number) => format!(
            "percentage(kind={}, value={})",
            numeric_kind_label(number.kind),
            quoted_text(input, &number.repr)
        ),
        CssTokenKind::Dimension(dimension) => format!(
            "dimension(kind={}, value={}, unit={})",
            numeric_kind_label(dimension.number.kind),
            quoted_text(input, &dimension.number.repr),
            quoted_text(input, &dimension.unit)
        ),
        CssTokenKind::UnicodeRange(range) => {
            format!("unicode-range(U+{:X}-U+{:X})", range.start(), range.end())
        }
        CssTokenKind::Colon => "colon".to_string(),
        CssTokenKind::Semicolon => "semicolon".to_string(),
        CssTokenKind::Comma => "comma".to_string(),
        CssTokenKind::LeftSquareBracket => "left-square-bracket".to_string(),
        CssTokenKind::RightSquareBracket => "right-square-bracket".to_string(),
        CssTokenKind::LeftParenthesis => "left-parenthesis".to_string(),
        CssTokenKind::RightParenthesis => "right-parenthesis".to_string(),
        CssTokenKind::LeftCurlyBracket => "left-curly-bracket".to_string(),
        CssTokenKind::RightCurlyBracket => "right-curly-bracket".to_string(),
        CssTokenKind::IncludeMatch => "include-match".to_string(),
        CssTokenKind::DashMatch => "dash-match".to_string(),
        CssTokenKind::PrefixMatch => "prefix-match".to_string(),
        CssTokenKind::SuffixMatch => "suffix-match".to_string(),
        CssTokenKind::SubstringMatch => "substring-match".to_string(),
        CssTokenKind::Column => "column".to_string(),
        CssTokenKind::Cdo => "cdo".to_string(),
        CssTokenKind::Cdc => "cdc".to_string(),
        CssTokenKind::Eof => "eof".to_string(),
    }
}

fn numeric_kind_label(kind: CssNumericKind) -> &'static str {
    match kind {
        CssNumericKind::Integer => "integer",
        CssNumericKind::Number => "number",
    }
}

fn quoted_text(input: &CssInput, text: &CssTokenText) -> String {
    match text.resolve(input) {
        Some(text) => format!("{:?}", text),
        None => "<invalid-span>".to_string(),
    }
}

fn quoted_char(ch: char) -> String {
    format!("{ch:?}")
}

#[cfg(test)]
mod tests {
    use super::{
        CssDimension, CssHashKind, CssNumber, CssNumericKind, CssToken, CssTokenKind, CssTokenText,
        CssUnicodeRange, serialize_tokens_for_snapshot,
    };
    use crate::syntax::input::CssInput;

    #[test]
    fn token_text_resolves_owned_and_spanned_payloads() {
        let input = CssInput::from("color");
        let span = input.span(0, 5).expect("span");
        let other = CssInput::from("color");

        assert_eq!(
            CssTokenText::Span(span).resolve(&input).as_deref(),
            Some("color")
        );
        assert_eq!(CssTokenText::Span(span).resolve(&other).as_deref(), None);
        assert_eq!(
            CssTokenText::Owned("red".to_string())
                .resolve(&input)
                .as_deref(),
            Some("red")
        );
    }

    #[test]
    fn token_snapshot_is_stable_and_parser_neutral() {
        let input = CssInput::from("@media #hero { color: 10px; }");
        let tokens = vec![
            CssToken::new(
                CssTokenKind::AtKeyword(CssTokenText::Owned("media".to_string())),
                input.span(0, 6).expect("span"),
            ),
            CssToken::new(CssTokenKind::Whitespace, input.span(6, 7).expect("span")),
            CssToken::new(
                CssTokenKind::Hash {
                    value: CssTokenText::Owned("hero".to_string()),
                    kind: CssHashKind::Id,
                },
                input.span(7, 12).expect("span"),
            ),
            CssToken::new(
                CssTokenKind::Dimension(CssDimension {
                    number: CssNumber {
                        repr: CssTokenText::Owned("10".to_string()),
                        kind: CssNumericKind::Integer,
                    },
                    unit: CssTokenText::Owned("px".to_string()),
                }),
                input.span(22, 26).expect("span"),
            ),
            CssToken::new(CssTokenKind::Eof, input.span(29, 29).expect("span")),
        ];

        assert_eq!(
            serialize_tokens_for_snapshot(&input, &tokens),
            concat!(
                "tokens\n",
                "token[0] at-keyword(\"media\") @0..6\n",
                "token[1] whitespace @6..7\n",
                "token[2] hash(kind=id, value=\"hero\") @7..12\n",
                "token[3] dimension(kind=integer, value=\"10\", unit=\"px\") @22..26\n",
                "token[4] eof @29..29\n",
            )
        );
    }

    #[test]
    fn token_model_covers_core_lexical_forms() {
        let input = CssInput::from("[]");
        let tokens = vec![
            CssToken::new(
                CssTokenKind::Ident(CssTokenText::Owned("body".to_string())),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::Function(CssTokenText::Owned("rgb".to_string())),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::String(CssTokenText::Owned("text".to_string())),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::BadString,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::Url(CssTokenText::Owned("https://example.com".to_string())),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(CssTokenKind::BadUrl, input.span(0, 0).expect("empty span")),
            CssToken::new(
                CssTokenKind::Delim('>'),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::Number(CssNumber {
                    repr: CssTokenText::Owned("1.5".to_string()),
                    kind: CssNumericKind::Number,
                }),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::Percentage(CssNumber {
                    repr: CssTokenText::Owned("75".to_string()),
                    kind: CssNumericKind::Integer,
                }),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::UnicodeRange(CssUnicodeRange::new(0x41, 0x5A).expect("valid range")),
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(CssTokenKind::Colon, input.span(0, 0).expect("empty span")),
            CssToken::new(
                CssTokenKind::Semicolon,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(CssTokenKind::Comma, input.span(0, 0).expect("empty span")),
            CssToken::new(
                CssTokenKind::LeftSquareBracket,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::RightSquareBracket,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::LeftParenthesis,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::RightParenthesis,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::LeftCurlyBracket,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::RightCurlyBracket,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::IncludeMatch,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::DashMatch,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::PrefixMatch,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::SuffixMatch,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(
                CssTokenKind::SubstringMatch,
                input.span(0, 0).expect("empty span"),
            ),
            CssToken::new(CssTokenKind::Column, input.span(0, 0).expect("empty span")),
            CssToken::new(CssTokenKind::Cdo, input.span(0, 0).expect("empty span")),
            CssToken::new(CssTokenKind::Cdc, input.span(0, 0).expect("empty span")),
            CssToken::new(CssTokenKind::Eof, input.span(0, 0).expect("empty span")),
        ];

        assert_eq!(tokens.len(), 28);
    }

    #[test]
    fn unicode_range_enforces_core_invariants() {
        assert!(CssUnicodeRange::new(0x41, 0x5A).is_some());
        assert!(CssUnicodeRange::new(0x5A, 0x41).is_none());
        assert!(CssUnicodeRange::new(0, 0x11_0000).is_none());
    }
}

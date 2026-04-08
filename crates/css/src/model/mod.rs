//! Engine-facing CSS stylesheet/rule model.
//!
//! This module is the first concrete implementation step of Milestone O. It
//! sits downstream of `css::syntax` and owns long-lived stylesheet/rule
//! containers. Selector/prelude/block payloads remain structurally preserved,
//! while declarations now use model-layer containers with explicit property
//! names, value attachment, and importance metadata.

mod entry;
mod serialize;

#[cfg(test)]
mod tests;

use crate::syntax::{
    CssBlockKind, CssComponentValue, CssHashKind, CssInput, CssNumericKind, CssParseOrigin,
    CssSpan, CssUnicodeRange, ParseStats, SyntaxDiagnostic,
};

pub use self::entry::{parse_stylesheet, parse_stylesheet_with_options};
pub use self::serialize::{
    serialize_stylesheet_for_snapshot, serialize_stylesheet_parse_for_snapshot,
};

/// Engine-facing stylesheet model built from structured syntax output.
///
/// Rules are stored in deterministic source order. The model is deliberately
/// structural: it preserves selector/prelude/block payloads without introducing
/// selector matching or at-rule semantics yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stylesheet {
    pub origin: CssParseOrigin,
    pub rules: Vec<Rule>,
}

impl Default for Stylesheet {
    fn default() -> Self {
        Self {
            origin: CssParseOrigin::Stylesheet,
            rules: Vec::new(),
        }
    }
}

/// One engine-facing stylesheet rule in deterministic source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    Style(StyleRule),
    At(AtRule),
}

/// Preserved component-value slice kept for later selector or at-rule work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreservedComponentList {
    pub span: Option<CssSpan>,
    pub values: Vec<CssComponentValue>,
}

/// Declaration block attached to a style rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationBlock {
    pub span: CssSpan,
    pub declarations: Vec<Declaration>,
}

/// Structured stylesheet declaration in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub span: CssSpan,
    pub name: PropertyName,
    pub value: DeclarationValue,
    pub important: Option<ImportantAnnotation>,
}

/// Explicit property-name representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyName {
    pub span: Option<CssSpan>,
    pub kind: PropertyNameKind,
    pub text: Option<String>,
}

/// Property-name classification at the declaration layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyNameKind {
    Standard,
    Custom,
    Invalid,
}

/// Structurally preserved declaration value.
///
/// If the value becomes empty after structural extraction such as removing a
/// trailing `!important` annotation, `span` is represented as a zero-length
/// span at the original declaration value start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationValue {
    pub span: CssSpan,
    pub components: Vec<ValueComponent>,
}

/// `!important` annotation attached to a declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportantAnnotation {
    pub span: CssSpan,
}

/// Semi-typed declaration value component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueComponent {
    Token(ValueToken),
    SimpleBlock(ValueBlock),
    Function(ValueFunction),
}

/// Semi-typed preserved token inside a declaration value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueToken {
    Whitespace {
        span: CssSpan,
    },
    Comment {
        span: CssSpan,
        text: ValueText,
    },
    Ident {
        span: CssSpan,
        text: ValueText,
    },
    AtKeyword {
        span: CssSpan,
        text: ValueText,
    },
    Hash {
        span: CssSpan,
        kind: CssHashKind,
        text: ValueText,
    },
    String {
        span: CssSpan,
        text: ValueText,
    },
    BadString {
        span: CssSpan,
    },
    Url {
        span: CssSpan,
        text: ValueText,
    },
    BadUrl {
        span: CssSpan,
    },
    Delim {
        span: CssSpan,
        value: char,
    },
    Number {
        span: CssSpan,
        kind: CssNumericKind,
        text: ValueText,
    },
    Percentage {
        span: CssSpan,
        kind: CssNumericKind,
        text: ValueText,
    },
    Dimension {
        span: CssSpan,
        kind: CssNumericKind,
        number: ValueText,
        unit: ValueText,
    },
    UnicodeRange {
        span: CssSpan,
        range: CssUnicodeRange,
    },
    Symbol {
        span: CssSpan,
        kind: ValueSymbol,
    },
}

/// Resolved text payload preserved for model-layer values.
///
/// `text` is authored source text when it resolves successfully against the
/// owning input. It is preserved source text, not computed normalization.
/// `None` means the text could not be resolved from the source-backed payload,
/// not that the authored text was empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueText {
    pub span: Option<CssSpan>,
    pub text: Option<String>,
}

/// Structural block inside a declaration value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueBlock {
    pub span: CssSpan,
    pub kind: CssBlockKind,
    pub components: Vec<ValueComponent>,
}

/// Structural function inside a declaration value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueFunction {
    pub span: CssSpan,
    pub name: ValueText,
    pub components: Vec<ValueComponent>,
}

/// Non-text symbolic token kinds preserved in the value model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueSymbol {
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
}

/// Engine-facing style rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleRule {
    pub span: CssSpan,
    pub selector_source: PreservedComponentList,
    pub declarations: DeclarationBlock,
}

/// Engine-facing at-rule.
///
/// The name is canonicalized to ASCII lowercase when it resolves successfully
/// against the owning source input. Structural payloads remain preserved until
/// later milestones interpret supported at-rules semantically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtRule {
    pub span: CssSpan,
    pub name: Option<String>,
    pub prelude: PreservedComponentList,
    pub block: Option<AtRuleBlock>,
}

/// Extensible at-rule block surface.
///
/// Only preserved blocks are supported in O2. Future milestones can extend
/// this enum with richer variants without changing the outer at-rule contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtRuleBlock {
    Preserved(PreservedBlock),
}

/// Structurally preserved at-rule block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreservedBlock {
    pub span: CssSpan,
    pub kind: CssBlockKind,
    pub values: Vec<CssComponentValue>,
}

/// Parsed stylesheet result for the engine-facing rule model.
#[derive(Clone, Debug)]
pub struct StylesheetParse {
    pub input: CssInput,
    pub stylesheet: Stylesheet,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

impl StylesheetParse {
    pub fn to_debug_snapshot(&self) -> String {
        serialize_stylesheet_parse_for_snapshot(self)
    }
}

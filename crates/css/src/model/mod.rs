//! Engine-facing CSS stylesheet/rule model.
//!
//! This module is the first concrete implementation step of Milestone O. It
//! sits downstream of `css::syntax` and owns long-lived stylesheet/rule
//! containers while deliberately preserving syntax-layer component values for
//! selector/prelude/block payloads until later declaration/value milestones
//! replace those inner payloads with richer model-layer types.

mod entry;
mod serialize;

#[cfg(test)]
mod tests;

use crate::syntax::{
    CssBlockKind, CssComponentValue, CssDeclaration, CssInput, CssParseOrigin, CssSpan, ParseStats,
    SyntaxDiagnostic,
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
///
/// Declarations remain syntax-backed at this stage; later O-milestone work will
/// replace the declaration/value payloads with dedicated model-layer forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationBlock {
    pub span: CssSpan,
    pub declarations: Vec<CssDeclaration>,
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

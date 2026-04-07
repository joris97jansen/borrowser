use super::super::input::{CssInput, CssSpan};
use super::super::token::{CssToken, CssTokenText};
use super::super::{ParseStats, SyntaxDiagnostic};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssBlockKind {
    Curly,
    Square,
    Parenthesis,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssStylesheet {
    pub rules: Vec<CssRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssRule {
    Qualified(CssQualifiedRule),
    At(CssAtRule),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssQualifiedRule {
    pub span: CssSpan,
    pub prelude: Vec<CssComponentValue>,
    pub block: CssDeclarationBlock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssAtRule {
    pub span: CssSpan,
    pub name: CssTokenText,
    pub prelude: Vec<CssComponentValue>,
    pub block: Option<CssSimpleBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDeclarationBlock {
    pub span: CssSpan,
    pub declarations: Vec<CssDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDeclaration {
    pub span: CssSpan,
    pub name: CssTokenText,
    pub value: Vec<CssComponentValue>,
    pub value_span: CssSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssComponentValue {
    PreservedToken(CssToken),
    SimpleBlock(CssSimpleBlock),
    Function(CssFunction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssSimpleBlock {
    pub span: CssSpan,
    pub kind: CssBlockKind,
    pub value: Vec<CssComponentValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssFunction {
    pub span: CssSpan,
    pub name: CssTokenText,
    pub value: Vec<CssComponentValue>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StructuredDeclarationListParse {
    pub input: CssInput,
    pub declarations: Vec<CssDeclaration>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

use super::super::SyntaxDiagnostic;
use super::super::input::CssInput;
use super::super::token::CssToken;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssTokenizationStats {
    pub input_bytes: usize,
    pub tokens_emitted: usize,
    pub diagnostics_emitted: usize,
    pub hit_limit: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CssTokenization {
    pub input: CssInput,
    pub tokens: Vec<CssToken>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: CssTokenizationStats,
}

impl CssTokenization {
    pub fn to_debug_snapshot(&self) -> String {
        super::super::serialize_tokenization_for_snapshot(self)
    }
}

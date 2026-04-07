use super::SyntaxDiagnostic;
use super::compat::{self, CompatStylesheet};
use super::input::CssInput;
use super::parser::CssStylesheet;
use super::serialize::{
    serialize_declaration_list_parse_for_snapshot, serialize_stylesheet_parse_for_snapshot,
};

/// Parse summary for tests and downstream callers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseStats {
    pub input_bytes: usize,
    pub rules_emitted: usize,
    pub declarations_emitted: usize,
    pub diagnostics_emitted: usize,
    pub hit_limit: bool,
}

/// Compatibility-scoped declaration used by the current cascade path.
///
/// This raw-string form is migration-only and is not the permanent
/// engine-facing declaration/value contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default)]
pub struct StylesheetParse {
    pub input: CssInput,
    pub stylesheet: CssStylesheet,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

impl StylesheetParse {
    pub fn to_debug_snapshot(&self) -> String {
        serialize_stylesheet_parse_for_snapshot(self)
    }

    pub fn to_compat_stylesheet(&self) -> CompatStylesheet {
        compat::project_stylesheet_to_compat(&self.input, &self.stylesheet)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclarationListParse {
    pub declarations: Vec<Declaration>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

impl DeclarationListParse {
    pub fn to_debug_snapshot(&self) -> String {
        serialize_declaration_list_parse_for_snapshot(self)
    }
}

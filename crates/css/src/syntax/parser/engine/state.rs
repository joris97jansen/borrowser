use super::super::super::input::CssInput;
use super::super::super::token::{CssToken, CssTokenKind};
use super::super::super::{
    DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, SyntaxDiagnostic, push_diagnostic,
};
use super::super::model::CssBlockKind;
use super::super::support::{block_kind_matches_closer, block_kind_matches_opener};

pub(in super::super) struct StylesheetParser<'a> {
    pub(super) input: &'a CssInput,
    pub(super) tokens: &'a [CssToken],
    pub(super) options: &'a ParseOptions,
    pub(super) base_offset: usize,
    pub(super) diagnostics: &'a mut Vec<SyntaxDiagnostic>,
    pub(super) stats: &'a mut ParseStats,
}

impl<'a> StylesheetParser<'a> {
    pub(in super::super) fn new(
        input: &'a CssInput,
        tokens: &'a [CssToken],
        options: &'a ParseOptions,
        base_offset: usize,
        diagnostics: &'a mut Vec<SyntaxDiagnostic>,
        stats: &'a mut ParseStats,
    ) -> Self {
        Self {
            input,
            tokens,
            options,
            base_offset,
            diagnostics,
            stats,
        }
    }

    pub(in super::super) fn stats_mut(&mut self) -> &mut ParseStats {
        self.stats
    }

    pub(super) fn find_matching_closer(&self, start: usize, kind: CssBlockKind) -> Option<usize> {
        let mut depth = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(start + 1) {
            match &token.kind {
                kind_token if block_kind_matches_opener(kind, kind_token) => depth += 1,
                kind_token if block_kind_matches_closer(kind, kind_token) => {
                    if depth == 0 {
                        return Some(index);
                    }
                    depth -= 1;
                }
                CssTokenKind::Eof => return None,
                _ => {}
            }
        }
        None
    }

    pub(super) fn find_eof_index(&self, start: usize) -> usize {
        self.tokens
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, token)| matches!(token.kind, CssTokenKind::Eof).then_some(index))
            .unwrap_or(self.tokens.len())
    }

    pub(super) fn push_diagnostic(
        &mut self,
        severity: DiagnosticSeverity,
        kind: DiagnosticKind,
        byte_offset: usize,
        message: impl Into<String>,
    ) {
        push_diagnostic(
            self.options,
            self.diagnostics,
            self.stats,
            severity,
            kind,
            self.base_offset.saturating_add(byte_offset),
            message,
        );
    }
}

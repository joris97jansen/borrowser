use super::super::super::input::{CssInput, CssSpan};
use super::super::super::token::{CssToken, CssTokenKind, CssTokenText};
use super::super::super::{DiagnosticKind, DiagnosticSeverity, ParseOptions, SyntaxDiagnostic};
use super::super::scan::{
    advance_css_line_break_in_str, is_css_line_break_start, peek_char_at, peek_char_at_after,
    peek_char_at_after_after,
};

#[derive(Default)]
pub(in super::super) struct TokenizerStats {
    pub(in super::super) diagnostics_emitted: usize,
    pub(in super::super) hit_limit: bool,
}

pub(in super::super) struct CssTokenizer<'a> {
    pub(super) input: &'a CssInput,
    pub(super) options: &'a ParseOptions,
    pub(super) pos: usize,
    pub(in super::super) tokens: Vec<CssToken>,
    pub(in super::super) diagnostics: Vec<SyntaxDiagnostic>,
    pub(in super::super) stats: TokenizerStats,
}

impl<'a> CssTokenizer<'a> {
    pub(in super::super) fn new(input: &'a CssInput, options: &'a ParseOptions) -> Self {
        Self {
            input,
            options,
            pos: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            stats: TokenizerStats::default(),
        }
    }

    pub(super) fn consume_css_line_break_if_present(&mut self) -> bool {
        let Some(next_pos) = advance_css_line_break_in_str(self.input.as_str(), self.pos) else {
            return false;
        };
        self.pos = next_pos;
        true
    }

    pub(super) fn push_token(&mut self, kind: CssTokenKind, start: usize, end: usize) {
        self.push_token_with_kind(kind, start, end);
    }

    pub(super) fn push_token_with_kind(&mut self, kind: CssTokenKind, start: usize, end: usize) {
        let span = self.safe_span(start, end, "invalid token span");
        self.tokens.push(CssToken::new(kind, span));
    }

    pub(super) fn safe_text_span(
        &mut self,
        start: usize,
        end: usize,
        invariant: &'static str,
    ) -> CssTokenText {
        if let Some(span) = self.input.span(start, end) {
            return CssTokenText::Span(span);
        }

        self.push_diagnostic(
            DiagnosticSeverity::Error,
            DiagnosticKind::InvariantViolation,
            start.min(self.input.len_bytes()),
            format!("tokenizer invariant violated: {invariant}"),
        );
        CssTokenText::Owned(self.fallback_text(start, end))
    }

    pub(super) fn safe_span(
        &mut self,
        start: usize,
        end: usize,
        invariant: &'static str,
    ) -> CssSpan {
        if let Some(span) = self.input.span(start, end) {
            return span;
        }

        self.push_diagnostic(
            DiagnosticSeverity::Error,
            DiagnosticKind::InvariantViolation,
            start.min(self.input.len_bytes()),
            format!("tokenizer invariant violated: {invariant}"),
        );
        self.fallback_span(start, end)
    }

    pub(super) fn reached_lexical_token_limit(&mut self) -> bool {
        if self.tokens.len() < self.options.limits.max_lexical_tokens {
            return false;
        }

        self.stats.hit_limit = true;
        self.pos = self.input.len_bytes();
        self.push_diagnostic(
            DiagnosticSeverity::Error,
            DiagnosticKind::LimitExceeded,
            self.pos,
            format!(
                "token count exceeded limit {} (excluding trailing EOF sentinel)",
                self.options.limits.max_lexical_tokens
            ),
        );
        true
    }

    pub(super) fn push_diagnostic(
        &mut self,
        severity: DiagnosticSeverity,
        kind: DiagnosticKind,
        byte_offset: usize,
        message: impl Into<String>,
    ) {
        self.stats.diagnostics_emitted += 1;
        if !self.options.collect_diagnostics
            || self.diagnostics.len() >= self.options.limits.max_diagnostics
        {
            return;
        }
        self.diagnostics.push(SyntaxDiagnostic {
            severity,
            kind,
            byte_offset,
            message: message.into(),
        });
    }

    pub(super) fn peek_char(&self) -> Option<char> {
        self.peek_char_at(self.pos)
    }

    pub(super) fn peek_next_char(&self) -> Option<char> {
        let current = self.peek_char()?;
        self.peek_char_at(self.pos + current.len_utf8())
    }

    pub(super) fn peek_third_char(&self) -> Option<char> {
        let next = self.peek_next_char()?;
        let next_offset = self.pos + self.peek_char()?.len_utf8() + next.len_utf8();
        self.peek_char_at(next_offset)
    }

    pub(super) fn peek_char_at(&self, byte_offset: usize) -> Option<char> {
        peek_char_at(self.input.as_str(), byte_offset)
    }

    pub(super) fn peek_char_at_after(&self, byte_offset: usize) -> Option<char> {
        peek_char_at_after(self.input.as_str(), byte_offset)
    }

    pub(super) fn peek_char_at_after_after(&self, byte_offset: usize) -> Option<char> {
        peek_char_at_after_after(self.input.as_str(), byte_offset)
    }

    pub(super) fn take_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    pub(super) fn is_valid_escape_at(&self, byte_offset: usize) -> bool {
        if self.peek_char_at(byte_offset) != Some('\\') {
            return false;
        }
        let next = self.peek_char_at_after(byte_offset);
        match next {
            Some(ch) => !is_css_line_break_start(ch),
            None => false,
        }
    }

    fn fallback_text(&self, start: usize, end: usize) -> String {
        let fallback = self.fallback_span(start, end);
        self.input.slice(fallback).unwrap_or("").to_string()
    }

    fn fallback_span(&self, start: usize, end: usize) -> CssSpan {
        let clamped_start = self.clamp_char_boundary(start.min(end));
        let clamped_end = self.clamp_char_boundary(end.max(clamped_start));
        CssSpan::new(self.input.id(), clamped_start, clamped_end)
            .unwrap_or_else(|| self.input.zero_span())
    }

    fn clamp_char_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.input.len_bytes());
        while offset > 0 && !self.input.as_str().is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

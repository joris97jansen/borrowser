#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

impl DiagnosticSeverity {
    pub(crate) fn snapshot_label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    UnexpectedEof,
    UnexpectedToken,
    InvariantViolation,
    EmptySelectorList,
    InvalidSelector,
    InvalidDeclaration,
    UnterminatedComment,
    UnterminatedString,
    BadUrl,
    LimitExceeded,
}

impl DiagnosticKind {
    pub(crate) fn stable_code(self) -> &'static str {
        match self {
            Self::UnexpectedEof => "unexpected-eof",
            Self::UnexpectedToken => "unexpected-token",
            Self::InvariantViolation => "invariant-violation",
            Self::EmptySelectorList => "empty-selector-list",
            Self::InvalidSelector => "invalid-selector",
            Self::InvalidDeclaration => "invalid-declaration",
            Self::UnterminatedComment => "unterminated-comment",
            Self::UnterminatedString => "unterminated-string",
            Self::BadUrl => "bad-url",
            Self::LimitExceeded => "limit-exceeded",
        }
    }
}

/// Structured parse diagnostic.
///
/// Diagnostics expose a stable byte offset suitable for tokenizer and parser
/// recovery reporting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub severity: DiagnosticSeverity,
    pub kind: DiagnosticKind,
    pub byte_offset: usize,
    pub message: String,
}

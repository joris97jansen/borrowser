use super::{DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, SyntaxDiagnostic};

pub(crate) fn append_diagnostics(
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    incoming: Vec<SyntaxDiagnostic>,
) {
    if !options.collect_diagnostics || diagnostics.len() >= options.limits.max_diagnostics {
        return;
    }
    let remaining = options.limits.max_diagnostics - diagnostics.len();
    diagnostics.extend(incoming.into_iter().take(remaining));
}

pub(crate) fn push_diagnostic(
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    stats: &mut ParseStats,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
    byte_offset: usize,
    message: impl Into<String>,
) {
    stats.diagnostics_emitted += 1;
    if !options.collect_diagnostics || diagnostics.len() >= options.limits.max_diagnostics {
        return;
    }
    diagnostics.push(SyntaxDiagnostic {
        severity,
        kind,
        byte_offset,
        message: message.into(),
    });
}

pub(crate) fn truncate_to_limit(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }

    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

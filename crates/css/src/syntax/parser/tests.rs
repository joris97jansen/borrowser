use super::super::{
    CssInput, CssToken, CssTokenKind, CssTokenText, DiagnosticKind, ParseOptions, ParseStats,
    SyntaxDiagnostic,
};
use super::support::validate_token_stream_invariants;

#[test]
fn token_stream_invariant_validation_rejects_non_trailing_eof() {
    let input = CssInput::from("a");
    let tokens = vec![
        CssToken::new(CssTokenKind::Eof, input.span(0, 0).expect("eof span")),
        CssToken::new(
            CssTokenKind::Ident(CssTokenText::Owned("a".to_string())),
            input.span(0, 1).expect("ident span"),
        ),
        CssToken::new(CssTokenKind::Eof, input.span(1, 1).expect("final eof span")),
    ];
    let mut diagnostics: Vec<SyntaxDiagnostic> = Vec::new();
    let mut stats = ParseStats::default();

    let valid = validate_token_stream_invariants(
        &ParseOptions::stylesheet(),
        &input,
        &tokens,
        0,
        &mut diagnostics,
        &mut stats,
    );

    assert!(!valid);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::InvariantViolation);
}

#[test]
fn token_stream_invariant_validation_rejects_missing_trailing_eof() {
    let input = CssInput::from("a");
    let tokens = vec![CssToken::new(
        CssTokenKind::Ident(CssTokenText::Owned("a".to_string())),
        input.span(0, 1).expect("ident span"),
    )];
    let mut diagnostics: Vec<SyntaxDiagnostic> = Vec::new();
    let mut stats = ParseStats::default();

    let valid = validate_token_stream_invariants(
        &ParseOptions::stylesheet(),
        &input,
        &tokens,
        0,
        &mut diagnostics,
        &mut stats,
    );

    assert!(!valid);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::InvariantViolation);
}

#[test]
fn token_stream_invariant_validation_rejects_spans_from_other_inputs() {
    let input = CssInput::from("a");
    let other = CssInput::from("a");
    let tokens = vec![
        CssToken::new(
            CssTokenKind::Ident(CssTokenText::Owned("a".to_string())),
            other.span(0, 1).expect("foreign ident span"),
        ),
        CssToken::new(CssTokenKind::Eof, input.span(1, 1).expect("final eof span")),
    ];
    let mut diagnostics: Vec<SyntaxDiagnostic> = Vec::new();
    let mut stats = ParseStats::default();

    let valid = validate_token_stream_invariants(
        &ParseOptions::stylesheet(),
        &input,
        &tokens,
        0,
        &mut diagnostics,
        &mut stats,
    );

    assert!(!valid);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::InvariantViolation);
}

#[test]
fn token_stream_invariant_validation_rejects_non_monotonic_spans() {
    let input = CssInput::from("ab");
    let tokens = vec![
        CssToken::new(
            CssTokenKind::Ident(CssTokenText::Owned("b".to_string())),
            input.span(1, 2).expect("later ident span"),
        ),
        CssToken::new(
            CssTokenKind::Ident(CssTokenText::Owned("a".to_string())),
            input.span(0, 1).expect("earlier ident span"),
        ),
        CssToken::new(CssTokenKind::Eof, input.span(2, 2).expect("final eof span")),
    ];
    let mut diagnostics: Vec<SyntaxDiagnostic> = Vec::new();
    let mut stats = ParseStats::default();

    let valid = validate_token_stream_invariants(
        &ParseOptions::stylesheet(),
        &input,
        &tokens,
        0,
        &mut diagnostics,
        &mut stats,
    );

    assert!(!valid);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::InvariantViolation);
}

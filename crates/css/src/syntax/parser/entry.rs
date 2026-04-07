use super::super::{
    ParseOptions, ParseStats, StylesheetParse, SyntaxDiagnostic, append_diagnostics,
    tokenize_str_with_options,
};
use super::engine::StylesheetParser;
use super::model::{CssStylesheet, StructuredDeclarationListParse};
use super::support::validate_token_stream_invariants;

pub(super) fn parse_stylesheet_structured(input: &str, options: &ParseOptions) -> StylesheetParse {
    let tokenization = tokenize_str_with_options(input, options);
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: tokenization.stats.input_bytes,
        diagnostics_emitted: tokenization.stats.diagnostics_emitted,
        hit_limit: tokenization.stats.hit_limit,
        ..ParseStats::default()
    };
    append_diagnostics(options, &mut diagnostics, tokenization.diagnostics);

    let input = tokenization.input;
    let tokens = tokenization.tokens;
    if !validate_token_stream_invariants(options, &input, &tokens, 0, &mut diagnostics, &mut stats)
    {
        return StylesheetParse {
            input,
            stylesheet: CssStylesheet::default(),
            diagnostics,
            stats,
        };
    }

    let mut parser =
        StylesheetParser::new(&input, &tokens, options, 0, &mut diagnostics, &mut stats);
    let stylesheet = parser.parse_stylesheet();
    parser.stats_mut().rules_emitted = stylesheet.rules.len();

    StylesheetParse {
        input,
        stylesheet,
        diagnostics,
        stats,
    }
}

pub(super) fn parse_declaration_list_structured(
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
) -> StructuredDeclarationListParse {
    let tokenization = tokenize_str_with_options(input, options);
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: tokenization.stats.input_bytes,
        diagnostics_emitted: tokenization.stats.diagnostics_emitted,
        hit_limit: tokenization.stats.hit_limit,
        ..ParseStats::default()
    };
    append_offset_diagnostics(
        options,
        &mut diagnostics,
        tokenization.diagnostics,
        base_offset,
    );

    let input = tokenization.input;
    let tokens = tokenization.tokens;
    if !validate_token_stream_invariants(
        options,
        &input,
        &tokens,
        base_offset,
        &mut diagnostics,
        &mut stats,
    ) {
        return StructuredDeclarationListParse {
            input,
            declarations: Vec::new(),
            diagnostics,
            stats,
        };
    }

    let mut parser = StylesheetParser::new(
        &input,
        &tokens,
        options,
        base_offset,
        &mut diagnostics,
        &mut stats,
    );
    let declarations = parser.parse_declaration_list(0, None);
    parser.stats_mut().declarations_emitted = declarations.len();

    StructuredDeclarationListParse {
        input,
        declarations,
        diagnostics,
        stats,
    }
}

fn append_offset_diagnostics(
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    incoming: Vec<SyntaxDiagnostic>,
    base_offset: usize,
) {
    if base_offset == 0 {
        append_diagnostics(options, diagnostics, incoming);
        return;
    }

    let adjusted = incoming
        .into_iter()
        .map(|mut diagnostic| {
            diagnostic.byte_offset = diagnostic.byte_offset.saturating_add(base_offset);
            diagnostic
        })
        .collect();
    append_diagnostics(options, diagnostics, adjusted);
}

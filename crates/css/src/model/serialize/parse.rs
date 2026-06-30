use super::super::{DeclarationListParse, StylesheetParse};
use super::stylesheet::{
    serialize_declaration_list_for_snapshot, serialize_stylesheet_for_snapshot,
};
use crate::syntax::{ParseStats, SyntaxDiagnostic};
use std::fmt::Write;

pub fn serialize_stylesheet_parse_for_snapshot(parse: &StylesheetParse) -> String {
    let mut out = serialize_stylesheet_for_snapshot(&parse.input, &parse.stylesheet);
    serialize_diagnostics_for_snapshot(&mut out, &parse.diagnostics);
    serialize_stats_for_snapshot(&mut out, &parse.stats);
    out
}

pub(crate) fn serialize_declaration_list_parse_for_snapshot(
    parse: &DeclarationListParse,
) -> String {
    let mut out = serialize_declaration_list_for_snapshot(&parse.input, &parse.declarations);
    serialize_diagnostics_for_snapshot(&mut out, &parse.diagnostics);
    serialize_stats_for_snapshot(&mut out, &parse.stats);
    out
}

fn serialize_diagnostics_for_snapshot(out: &mut String, diagnostics: &[SyntaxDiagnostic]) {
    writeln!(out, "diagnostics").expect("write diagnostics header");
    for diagnostic in diagnostics {
        writeln!(
            out,
            "  - {} {} @{}",
            diagnostic.severity.snapshot_label(),
            diagnostic.kind.stable_code(),
            diagnostic.byte_offset,
        )
        .expect("write diagnostic snapshot");
    }
}

fn serialize_stats_for_snapshot(out: &mut String, stats: &ParseStats) {
    writeln!(out, "stats").expect("write stats header");
    writeln!(out, "  input_bytes: {}", stats.input_bytes).expect("write input_bytes");
    writeln!(out, "  rules_emitted: {}", stats.rules_emitted).expect("write rules_emitted");
    writeln!(
        out,
        "  declarations_emitted: {}",
        stats.declarations_emitted
    )
    .expect("write declarations_emitted");
    writeln!(out, "  diagnostics_emitted: {}", stats.diagnostics_emitted)
        .expect("write diagnostics_emitted");
    writeln!(out, "  hit_limit: {}", stats.hit_limit).expect("write hit_limit");
}

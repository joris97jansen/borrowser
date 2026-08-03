use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, fixed_fields, strict_record_lines, validate_u64,
};
use crate::parser_snapshot::parse_errors::insertion_mode_name;
use html::conformance::{
    ObservationState, ParserTokenKind, TreeConstructionUnsupportedFeature, UnsupportedFeatureEvent,
};
use std::fmt::Write;

const HEADER: &str = "# format: html5-unsupported-features-v1";

define_snapshot_types!(
    ParsedUnsupportedFeaturesSnapshot,
    CanonicalUnsupportedFeaturesSnapshot
);

pub(super) fn write(
    state: &ObservationState<Vec<UnsupportedFeatureEvent>>,
) -> Result<CanonicalUnsupportedFeaturesSnapshot, ()> {
    let ObservationState::Captured(events) = state else {
        return Err(());
    };
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    for event in events {
        let UnsupportedFeatureEvent::TreeConstruction {
            occurrence,
            feature,
            context,
        } = event;
        let line = format!(
            "UNSUPPORTED_FEATURE occurrence={occurrence} subsystem=tree-construction feature={} context-token={} context-mode={} context-namespace={}",
            feature_name(*feature),
            context.token_kind.map_or("null", token_name),
            context.insertion_mode.map_or("null", insertion_mode_name),
            context
                .adjusted_current_node_namespace
                .map_or("null", |v| v.snapshot_name())
        );
        let _ = writeln!(&mut bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("occurrence {occurrence}"),
            line,
        });
    }
    Ok(CanonicalUnsupportedFeaturesSnapshot::new(
        SnapshotData::new(bytes, records),
    ))
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedUnsupportedFeaturesSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, true)?;
    let mut expected = 1u64;
    let mut records = Vec::new();
    for (line_number, line) in lines {
        let Some(f) = fixed_fields(
            line,
            "UNSUPPORTED_FEATURE",
            &[
                "occurrence",
                "subsystem",
                "feature",
                "context-token",
                "context-mode",
                "context-namespace",
            ],
        ) else {
            return malformed(line_number);
        };
        if !validate_u64(f[0])
            || f[0].parse::<u64>().ok() != Some(expected)
            || f[1] != "tree-construction"
            || !valid_feature(f[2])
            || !valid_token(f[3])
            || !valid_mode(f[4])
            || !matches!(f[5], "null" | "html" | "svg" | "mathml")
        {
            return malformed(line_number);
        }
        records.push(SnapshotRecord {
            location: format!("occurrence {expected}"),
            line: line.to_string(),
        });
        expected = expected
            .checked_add(1)
            .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?;
    }
    Ok(ParsedUnsupportedFeaturesSnapshot::new(SnapshotData::new(
        std::str::from_utf8(bytes)
            .map_err(|_| SnapshotReadError::InvalidUtf8)?
            .to_string(),
        records,
    )))
}

fn feature_name(value: TreeConstructionUnsupportedFeature) -> &'static str {
    match value {
    TreeConstructionUnsupportedFeature::MergeAttributesIntoExistingHtmlElement => "merge-attributes-into-existing-html-element",
    TreeConstructionUnsupportedFeature::MergeAttributesIntoExistingBodyElement => "merge-attributes-into-existing-body-element",
    TreeConstructionUnsupportedFeature::MarkFramesetNotOkForRepeatedBodyStartTag => "mark-frameset-not-ok-for-repeated-body-start-tag",
    TreeConstructionUnsupportedFeature::RequireSameNamedTableCellInScopeForEndTag => "require-same-named-table-cell-in-scope-for-end-tag",
    TreeConstructionUnsupportedFeature::GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingTableCell => "generate-implied-end-tags-and-check-current-node-before-closing-table-cell",
    TreeConstructionUnsupportedFeature::GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingCaption => "generate-implied-end-tags-and-check-current-node-before-closing-caption",
}
}
fn token_name(value: ParserTokenKind) -> &'static str {
    match value {
        ParserTokenKind::Doctype => "doctype",
        ParserTokenKind::StartTag => "start-tag",
        ParserTokenKind::EndTag => "end-tag",
        ParserTokenKind::Character => "character",
        ParserTokenKind::Comment => "comment",
        ParserTokenKind::ProcessingInstruction => "processing-instruction",
        ParserTokenKind::Eof => "eof",
    }
}
fn valid_feature(v: &str) -> bool {
    matches!(
        v,
        "merge-attributes-into-existing-html-element"
            | "merge-attributes-into-existing-body-element"
            | "mark-frameset-not-ok-for-repeated-body-start-tag"
            | "require-same-named-table-cell-in-scope-for-end-tag"
            | "generate-implied-end-tags-and-check-current-node-before-closing-table-cell"
            | "generate-implied-end-tags-and-check-current-node-before-closing-caption"
    )
}
fn valid_token(v: &str) -> bool {
    matches!(
        v,
        "null"
            | "doctype"
            | "start-tag"
            | "end-tag"
            | "character"
            | "comment"
            | "processing-instruction"
            | "eof"
    )
}
fn valid_mode(v: &str) -> bool {
    matches!(
        v,
        "null"
            | "initial"
            | "before-html"
            | "before-head"
            | "in-head"
            | "after-head"
            | "in-body"
            | "after-body"
            | "after-after-body"
            | "in-table"
            | "in-table-text"
            | "in-caption"
            | "in-column-group"
            | "in-table-body"
            | "in-row"
            | "in-cell"
            | "in-template"
            | "text"
    )
}
fn malformed<T>(line: usize) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord {
        line,
        reason: "invalid unsupported-feature record",
    })
}

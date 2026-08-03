use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, escape_quoted, fixed_fields, strict_record_lines,
    validate_bool, validate_nullable_quoted, validate_u64,
};
use crate::parser_snapshot::parse_errors::insertion_mode_name;
use html::conformance::{
    ObservationState, TransitionTokenSummary, TreeDispatchPath, TreeTransitionEvent,
};
use std::fmt::Write;

const HEADER: &str = "# format: html5-tree-transitions-v1";

define_snapshot_types!(ParsedTransitionsSnapshot, CanonicalTransitionsSnapshot);

pub(super) fn write(
    state: &ObservationState<Vec<TreeTransitionEvent>>,
) -> Result<CanonicalTransitionsSnapshot, ()> {
    let ObservationState::Captured(events) = state else {
        return Err(());
    };
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    for event in events {
        let (kind, name, data, self_closing) = match event.token.as_ref() {
            TransitionTokenSummary::Doctype => {
                ("doctype", "null".to_string(), "null".to_string(), "null")
            }
            TransitionTokenSummary::StartTag { name, self_closing } => (
                "start-tag",
                escape_quoted(name),
                "null".to_string(),
                if *self_closing { "true" } else { "false" },
            ),
            TransitionTokenSummary::EndTag { name } => {
                ("end-tag", escape_quoted(name), "null".to_string(), "null")
            }
            TransitionTokenSummary::Character { data } => {
                ("character", "null".to_string(), escape_quoted(data), "null")
            }
            TransitionTokenSummary::Comment => {
                ("comment", "null".to_string(), "null".to_string(), "null")
            }
            TransitionTokenSummary::ProcessingInstruction { target } => (
                "processing-instruction",
                escape_quoted(target),
                "null".to_string(),
                "null",
            ),
            TransitionTokenSummary::Eof => ("eof", "null".to_string(), "null".to_string(), "null"),
        };
        let dispatch = match event.dispatch_path {
            TreeDispatchPath::HtmlInsertionMode(mode) => {
                format!("html-insertion-mode:{}", insertion_mode_name(mode))
            }
            TreeDispatchPath::SharedTemplateRules => "shared-template-rules".to_string(),
            TreeDispatchPath::ForeignContent => "foreign-content".to_string(),
            TreeDispatchPath::TextMode => "text-mode".to_string(),
        };
        let line = format!(
            "TRANSITION occurrence={} token-kind={kind} token-name={name} token-data={data} token-self-closing={self_closing} mode-before={} dispatch={} mode-after={} reprocessed={}",
            event.occurrence,
            insertion_mode_name(event.insertion_mode_before),
            dispatch,
            insertion_mode_name(event.insertion_mode_after),
            event.reprocessed
        );
        let _ = writeln!(&mut bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("occurrence {}", event.occurrence),
            line,
        });
    }
    Ok(CanonicalTransitionsSnapshot::new(SnapshotData::new(
        bytes, records,
    )))
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedTransitionsSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, true)?;
    let mut expected = 1u64;
    let mut records = Vec::new();
    for (line_number, line) in lines {
        let Some(f) = fixed_fields(
            line,
            "TRANSITION",
            &[
                "occurrence",
                "token-kind",
                "token-name",
                "token-data",
                "token-self-closing",
                "mode-before",
                "dispatch",
                "mode-after",
                "reprocessed",
            ],
        ) else {
            return malformed(line_number);
        };
        if !validate_u64(f[0])
            || f[0].parse::<u64>().ok() != Some(expected)
            || !valid_token(f[1], f[2], f[3], f[4])
            || !valid_mode(f[5])
            || !valid_dispatch(f[6])
            || !valid_mode(f[7])
            || !validate_bool(f[8])
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
    Ok(ParsedTransitionsSnapshot::new(SnapshotData::new(
        std::str::from_utf8(bytes)
            .map_err(|_| SnapshotReadError::InvalidUtf8)?
            .to_string(),
        records,
    )))
}

fn valid_token(kind: &str, name: &str, data: &str, self_closing: &str) -> bool {
    match kind {
        "doctype" | "comment" | "eof" => name == "null" && data == "null" && self_closing == "null",
        "start-tag" => {
            validate_nullable_quoted(name)
                && name != "null"
                && data == "null"
                && validate_bool(self_closing)
        }
        "end-tag" | "processing-instruction" => {
            validate_nullable_quoted(name)
                && name != "null"
                && data == "null"
                && self_closing == "null"
        }
        "character" => {
            name == "null"
                && validate_nullable_quoted(data)
                && data != "null"
                && self_closing == "null"
        }
        _ => false,
    }
}
fn valid_mode(value: &str) -> bool {
    matches!(
        value,
        "initial"
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
fn valid_dispatch(value: &str) -> bool {
    matches!(
        value,
        "shared-template-rules" | "foreign-content" | "text-mode"
    ) || value
        .strip_prefix("html-insertion-mode:")
        .is_some_and(valid_mode)
}
fn malformed<T>(line: usize) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord {
        line,
        reason: "invalid tree-transition record",
    })
}

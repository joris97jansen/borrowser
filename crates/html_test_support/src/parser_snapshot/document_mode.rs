use super::SnapshotData;
use super::lexical::{SnapshotReadError, SnapshotRecord, fixed_fields, strict_record_lines};
use html::DocumentMode;
use html::conformance::ObservationState;

const HEADER: &str = "# format: html5-document-mode-v1";

define_snapshot_types!(ParsedDocumentModeSnapshot, CanonicalDocumentModeSnapshot);

pub(super) fn write(
    state: &ObservationState<DocumentMode>,
) -> Result<CanonicalDocumentModeSnapshot, ()> {
    let ObservationState::Captured(mode) = state else {
        return Err(());
    };
    let line = match mode {
        DocumentMode::NoQuirks => "MODE value=no-quirks",
        DocumentMode::LimitedQuirks => "MODE value=limited-quirks",
        DocumentMode::Quirks => "MODE value=quirks",
    }
    .to_string();
    Ok(CanonicalDocumentModeSnapshot::new(SnapshotData::new(
        format!("{HEADER}\n{line}\n"),
        vec![SnapshotRecord {
            location: "document mode".to_string(),
            line,
        }],
    )))
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedDocumentModeSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, false)?;
    if lines.len() != 1 {
        return Err(SnapshotReadError::TrailingContent {
            line: lines.get(1).map_or(2, |v| v.0),
        });
    }
    let (line_number, line) = lines[0];
    let Some(fields) = fixed_fields(line, "MODE", &["value"]) else {
        return malformed(line_number);
    };
    if !matches!(fields[0], "no-quirks" | "limited-quirks" | "quirks") {
        return malformed(line_number);
    }
    Ok(ParsedDocumentModeSnapshot::new(SnapshotData::new(
        std::str::from_utf8(bytes)
            .map_err(|_| SnapshotReadError::InvalidUtf8)?
            .to_string(),
        vec![SnapshotRecord {
            location: "document mode".to_string(),
            line: line.to_string(),
        }],
    )))
}

fn malformed<T>(line: usize) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord {
        line,
        reason: "invalid document-mode record",
    })
}

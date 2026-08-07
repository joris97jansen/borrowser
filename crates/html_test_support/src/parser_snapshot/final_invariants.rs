use super::SnapshotData;
use super::lexical::{SnapshotReadError, SnapshotRecord, fixed_fields, strict_record_lines};
use html::conformance::{
    FinalInvariantField, InvariantNotApplicableReason, InvariantOutcome, ObservationState,
    ParserFinalizationReport,
};

const HEADER: &str = "# format: html5-final-invariants-v1";

define_snapshot_types!(
    ParsedFinalInvariantsSnapshot,
    CanonicalFinalInvariantsSnapshot
);

pub(super) fn write(
    state: &ObservationState<ParserFinalizationReport>,
) -> Result<CanonicalFinalInvariantsSnapshot, ()> {
    let ObservationState::Captured(report) = state else {
        return Err(());
    };
    let mut bytes = String::from(HEADER);
    bytes.push('\n');
    let mut records = Vec::with_capacity(16);
    for (index, (field, outcome)) in report.fields().enumerate() {
        let ordinal = index.checked_add(1).ok_or(())?;
        let line = format!(
            "INVARIANT ordinal={ordinal} field={} outcome={}",
            field_name(field),
            outcome_name(outcome)
        );
        bytes.push_str(&line);
        bytes.push('\n');
        records.push(SnapshotRecord {
            location: field_name(field).to_string(),
            line,
        });
    }
    Ok(CanonicalFinalInvariantsSnapshot::new(SnapshotData::new(
        bytes, records,
    )))
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedFinalInvariantsSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, false)?;
    if lines.len() != 16 {
        return Err(SnapshotReadError::TrailingContent {
            line: lines.get(16).map_or(lines.len() + 2, |line| line.0),
        });
    }
    let fields = canonical_fields();
    let mut records = Vec::with_capacity(16);
    for (index, (line_number, line)) in lines.iter().copied().enumerate() {
        let Some(values) = fixed_fields(line, "INVARIANT", &["ordinal", "field", "outcome"]) else {
            return malformed(line_number);
        };
        let expected_ordinal = index
            .checked_add(1)
            .ok_or(SnapshotReadError::MalformedRecord {
                line: line_number,
                reason: "final-invariant ordinal overflow",
            })?;
        if values[0] != expected_ordinal.to_string()
            || values[1] != field_name(fields[index])
            || !valid_outcome(values[2])
        {
            return malformed(line_number);
        }
        records.push(SnapshotRecord {
            location: field_name(fields[index]).to_string(),
            line: line.to_string(),
        });
    }
    Ok(ParsedFinalInvariantsSnapshot::new(SnapshotData::new(
        std::str::from_utf8(bytes)
            .map_err(|_| SnapshotReadError::InvalidUtf8)?
            .to_string(),
        records,
    )))
}

fn canonical_fields() -> [FinalInvariantField; 16] {
    [
        FinalInvariantField::DecoderCarryEmpty,
        FinalInvariantField::PreprocessingFlushed,
        FinalInvariantField::EofEmittedOnce,
        FinalInvariantField::PendingConstructsFlushed,
        FinalInvariantField::OutputAccountedFor,
        FinalInvariantField::PendingTableTextEmpty,
        FinalInvariantField::InsertionModeValid,
        FinalInvariantField::OpenElementsConsistent,
        FinalInvariantField::ActiveFormattingConsistent,
        FinalInvariantField::TemplateModesConsistent,
        FinalInvariantField::FormPointerValid,
        FinalInvariantField::ParentChildLinksValid,
        FinalInvariantField::NamespacesValid,
        FinalInvariantField::TemplateAssociationsValid,
        FinalInvariantField::AllPatchesMaterialized,
        FinalInvariantField::LiveTreeMatchesMaterializedDom,
    ]
}

pub(crate) const fn field_name(field: FinalInvariantField) -> &'static str {
    match field {
        FinalInvariantField::DecoderCarryEmpty => "decoder-carry-empty",
        FinalInvariantField::PreprocessingFlushed => "preprocessing-flushed",
        FinalInvariantField::EofEmittedOnce => "eof-emitted-once",
        FinalInvariantField::PendingConstructsFlushed => "pending-constructs-flushed",
        FinalInvariantField::OutputAccountedFor => "output-accounted-for",
        FinalInvariantField::PendingTableTextEmpty => "pending-table-text-empty",
        FinalInvariantField::InsertionModeValid => "insertion-mode-valid",
        FinalInvariantField::OpenElementsConsistent => "open-elements-consistent",
        FinalInvariantField::ActiveFormattingConsistent => "active-formatting-consistent",
        FinalInvariantField::TemplateModesConsistent => "template-modes-consistent",
        FinalInvariantField::FormPointerValid => "form-pointer-valid",
        FinalInvariantField::ParentChildLinksValid => "parent-child-links-valid",
        FinalInvariantField::NamespacesValid => "namespaces-valid",
        FinalInvariantField::TemplateAssociationsValid => "template-associations-valid",
        FinalInvariantField::AllPatchesMaterialized => "all-patches-materialized",
        FinalInvariantField::LiveTreeMatchesMaterializedDom => "live-tree-matches-materialized-dom",
    }
}

fn outcome_name(outcome: &InvariantOutcome) -> &'static str {
    match outcome {
        InvariantOutcome::Satisfied => "satisfied",
        InvariantOutcome::Failed => "failed",
        InvariantOutcome::NotApplicable(InvariantNotApplicableReason::StandaloneTokenizerRun) => {
            "not-applicable:standalone-tokenizer-run"
        }
        InvariantOutcome::NotApplicable(InvariantNotApplicableReason::DocumentParserRun) => {
            "not-applicable:document-parser-run"
        }
        InvariantOutcome::NotApplicable(InvariantNotApplicableReason::FragmentParserRun) => {
            "not-applicable:fragment-parser-run"
        }
    }
}

fn valid_outcome(value: &str) -> bool {
    matches!(
        value,
        "satisfied"
            | "failed"
            | "not-applicable:standalone-tokenizer-run"
            | "not-applicable:document-parser-run"
            | "not-applicable:fragment-parser-run"
    )
}

fn malformed<T>(line: usize) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord {
        line,
        reason: "invalid final-invariant record",
    })
}

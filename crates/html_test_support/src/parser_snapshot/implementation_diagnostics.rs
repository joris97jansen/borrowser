use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, fixed_fields, strict_record_lines, validate_u64,
};
use crate::parser_snapshot::parse_errors::insertion_mode_name;
use html::conformance::*;
use std::fmt::Write;

const HEADER: &str = "# format: html5-implementation-diagnostics-v1";

define_snapshot_types!(
    ParsedImplementationDiagnosticsSnapshot,
    CanonicalImplementationDiagnosticsSnapshot
);

pub(super) fn write(
    state: &ObservationState<Vec<ImplementationDiagnosticEvent>>,
) -> Result<CanonicalImplementationDiagnosticsSnapshot, ()> {
    let ObservationState::Captured(events) = state else {
        return Err(());
    };
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    for event in events {
        let metadata = event.metadata();
        let (code, payload) = match event {
            ImplementationDiagnosticEvent::InvalidUtf8Replaced {
                reason, payload, ..
            } => (
                match reason {
                    Utf8ReplacementReason::InvalidSequence => {
                        "invalid-utf8-replaced:invalid-sequence"
                    }
                    Utf8ReplacementReason::IncompleteSequenceAtEof => {
                        "invalid-utf8-replaced:incomplete-sequence-at-eof"
                    }
                },
                format!("affected-byte-count:{}", payload.affected_byte_count.get()),
            ),
            ImplementationDiagnosticEvent::ParserResourceLimitActivated {
                limit, payload, ..
            } => (
                resource_limit_name(*limit),
                format!("configured-limit:{}", payload.configured_limit),
            ),
            ImplementationDiagnosticEvent::ParserGuardrailActivated {
                guardrail, payload, ..
            } => (
                match guardrail {
                    ParserGuardrail::TokenizerStallRecovery => {
                        "parser-guardrail:tokenizer-stall-recovery"
                    }
                },
                format!(
                    "consecutive-stall-steps:{}",
                    payload.consecutive_stall_steps.get()
                ),
            ),
            ImplementationDiagnosticEvent::TreeConstruction { code, .. } => {
                (tree_code_name(*code), "none".to_string())
            }
        };
        let (context, token, mode, namespace) = context_fields(metadata.context.as_ref());
        let line = format!(
            "IMPLEMENTATION_DIAGNOSTIC occurrence={} stage={} code={} payload={} position={} context={} context-token={} context-mode={} context-namespace={}",
            metadata.occurrence,
            stage_name(metadata.stage),
            code,
            payload,
            position_name(&metadata.position),
            context,
            token,
            mode,
            namespace
        );
        let _ = writeln!(&mut bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("occurrence {}", metadata.occurrence),
            line,
        });
    }
    Ok(CanonicalImplementationDiagnosticsSnapshot::new(
        SnapshotData::new(bytes, records),
    ))
}

pub(super) fn read(
    bytes: &[u8],
) -> Result<ParsedImplementationDiagnosticsSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, true)?;
    let mut expected = 1u64;
    let mut records = Vec::new();
    for (line_number, line) in lines {
        let Some(fields) = fixed_fields(
            line,
            "IMPLEMENTATION_DIAGNOSTIC",
            &[
                "occurrence",
                "stage",
                "code",
                "payload",
                "position",
                "context",
                "context-token",
                "context-mode",
                "context-namespace",
            ],
        ) else {
            return malformed(
                line_number,
                "invalid implementation-diagnostic record shape",
            );
        };
        if !validate_u64(fields[0]) || fields[0].parse::<u64>().ok() != Some(expected) {
            return Err(SnapshotReadError::NonContiguousOrdinal { line: line_number });
        }
        if !valid_stage(fields[1])
            || !valid_code_payload(fields[2], fields[3])
            || !valid_position(fields[4])
            || !valid_context(fields[5], fields[6], fields[7], fields[8])
        {
            return malformed(
                line_number,
                "unknown spelling or malformed implementation-diagnostic field",
            );
        }
        records.push(SnapshotRecord {
            location: format!("occurrence {expected}"),
            line: line.to_string(),
        });
        expected = expected
            .checked_add(1)
            .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?;
    }
    Ok(ParsedImplementationDiagnosticsSnapshot::new(
        SnapshotData::new(
            std::str::from_utf8(bytes)
                .map_err(|_| SnapshotReadError::InvalidUtf8)?
                .to_string(),
            records,
        ),
    ))
}

fn resource_limit_name(value: ParserResourceLimit) -> &'static str {
    match value {
        ParserResourceLimit::TokenBatchCapacity => "parser-resource-limit:token-batch-capacity",
        ParserResourceLimit::TagNameBytes => "parser-resource-limit:tag-name-bytes",
        ParserResourceLimit::AttributeNameBytes => "parser-resource-limit:attribute-name-bytes",
        ParserResourceLimit::AttributeValueBytes => "parser-resource-limit:attribute-value-bytes",
        ParserResourceLimit::AttributesPerTag => "parser-resource-limit:attributes-per-tag",
        ParserResourceLimit::CommentBytes => "parser-resource-limit:comment-bytes",
        ParserResourceLimit::ProcessingInstructionTargetBytes => {
            "parser-resource-limit:processing-instruction-target-bytes"
        }
        ParserResourceLimit::ProcessingInstructionDataBytes => {
            "parser-resource-limit:processing-instruction-data-bytes"
        }
        ParserResourceLimit::DoctypeBytes => "parser-resource-limit:doctype-bytes",
        ParserResourceLimit::EndTagMatchScanBytes => {
            "parser-resource-limit:end-tag-match-scan-bytes"
        }
        ParserResourceLimit::NumericCharacterReferenceDigits => {
            "parser-resource-limit:numeric-character-reference-digits"
        }
        ParserResourceLimit::TreeOpenElementsDepth => {
            "parser-resource-limit:tree-open-elements-depth"
        }
        ParserResourceLimit::TreeNodeCount => "parser-resource-limit:tree-node-count",
        ParserResourceLimit::TreeChildrenPerNode => "parser-resource-limit:tree-children-per-node",
        ParserResourceLimit::TreeTemplateModeDepth => {
            "parser-resource-limit:tree-template-mode-depth"
        }
    }
}
fn tree_code_name(value: TreeConstructionImplementationDiagnosticCode) -> &'static str {
    match value {
    TreeConstructionImplementationDiagnosticCode::UnsupportedTableInsertionModeFallback => "tree-construction:unsupported-table-insertion-mode-fallback",
    TreeConstructionImplementationDiagnosticCode::UnexpectedStartTagTokenInTextMode => "tree-construction:unexpected-start-tag-token-in-text-mode",
    TreeConstructionImplementationDiagnosticCode::TextModeStartTagAttributeValuesDiscarded => "tree-construction:text-mode-start-tag-attribute-values-discarded",
    TreeConstructionImplementationDiagnosticCode::TextModeStartTagAttributeNamesCanonicalized => "tree-construction:text-mode-start-tag-attribute-names-canonicalized",
    TreeConstructionImplementationDiagnosticCode::UnexpectedDoctypeTokenInTextMode => "tree-construction:unexpected-doctype-token-in-text-mode",
    TreeConstructionImplementationDiagnosticCode::UnexpectedEndTagTokenInTextMode => "tree-construction:unexpected-end-tag-token-in-text-mode",
    TreeConstructionImplementationDiagnosticCode::NonVoidHtmlSelfClosingFlagAlteredStackDisposition => "tree-construction:non-void-html-self-closing-flag-altered-stack-disposition",
}
}
fn stage_name(value: ParserStage) -> &'static str {
    match value {
        ParserStage::InputPreprocessing(InputPreprocessingStage::Utf8Decoding) => {
            "input-preprocessing:utf8-decoding"
        }
        ParserStage::InputPreprocessing(InputPreprocessingStage::NewlineNormalization) => {
            "input-preprocessing:newline-normalization"
        }
        ParserStage::Tokenizer => "tokenizer",
        ParserStage::TreeConstruction => "tree-construction",
        ParserStage::Finalization => "finalization",
    }
}
fn position_name(value: &EventPosition) -> String {
    match value {
        EventPosition::Unavailable(PositionUnavailableReason::ParserDidNotProvidePosition) => {
            "unavailable:parser-did-not-provide-position".to_string()
        }
        EventPosition::Known(position) => {
            let source = match position.source_bytes {
                SourceBytePosition::Exact(value) => format!("exact:{value}"),
                SourceBytePosition::Unavailable(
                    SourcePositionUnavailableReason::NoInputProvenanceMap,
                ) => "unavailable:no-input-provenance-map".to_string(),
            };
            match position.normalized.space {
                InputCoordinateSpace::NormalizedUtf8 => format!(
                    "normalized-utf8:{}:{}:{}:source-{source}",
                    position.normalized.utf8_byte_offset,
                    position.normalized.line.get(),
                    position.normalized.column.get()
                ),
            }
        }
    }
}
fn context_fields(
    value: Option<&ParserContextSummary>,
) -> (&'static str, &'static str, &'static str, &'static str) {
    let Some(value) = value else {
        return ("absent", "null", "null", "null");
    };
    (
        "present",
        value.token_kind.map_or("null", token_kind_name),
        value.insertion_mode.map_or("null", insertion_mode_name),
        value
            .adjusted_current_node_namespace
            .map_or("null", |v| v.snapshot_name()),
    )
}
fn token_kind_name(value: ParserTokenKind) -> &'static str {
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
fn valid_stage(v: &str) -> bool {
    matches!(
        v,
        "input-preprocessing:utf8-decoding"
            | "input-preprocessing:newline-normalization"
            | "tokenizer"
            | "tree-construction"
            | "finalization"
    )
}
fn valid_code_payload(code: &str, payload: &str) -> bool {
    if matches!(
        code,
        "invalid-utf8-replaced:invalid-sequence"
            | "invalid-utf8-replaced:incomplete-sequence-at-eof"
    ) {
        return payload
            .strip_prefix("affected-byte-count:")
            .is_some_and(|v| validate_u64(v) && v != "0");
    }
    if let Some(limit) = code.strip_prefix("parser-resource-limit:") {
        return matches!(
            limit,
            "token-batch-capacity"
                | "tag-name-bytes"
                | "attribute-name-bytes"
                | "attribute-value-bytes"
                | "attributes-per-tag"
                | "comment-bytes"
                | "processing-instruction-target-bytes"
                | "processing-instruction-data-bytes"
                | "doctype-bytes"
                | "end-tag-match-scan-bytes"
                | "numeric-character-reference-digits"
                | "tree-open-elements-depth"
                | "tree-node-count"
                | "tree-children-per-node"
                | "tree-template-mode-depth"
        ) && payload
            .strip_prefix("configured-limit:")
            .is_some_and(validate_u64);
    }
    if code == "parser-guardrail:tokenizer-stall-recovery" {
        return payload
            .strip_prefix("consecutive-stall-steps:")
            .is_some_and(|v| validate_u64(v) && v != "0");
    }
    matches!(
        code,
        "tree-construction:unsupported-table-insertion-mode-fallback"
            | "tree-construction:unexpected-start-tag-token-in-text-mode"
            | "tree-construction:text-mode-start-tag-attribute-values-discarded"
            | "tree-construction:text-mode-start-tag-attribute-names-canonicalized"
            | "tree-construction:unexpected-doctype-token-in-text-mode"
            | "tree-construction:unexpected-end-tag-token-in-text-mode"
            | "tree-construction:non-void-html-self-closing-flag-altered-stack-disposition"
    ) && payload == "none"
}
fn valid_position(value: &str) -> bool {
    if value == "unavailable:parser-did-not-provide-position" {
        return true;
    }
    let Some(rest) = value.strip_prefix("normalized-utf8:") else {
        return false;
    };
    let Some((offset, rest)) = rest.split_once(':') else {
        return false;
    };
    let Some((line, rest)) = rest.split_once(':') else {
        return false;
    };
    let Some((column, source)) = rest.split_once(":source-") else {
        return false;
    };
    validate_u64(offset)
        && validate_u64(line)
        && line != "0"
        && validate_u64(column)
        && column != "0"
        && (source == "unavailable:no-input-provenance-map"
            || source.strip_prefix("exact:").is_some_and(validate_u64))
}
fn valid_context(context: &str, token: &str, mode: &str, namespace: &str) -> bool {
    match context {
        "absent" => token == "null" && mode == "null" && namespace == "null",
        "present" => {
            matches!(
                token,
                "null"
                    | "doctype"
                    | "start-tag"
                    | "end-tag"
                    | "character"
                    | "comment"
                    | "processing-instruction"
                    | "eof"
            ) && matches!(
                mode,
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
            ) && matches!(namespace, "null" | "html" | "svg" | "mathml")
        }
        _ => false,
    }
}
fn malformed<T>(line: usize, reason: &'static str) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord { line, reason })
}

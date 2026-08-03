use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, escape_quoted, fixed_fields, optional_quoted,
    strict_record_lines, validate_nullable_quoted, validate_quoted, validate_u64,
};
use html::conformance::{
    ObservationState, ObservedDomAttribute, ObservedPatchOperation, ObservedPatchStream,
    PatchNodeLabel,
};
use std::collections::BTreeSet;
use std::fmt::Write;

const HEADER: &str = "# format: html5-dompatch-v3";

define_snapshot_types!(ParsedPatchesSnapshot, CanonicalPatchesSnapshot);

pub(super) fn write(
    state: &ObservationState<ObservedPatchStream>,
) -> Result<CanonicalPatchesSnapshot, ()> {
    let ObservationState::Captured(stream) = state else {
        return Err(());
    };
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    for (index, operation) in stream.operations.iter().enumerate() {
        if !operation_labels_are_canonical(operation) {
            return Err(());
        }
        let ordinal = index.checked_add(1).ok_or(())?;
        let line = match operation {
            ObservedPatchOperation::Clear => format!("PATCH operation={ordinal} kind=clear"),
            ObservedPatchOperation::CreateDocument {
                node,
                legacy_doctype,
            } => format!(
                "PATCH operation={ordinal} kind=create-document node={} legacy-doctype={}",
                label(node),
                optional_quoted(legacy_doctype.as_deref())
            ),
            ObservedPatchOperation::CreateDocumentType {
                node,
                name,
                public_id,
                system_id,
            } => format!(
                "PATCH operation={ordinal} kind=create-document-type node={} name={} public-id={} system-id={}",
                label(node),
                optional_quoted(name.as_deref()),
                optional_quoted(public_id.as_deref()),
                optional_quoted(system_id.as_deref())
            ),
            ObservedPatchOperation::CreateElement {
                node,
                namespace,
                local_name,
                ..
            } => format!(
                "PATCH operation={ordinal} kind=create-element node={} namespace={} local-name={}",
                label(node),
                namespace.snapshot_name(),
                escape_quoted(local_name)
            ),
            ObservedPatchOperation::CreateTemplateContents { host, contents } => format!(
                "PATCH operation={ordinal} kind=create-template-contents host={} contents={}",
                label(host),
                label(contents)
            ),
            ObservedPatchOperation::CreateText { node, text } => format!(
                "PATCH operation={ordinal} kind=create-text node={} text={}",
                label(node),
                escape_quoted(text)
            ),
            ObservedPatchOperation::CreateComment { node, data } => format!(
                "PATCH operation={ordinal} kind=create-comment node={} data={}",
                label(node),
                escape_quoted(data)
            ),
            ObservedPatchOperation::CreateProcessingInstruction { node, target, data } => format!(
                "PATCH operation={ordinal} kind=create-processing-instruction node={} target={} data={}",
                label(node),
                escape_quoted(target),
                escape_quoted(data)
            ),
            ObservedPatchOperation::AppendChild { parent, child } => format!(
                "PATCH operation={ordinal} kind=append-child parent={} child={}",
                label(parent),
                label(child)
            ),
            ObservedPatchOperation::InsertBefore {
                parent,
                child,
                before,
            } => format!(
                "PATCH operation={ordinal} kind=insert-before parent={} child={} before={}",
                label(parent),
                label(child),
                label(before)
            ),
            ObservedPatchOperation::RemoveNode { node } => format!(
                "PATCH operation={ordinal} kind=remove-node node={}",
                label(node)
            ),
            ObservedPatchOperation::SetAttributes { node, .. } => format!(
                "PATCH operation={ordinal} kind=set-attributes node={}",
                label(node)
            ),
            ObservedPatchOperation::SetText { node, text } => format!(
                "PATCH operation={ordinal} kind=set-text node={} text={}",
                label(node),
                escape_quoted(text)
            ),
            ObservedPatchOperation::AppendText { node, text } => format!(
                "PATCH operation={ordinal} kind=append-text node={} text={}",
                label(node),
                escape_quoted(text)
            ),
        };
        let _ = writeln!(&mut bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("operation {ordinal}"),
            line,
        });
        match operation {
            ObservedPatchOperation::CreateElement { attributes, .. }
            | ObservedPatchOperation::SetAttributes { attributes, .. } => {
                write_attributes(ordinal, attributes, &mut bytes, &mut records)
            }
            ObservedPatchOperation::Clear
            | ObservedPatchOperation::CreateDocument { .. }
            | ObservedPatchOperation::CreateDocumentType { .. }
            | ObservedPatchOperation::CreateTemplateContents { .. }
            | ObservedPatchOperation::CreateText { .. }
            | ObservedPatchOperation::CreateComment { .. }
            | ObservedPatchOperation::CreateProcessingInstruction { .. }
            | ObservedPatchOperation::AppendChild { .. }
            | ObservedPatchOperation::InsertBefore { .. }
            | ObservedPatchOperation::RemoveNode { .. }
            | ObservedPatchOperation::SetText { .. }
            | ObservedPatchOperation::AppendText { .. } => {}
        }
    }
    Ok(CanonicalPatchesSnapshot::new(SnapshotData::new(
        bytes, records,
    )))
}

fn label(value: &PatchNodeLabel) -> String {
    escape_quoted(&value.0)
}

fn operation_labels_are_canonical(operation: &ObservedPatchOperation) -> bool {
    let valid = |label: &PatchNodeLabel| valid_label_text(&label.0);
    match operation {
        ObservedPatchOperation::Clear => true,
        ObservedPatchOperation::CreateDocument { node, .. }
        | ObservedPatchOperation::CreateDocumentType { node, .. }
        | ObservedPatchOperation::CreateElement { node, .. }
        | ObservedPatchOperation::CreateText { node, .. }
        | ObservedPatchOperation::CreateComment { node, .. }
        | ObservedPatchOperation::CreateProcessingInstruction { node, .. }
        | ObservedPatchOperation::RemoveNode { node }
        | ObservedPatchOperation::SetAttributes { node, .. }
        | ObservedPatchOperation::SetText { node, .. }
        | ObservedPatchOperation::AppendText { node, .. } => valid(node),
        ObservedPatchOperation::CreateTemplateContents { host, contents }
        | ObservedPatchOperation::AppendChild {
            parent: host,
            child: contents,
        } => valid(host) && valid(contents),
        ObservedPatchOperation::InsertBefore {
            parent,
            child,
            before,
        } => valid(parent) && valid(child) && valid(before),
    }
}
fn write_attributes(
    operation: usize,
    attributes: &[ObservedDomAttribute],
    bytes: &mut String,
    records: &mut Vec<SnapshotRecord>,
) {
    for (index, attribute) in attributes.iter().enumerate() {
        let line = format!(
            "PATCH_ATTRIBUTE operation={operation} index={index} namespace={} prefix={} local-name={} value={}",
            attribute.namespace.snapshot_name(),
            optional_quoted(attribute.prefix.as_deref()),
            escape_quoted(&attribute.local_name),
            escape_quoted(&attribute.value)
        );
        let _ = writeln!(bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("operation {operation} attribute {index}"),
            line,
        });
    }
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedPatchesSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, true)?;
    let mut expected_operation = 1u64;
    let mut expected_attribute = None::<(u64, u64)>;
    let mut locations = BTreeSet::new();
    let mut records = Vec::new();
    for (line_number, line) in lines {
        if line.starts_with("PATCH_ATTRIBUTE ") {
            let Some((operation, index)) = expected_attribute else {
                return malformed(
                    line_number,
                    "patch attribute is not grouped under create-element or set-attributes",
                );
            };
            let Some(fields) = fixed_fields(
                line,
                "PATCH_ATTRIBUTE",
                &[
                    "operation",
                    "index",
                    "namespace",
                    "prefix",
                    "local-name",
                    "value",
                ],
            ) else {
                return malformed(line_number, "invalid patch attribute shape");
            };
            if !validate_u64(fields[0])
                || !validate_u64(fields[1])
                || fields[0].parse::<u64>().ok() != Some(operation)
                || fields[1].parse::<u64>().ok() != Some(index)
                || !matches!(fields[2], "none" | "xml" | "xmlns" | "xlink")
                || !validate_nullable_quoted(fields[3])
                || !validate_quoted(fields[4])
                || !validate_quoted(fields[5])
            {
                return Err(SnapshotReadError::NonContiguousOrdinal { line: line_number });
            }
            let location = format!("operation {operation} attribute {index}");
            if !locations.insert(location.clone()) {
                return Err(SnapshotReadError::DuplicateLocation { line: line_number });
            }
            records.push(SnapshotRecord {
                location,
                line: line.to_string(),
            });
            expected_attribute = Some((
                operation,
                index
                    .checked_add(1)
                    .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?,
            ));
            continue;
        }
        expected_attribute = None;
        let mut prefix = line.splitn(4, ' ');
        let (Some("PATCH"), Some(operation_field), Some(kind_field)) =
            (prefix.next(), prefix.next(), prefix.next())
        else {
            return malformed(line_number, "invalid patch prefix");
        };
        let Some(operation) = operation_field.strip_prefix("operation=") else {
            return malformed(line_number, "missing patch operation ordinal");
        };
        let Some(kind) = kind_field.strip_prefix("kind=") else {
            return malformed(line_number, "missing patch kind");
        };
        if !validate_u64(operation) || operation.parse::<u64>().ok() != Some(expected_operation) {
            return Err(SnapshotReadError::NonContiguousOrdinal { line: line_number });
        }
        let valid = match kind {
            "clear" => fixed_fields(line, "PATCH", &["operation", "kind"]).is_some(),
            "create-document" => fixed_fields(
                line,
                "PATCH",
                &["operation", "kind", "node", "legacy-doctype"],
            )
            .is_some_and(|f| validate_label(f[2]) && validate_nullable_quoted(f[3])),
            "create-document-type" => fixed_fields(
                line,
                "PATCH",
                &[
                    "operation",
                    "kind",
                    "node",
                    "name",
                    "public-id",
                    "system-id",
                ],
            )
            .is_some_and(|f| {
                validate_label(f[2])
                    && validate_nullable_quoted(f[3])
                    && validate_nullable_quoted(f[4])
                    && validate_nullable_quoted(f[5])
            }),
            "create-element" => fixed_fields(
                line,
                "PATCH",
                &["operation", "kind", "node", "namespace", "local-name"],
            )
            .is_some_and(|f| {
                validate_label(f[2])
                    && matches!(f[3], "html" | "svg" | "mathml")
                    && validate_quoted(f[4])
            }),
            "create-template-contents" => {
                fixed_fields(line, "PATCH", &["operation", "kind", "host", "contents"])
                    .is_some_and(|f| validate_label(f[2]) && validate_label(f[3]))
            }
            "create-text" | "set-text" | "append-text" => {
                fixed_fields(line, "PATCH", &["operation", "kind", "node", "text"])
                    .is_some_and(|f| validate_label(f[2]) && validate_quoted(f[3]))
            }
            "create-comment" => fixed_fields(line, "PATCH", &["operation", "kind", "node", "data"])
                .is_some_and(|f| validate_label(f[2]) && validate_quoted(f[3])),
            "create-processing-instruction" => fixed_fields(
                line,
                "PATCH",
                &["operation", "kind", "node", "target", "data"],
            )
            .is_some_and(|f| {
                validate_label(f[2]) && validate_quoted(f[3]) && validate_quoted(f[4])
            }),
            "append-child" => {
                fixed_fields(line, "PATCH", &["operation", "kind", "parent", "child"])
                    .is_some_and(|f| validate_label(f[2]) && validate_label(f[3]))
            }
            "insert-before" => fixed_fields(
                line,
                "PATCH",
                &["operation", "kind", "parent", "child", "before"],
            )
            .is_some_and(|f| validate_label(f[2]) && validate_label(f[3]) && validate_label(f[4])),
            "remove-node" | "set-attributes" => {
                fixed_fields(line, "PATCH", &["operation", "kind", "node"])
                    .is_some_and(|f| validate_label(f[2]))
            }
            _ => false,
        };
        if !valid {
            return malformed(line_number, "unknown patch kind or malformed fixed fields");
        }
        let location = format!("operation {expected_operation}");
        if !locations.insert(location.clone()) {
            return Err(SnapshotReadError::DuplicateLocation { line: line_number });
        }
        records.push(SnapshotRecord {
            location,
            line: line.to_string(),
        });
        if matches!(kind, "create-element" | "set-attributes") {
            expected_attribute = Some((expected_operation, 0));
        }
        expected_operation = expected_operation
            .checked_add(1)
            .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?;
    }
    Ok(ParsedPatchesSnapshot::new(SnapshotData::new(
        std::str::from_utf8(bytes)
            .map_err(|_| SnapshotReadError::InvalidUtf8)?
            .to_string(),
        records,
    )))
}

fn validate_label(value: &str) -> bool {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .is_some_and(valid_label_text)
}

fn valid_label_text(value: &str) -> bool {
    let Some(decimal) = value.strip_prefix("node-") else {
        return false;
    };
    validate_u64(decimal) && decimal != "0"
}

fn malformed<T>(line: usize, reason: &'static str) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord { line, reason })
}

use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, escape_quoted, fixed_fields, optional_quoted,
    strict_record_lines, validate_bool, validate_nullable_quoted, validate_quoted, validate_u64,
};
use html::conformance::{ObservationState, ObservedToken};
use std::collections::BTreeSet;
use std::fmt::Write;

const HEADER: &str = "# format: html5-token-v2";

define_snapshot_types!(ParsedTokenSnapshot, CanonicalTokenSnapshot);

pub(super) fn write(
    state: &ObservationState<Vec<ObservedToken>>,
) -> Result<CanonicalTokenSnapshot, ()> {
    let ObservationState::Captured(tokens) = state else {
        return Err(());
    };
    if !matches!(tokens.last(), Some(ObservedToken::Eof))
        || tokens[..tokens.len().saturating_sub(1)]
            .iter()
            .any(|token| matches!(token, ObservedToken::Eof))
    {
        return Err(());
    }
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let ordinal = index.checked_add(1).ok_or(())?;
        let line = match token {
            ObservedToken::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => format!(
                "TOKEN ordinal={ordinal} kind=doctype name={} public-id={} system-id={} force-quirks={force_quirks}",
                optional_quoted(name.as_deref()),
                optional_quoted(public_id.as_deref()),
                optional_quoted(system_id.as_deref())
            ),
            ObservedToken::StartTag {
                name,
                attributes: _,
                self_closing,
            } => format!(
                "TOKEN ordinal={ordinal} kind=start-tag name={} self-closing={self_closing}",
                escape_quoted(name)
            ),
            ObservedToken::EndTag { name } => format!(
                "TOKEN ordinal={ordinal} kind=end-tag name={}",
                escape_quoted(name)
            ),
            ObservedToken::Character { data } => format!(
                "TOKEN ordinal={ordinal} kind=character data={}",
                escape_quoted(data)
            ),
            ObservedToken::Comment { data } => format!(
                "TOKEN ordinal={ordinal} kind=comment data={}",
                escape_quoted(data)
            ),
            ObservedToken::ProcessingInstruction { target, data } => format!(
                "TOKEN ordinal={ordinal} kind=processing-instruction target={} data={}",
                escape_quoted(target),
                escape_quoted(data)
            ),
            ObservedToken::Eof => format!("TOKEN ordinal={ordinal} kind=eof"),
        };
        let _ = writeln!(&mut bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("token {ordinal}"),
            line,
        });
        if let ObservedToken::StartTag { attributes, .. } = token {
            for (attribute_index, attribute) in attributes.iter().enumerate() {
                let line = format!(
                    "TOKEN_ATTRIBUTE token={ordinal} index={attribute_index} name={} value={}",
                    escape_quoted(&attribute.name),
                    escape_quoted(&attribute.value)
                );
                let _ = writeln!(&mut bytes, "{line}");
                records.push(SnapshotRecord {
                    location: format!("token {ordinal} attribute {attribute_index}"),
                    line,
                });
            }
        }
    }
    Ok(CanonicalTokenSnapshot::new(SnapshotData::new(
        bytes, records,
    )))
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedTokenSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, false)?;
    let mut expected_ordinal = 1u64;
    let mut expected_attribute = None::<(u64, u64)>;
    let mut saw_eof = false;
    let mut locations = BTreeSet::new();
    let mut records = Vec::new();
    for (line_number, line) in lines {
        if saw_eof {
            return Err(SnapshotReadError::TrailingContent { line: line_number });
        }
        if line.starts_with("TOKEN_ATTRIBUTE ") {
            let Some((token, index)) = expected_attribute else {
                return malformed(
                    line_number,
                    "attribute record is not grouped below a start tag",
                );
            };
            let Some(fields) = fixed_fields(
                line,
                "TOKEN_ATTRIBUTE",
                &["token", "index", "name", "value"],
            ) else {
                return malformed(line_number, "invalid token attribute shape");
            };
            if !validate_u64(fields[0])
                || !validate_u64(fields[1])
                || !validate_quoted(fields[2])
                || !validate_quoted(fields[3])
            {
                return malformed(line_number, "invalid token attribute field");
            }
            if fields[0].parse::<u64>().ok() != Some(token)
                || fields[1].parse::<u64>().ok() != Some(index)
            {
                return Err(SnapshotReadError::NonContiguousOrdinal { line: line_number });
            }
            let location = format!("token {token} attribute {index}");
            if !locations.insert(location.clone()) {
                return Err(SnapshotReadError::DuplicateLocation { line: line_number });
            }
            records.push(SnapshotRecord {
                location,
                line: line.to_string(),
            });
            expected_attribute = Some((
                token,
                index
                    .checked_add(1)
                    .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?,
            ));
            continue;
        }

        expected_attribute = None;
        let mut prefix = line.splitn(4, ' ');
        let (Some("TOKEN"), Some(ordinal_field), Some(kind_field)) =
            (prefix.next(), prefix.next(), prefix.next())
        else {
            return malformed(line_number, "invalid token record prefix");
        };
        let Some(ordinal) = ordinal_field.strip_prefix("ordinal=") else {
            return malformed(line_number, "invalid token ordinal field");
        };
        let Some(kind) = kind_field.strip_prefix("kind=") else {
            return malformed(line_number, "invalid token kind field");
        };
        if !validate_u64(ordinal) || ordinal.parse::<u64>().ok() != Some(expected_ordinal) {
            return Err(SnapshotReadError::NonContiguousOrdinal { line: line_number });
        }
        let valid = match kind {
            "doctype" => fixed_fields(
                line,
                "TOKEN",
                &[
                    "ordinal",
                    "kind",
                    "name",
                    "public-id",
                    "system-id",
                    "force-quirks",
                ],
            )
            .is_some_and(|f| {
                validate_nullable_quoted(f[2])
                    && validate_nullable_quoted(f[3])
                    && validate_nullable_quoted(f[4])
                    && validate_bool(f[5])
            }),
            "start-tag" => {
                fixed_fields(line, "TOKEN", &["ordinal", "kind", "name", "self-closing"])
                    .is_some_and(|f| validate_quoted(f[2]) && validate_bool(f[3]))
            }
            "end-tag" => fixed_fields(line, "TOKEN", &["ordinal", "kind", "name"])
                .is_some_and(|f| validate_quoted(f[2])),
            "character" | "comment" => fixed_fields(line, "TOKEN", &["ordinal", "kind", "data"])
                .is_some_and(|f| validate_quoted(f[2])),
            "processing-instruction" => {
                fixed_fields(line, "TOKEN", &["ordinal", "kind", "target", "data"])
                    .is_some_and(|f| validate_quoted(f[2]) && validate_quoted(f[3]))
            }
            "eof" => fixed_fields(line, "TOKEN", &["ordinal", "kind"]).is_some(),
            _ => false,
        };
        if !valid {
            return malformed(line_number, "unknown token kind or invalid fixed fields");
        }
        let location = format!("token {expected_ordinal}");
        if !locations.insert(location.clone()) {
            return Err(SnapshotReadError::DuplicateLocation { line: line_number });
        }
        records.push(SnapshotRecord {
            location,
            line: line.to_string(),
        });
        if kind == "start-tag" {
            expected_attribute = Some((expected_ordinal, 0));
        } else if kind == "eof" {
            saw_eof = true;
        }
        expected_ordinal = expected_ordinal
            .checked_add(1)
            .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?;
    }
    if !saw_eof {
        return malformed(1, "token snapshot requires one final EOF record");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SnapshotReadError::InvalidUtf8)?;
    Ok(ParsedTokenSnapshot::new(SnapshotData::new(
        text.to_string(),
        records,
    )))
}

fn malformed<T>(line: usize, reason: &'static str) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord { line, reason })
}

use std::fmt::Write;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SnapshotRecord {
    pub(crate) location: String,
    pub(crate) line: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoredSnapshotRecord {
    pub(crate) location: String,
    pub(crate) line: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotReadError {
    InvalidUtf8,
    BomNotAllowed,
    CarriageReturnNotAllowed,
    MissingTerminalLf,
    InvalidHeader,
    HeaderOnlyNotAllowed,
    BlankLine { line: usize },
    CommentNotAllowed { line: usize },
    MalformedRecord { line: usize, reason: &'static str },
    DuplicateLocation { line: usize },
    NonContiguousOrdinal { line: usize },
    TrailingContent { line: usize },
}

impl std::fmt::Display for SnapshotReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUtf8 => f.write_str("snapshot must be valid UTF-8"),
            Self::BomNotAllowed => f.write_str("snapshot must not begin with a UTF-8 BOM"),
            Self::CarriageReturnNotAllowed => {
                f.write_str("snapshot must use LF line endings; carriage return is forbidden")
            }
            Self::MissingTerminalLf => f.write_str("snapshot must end with exactly one LF"),
            Self::InvalidHeader => f.write_str("snapshot format header is missing or incorrect"),
            Self::HeaderOnlyNotAllowed => {
                f.write_str("this snapshot surface requires at least one record")
            }
            Self::BlankLine { line } => write!(f, "blank physical line at line {line}"),
            Self::CommentNotAllowed { line } => write!(f, "comment is forbidden at line {line}"),
            Self::MalformedRecord { line, reason } => {
                write!(f, "malformed snapshot record at line {line}: {reason}")
            }
            Self::DuplicateLocation { line } => {
                write!(f, "duplicate snapshot record location at line {line}")
            }
            Self::NonContiguousOrdinal { line } => {
                write!(f, "non-contiguous snapshot ordinal at line {line}")
            }
            Self::TrailingContent { line } => {
                write!(f, "snapshot contains trailing content at line {line}")
            }
        }
    }
}

impl std::error::Error for SnapshotReadError {}

pub(crate) fn strict_record_lines<'a>(
    bytes: &'a [u8],
    header: &str,
    header_only_allowed: bool,
) -> Result<Vec<(usize, &'a str)>, SnapshotReadError> {
    let text = std::str::from_utf8(bytes).map_err(|_| SnapshotReadError::InvalidUtf8)?;
    if text.starts_with('\u{feff}') {
        return Err(SnapshotReadError::BomNotAllowed);
    }
    if text.contains('\r') {
        return Err(SnapshotReadError::CarriageReturnNotAllowed);
    }
    if !text.ends_with('\n') || text.ends_with("\n\n") {
        return Err(SnapshotReadError::MissingTerminalLf);
    }
    let mut lines = text
        .strip_suffix('\n')
        .expect("terminal LF checked")
        .split('\n');
    if lines.next() != Some(header) {
        return Err(SnapshotReadError::InvalidHeader);
    }
    let records = lines
        .enumerate()
        .map(|(index, line)| (index + 2, line))
        .collect::<Vec<_>>();
    if records.is_empty() && !header_only_allowed {
        return Err(SnapshotReadError::HeaderOnlyNotAllowed);
    }
    for (line_number, line) in &records {
        if line.is_empty() {
            return Err(SnapshotReadError::BlankLine { line: *line_number });
        }
        if line.starts_with('#') {
            return Err(SnapshotReadError::CommentNotAllowed { line: *line_number });
        }
        if line.starts_with(' ') || line.ends_with(' ') || line.contains("  ") {
            return Err(SnapshotReadError::MalformedRecord {
                line: *line_number,
                reason: "records use one ASCII space and no surrounding whitespace",
            });
        }
    }
    Ok(records)
}

pub(crate) fn escape_quoted(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('"');
    for ch in value.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\0'..='\u{0008}' | '\u{000b}' | '\u{000c}' | '\u{000e}'..='\u{001f}' | '\u{007f}' => {
                let _ = write!(&mut result, "\\u{:04X}", u32::from(ch));
            }
            _ => result.push(ch),
        }
    }
    result.push('"');
    result
}

pub(crate) fn optional_quoted(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_string(), escape_quoted)
}

pub(crate) fn validate_quoted(value: &str) -> bool {
    let Some(mut rest) = value.strip_prefix('"') else {
        return false;
    };
    loop {
        let Some(ch) = rest.chars().next() else {
            return false;
        };
        rest = &rest[ch.len_utf8()..];
        match ch {
            '"' => return rest.is_empty(),
            '\\' => {
                let Some(escape) = rest.chars().next() else {
                    return false;
                };
                rest = &rest[escape.len_utf8()..];
                match escape {
                    '"' | '\\' | 'n' | 'r' | 't' => {}
                    'u' => {
                        if rest.len() < 4 {
                            return false;
                        }
                        let digits = &rest[..4];
                        if !digits
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
                        {
                            return false;
                        }
                        let Ok(scalar) = u32::from_str_radix(digits, 16) else {
                            return false;
                        };
                        let Some(scalar) = char::from_u32(scalar) else {
                            return false;
                        };
                        if !matches!(scalar, '\0'..='\u{0008}' | '\u{000b}' | '\u{000c}' | '\u{000e}'..='\u{001f}' | '\u{007f}')
                        {
                            return false;
                        }
                        rest = &rest[4..];
                    }
                    _ => return false,
                }
            }
            '\0'..='\u{001f}' | '\u{007f}' => return false,
            _ => {}
        }
    }
}

pub(crate) fn validate_nullable_quoted(value: &str) -> bool {
    value == "null" || validate_quoted(value)
}

pub(crate) fn validate_u64(value: &str) -> bool {
    value == "0"
        || (!value.starts_with('0')
            && !value.is_empty()
            && value.bytes().all(|b| b.is_ascii_digit()))
}

pub(crate) fn validate_bool(value: &str) -> bool {
    matches!(value, "true" | "false")
}

/// Split a fixed-order record without interpreting its semantic payload.
pub(crate) fn fixed_fields<'a>(line: &'a str, record: &str, keys: &[&str]) -> Option<Vec<&'a str>> {
    let mut rest = line.strip_prefix(record)?;
    let mut values = Vec::with_capacity(keys.len());
    for key in keys {
        rest = rest.strip_prefix(' ')?;
        rest = rest.strip_prefix(key)?.strip_prefix('=')?;
        let (value, tail) = consume_field(rest)?;
        values.push(value);
        rest = tail;
    }
    rest.is_empty().then_some(values)
}

fn consume_field(value: &str) -> Option<(&str, &str)> {
    if value.starts_with('"') {
        let mut escaped = false;
        for (index, ch) in value.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some((&value[..=index], &value[index + 1..]));
            }
        }
        None
    } else {
        let end = value.find(' ').unwrap_or(value.len());
        Some((&value[..end], &value[end..]))
    }
}

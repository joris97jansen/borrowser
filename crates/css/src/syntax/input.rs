//! CSS syntax input primitives.
//!
//! This module defines the decoded source-text abstraction used by the CSS
//! syntax layer. The input model is append-only, source-bound, and deterministic
//! for parser/debug use.

use std::sync::atomic::{AtomicU64, Ordering};

/// Opaque identity for one CSS input buffer instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CssInputId(u64);

impl CssInputId {
    fn next() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Byte span into a decoded CSS input buffer.
///
/// Invariants:
/// - `input_id` identifies the owning `CssInput`
/// - `start <= end`
/// - both bounds are UTF-8 character boundaries in the owning `CssInput`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CssSpan {
    pub input_id: CssInputId,
    pub start: usize,
    pub end: usize,
}

impl CssSpan {
    pub fn new(input_id: CssInputId, start: usize, end: usize) -> Option<Self> {
        if start <= end {
            Some(Self {
                input_id,
                start,
                end,
            })
        } else {
            None
        }
    }

    pub fn len_bytes(self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Source position for parser diagnostics and debug output.
///
/// `line` and `column` are 1-based. `column` counts Unicode scalar values
/// within the line rather than raw bytes.
///
/// Line-boundary policy:
/// - `\n`, `\r`, and `\u{000C}` are line breaks
/// - `\r\n` is treated as one logical line break
/// - positions that point into a line-break sequence are reported on the
///   preceding line, at the column immediately after the line's last non-break
///   scalar
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssPosition {
    pub byte_offset: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug)]
struct CssLineRecord {
    start: usize,
    break_start: usize,
    break_end: usize,
}

/// Decoded CSS source text.
///
/// Invariant: the internal buffer is append-only while spans derived from it
/// are in use.
#[derive(Clone, Debug)]
pub struct CssInput {
    id: CssInputId,
    buffer: String,
    line_records: Vec<CssLineRecord>,
}

impl CssInput {
    pub fn new() -> Self {
        Self {
            id: CssInputId::next(),
            buffer: String::new(),
            line_records: vec![CssLineRecord {
                start: 0,
                break_start: 0,
                break_end: 0,
            }],
        }
    }

    pub fn from_string(buffer: String) -> Self {
        let mut input = Self {
            id: CssInputId::next(),
            buffer,
            line_records: Vec::new(),
        };
        input.rebuild_line_records();
        input
    }

    pub fn id(&self) -> CssInputId {
        self.id
    }

    pub fn push_str(&mut self, text: &str) {
        self.buffer.push_str(text);
        if !text.is_empty() {
            self.rebuild_line_records();
        }
    }

    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    pub fn len_bytes(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn span(&self, start: usize, end: usize) -> Option<CssSpan> {
        let span = CssSpan::new(self.id, start, end)?;
        if span.end > self.buffer.len() {
            return None;
        }
        if !self.buffer.is_char_boundary(span.start) || !self.buffer.is_char_boundary(span.end) {
            return None;
        }
        Some(span)
    }

    pub fn slice(&self, span: CssSpan) -> Option<&str> {
        if span.input_id != self.id {
            return None;
        }
        self.buffer.get(span.start..span.end)
    }

    pub fn position(&self, byte_offset: usize) -> Option<CssPosition> {
        if byte_offset > self.buffer.len() || !self.buffer.is_char_boundary(byte_offset) {
            return None;
        }

        let line_index = match self
            .line_records
            .binary_search_by_key(&byte_offset, |record| record.start)
        {
            Ok(index) => index,
            Err(insert_index) => insert_index.saturating_sub(1),
        };
        let line_record = *self.line_records.get(line_index)?;
        let effective_offset = if byte_offset < line_record.break_end {
            byte_offset.min(line_record.break_start)
        } else {
            byte_offset
        };
        let line_slice = self.buffer.get(line_record.start..effective_offset)?;

        Some(CssPosition {
            byte_offset,
            line: line_index + 1,
            column: line_slice.chars().count() + 1,
        })
    }

    fn rebuild_line_records(&mut self) {
        self.line_records.clear();

        let mut line_start = 0usize;
        let mut chars = self.buffer.char_indices().peekable();

        while let Some((offset, ch)) = chars.next() {
            let break_end = match ch {
                '\r' => {
                    if let Some(&(next_offset, '\n')) = chars.peek() {
                        chars.next();
                        next_offset + '\n'.len_utf8()
                    } else {
                        offset + ch.len_utf8()
                    }
                }
                '\n' | '\u{000C}' => offset + ch.len_utf8(),
                _ => continue,
            };

            self.line_records.push(CssLineRecord {
                start: line_start,
                break_start: offset,
                break_end,
            });
            line_start = break_end;
        }

        self.line_records.push(CssLineRecord {
            start: line_start,
            break_start: self.buffer.len(),
            break_end: self.buffer.len(),
        });
    }
}

#[cfg(test)]
impl CssInput {
    fn line_count(&self) -> usize {
        self.line_records.len()
    }

    fn line_start(&self, line_index: usize) -> Option<usize> {
        self.line_records.get(line_index).map(|record| record.start)
    }
}

impl Default for CssInput {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&str> for CssInput {
    fn from(value: &str) -> Self {
        Self::from_string(value.to_string())
    }
}

impl From<String> for CssInput {
    fn from(value: String) -> Self {
        Self::from_string(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{CssInput, CssPosition};

    #[test]
    fn input_spans_are_checked_and_bound_to_their_owner() {
        let mut input = CssInput::new();
        input.push_str("body");
        input.push_str(" {\n");
        input.push_str("  color: red;");

        let span = input.span(9, 14).expect("valid span");
        assert_eq!(input.slice(span), Some("color"));
        assert!(input.span(100, 101).is_none());
        assert!(input.span(4, 3).is_none());

        let other = CssInput::from("  color: red;");
        assert_eq!(other.slice(span), None);
    }

    #[test]
    fn input_positions_are_line_and_scalar_column_based() {
        let input = CssInput::from("a\nβz\n");

        assert_eq!(
            input.position(0),
            Some(CssPosition {
                byte_offset: 0,
                line: 1,
                column: 1,
            })
        );
        assert_eq!(
            input.position(2),
            Some(CssPosition {
                byte_offset: 2,
                line: 2,
                column: 1,
            })
        );
        assert_eq!(
            input.position(4),
            Some(CssPosition {
                byte_offset: 4,
                line: 2,
                column: 2,
            })
        );
    }

    #[test]
    fn input_positions_treat_css_line_break_forms_consistently() {
        let input = CssInput::from("a\r\nb\rc\u{000C}d");

        assert_eq!(input.line_count(), 4);
        assert_eq!(input.line_start(0), Some(0));
        assert_eq!(input.line_start(1), Some(3));
        assert_eq!(input.line_start(2), Some(5));
        assert_eq!(input.line_start(3), Some(7));

        assert_eq!(
            input.position(2),
            Some(CssPosition {
                byte_offset: 2,
                line: 1,
                column: 2,
            })
        );
        assert_eq!(
            input.position(3),
            Some(CssPosition {
                byte_offset: 3,
                line: 2,
                column: 1,
            })
        );
        assert_eq!(
            input.position(5),
            Some(CssPosition {
                byte_offset: 5,
                line: 3,
                column: 1,
            })
        );
        assert_eq!(
            input.position(7),
            Some(CssPosition {
                byte_offset: 7,
                line: 4,
                column: 1,
            })
        );
    }

    #[test]
    fn input_rejects_non_boundary_offsets() {
        let input = CssInput::from("β");
        assert!(input.position(1).is_none());
        assert!(input.span(0, 1).is_none());
    }
}

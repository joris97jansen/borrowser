use super::scan::{
    RAWTEXT_TAIL_SLACK, SCRIPT_CLOSE_TAG, STYLE_CLOSE_TAG, clamp_char_boundary,
    find_rawtext_close_tag_internal, is_name_char, is_void_element,
};
use super::{ParseOutcome, PendingState, Tokenizer};
use crate::types::{AtomId, AttributeValue, Token};

type ParsedAttribute = (AtomId, Option<AttributeValue>);
type ParsedAttributes = (Vec<ParsedAttribute>, bool, usize);

impl Tokenizer {
    pub(crate) fn parse_start_tag(&mut self, is_final: bool) -> ParseOutcome {
        let (name, mut cursor_after_name) = match self.parse_tag_name(is_final) {
            Ok(result) => result,
            Err(outcome) => return outcome,
        };

        let (attributes, mut self_closing, cursor_after_attrs) =
            match self.parse_attributes(cursor_after_name, is_final) {
                Ok(result) => result,
                Err(outcome) => return outcome,
            };
        cursor_after_name = cursor_after_attrs;

        let input = self.source.as_str();
        let bytes = input.as_bytes();
        let len = bytes.len();

        if is_void_element(self.atoms.resolve(name)) {
            self_closing = true;
        }

        if cursor_after_name < len && bytes[cursor_after_name] == b'>' {
            cursor_after_name += 1;
        }
        let content_start = cursor_after_name;

        self.tokens.push(Token::StartTag {
            name,
            attributes,
            self_closing,
        });

        if let Some(outcome) =
            self.enter_rawtext_if_needed(name, content_start, self_closing, is_final)
        {
            return outcome;
        }

        self.cursor = content_start;
        ParseOutcome::Complete
    }

    fn parse_tag_name(&mut self, is_final: bool) -> Result<(AtomId, usize), ParseOutcome> {
        let input = self.source.as_str();
        let bytes = input.as_bytes();
        let len = bytes.len();
        let start = self.cursor + 1;
        let mut end = start;
        while end < len && is_name_char(bytes[end]) {
            end += 1;
        }
        if end == start {
            if !is_final {
                return Err(ParseOutcome::Incomplete);
            }
            self.emit_raw_text_span(self.cursor, (self.cursor + 1).min(len));
            self.cursor = (self.cursor + 1).min(len);
            return Err(ParseOutcome::Complete);
        }
        if end == len && !is_final {
            return Err(ParseOutcome::Incomplete);
        }
        Ok((self.atoms.intern_ascii_lowercase(&input[start..end]), end))
    }

    fn parse_attributes(
        &mut self,
        mut cursor: usize,
        is_final: bool,
    ) -> Result<ParsedAttributes, ParseOutcome> {
        let input = self.source.as_str();
        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut attributes: Vec<ParsedAttribute> = Vec::with_capacity(4);
        let mut self_closing = false;

        loop {
            skip_ascii_whitespace(bytes, &mut cursor);
            if cursor >= len {
                if is_final {
                    break;
                }
                return Err(ParseOutcome::Incomplete);
            }
            if bytes[cursor] == b'>' {
                cursor += 1;
                break;
            }
            if bytes[cursor] == b'/' {
                if cursor + 1 >= len {
                    if is_final {
                        cursor += 1;
                        continue;
                    }
                    return Err(ParseOutcome::Incomplete);
                }
                if bytes[cursor + 1] == b'>' {
                    self_closing = true;
                    cursor += 2;
                    break;
                }
                cursor += 1;
                continue;
            }

            let name_start = cursor;
            while cursor < len && is_name_char(bytes[cursor]) {
                cursor += 1;
            }
            if name_start == cursor {
                if cursor >= len && !is_final {
                    return Err(ParseOutcome::Incomplete);
                }
                cursor += 1;
                continue;
            }
            let attribute_name = self
                .atoms
                .intern_ascii_lowercase(&input[name_start..cursor]);

            skip_ascii_whitespace(bytes, &mut cursor);
            if cursor >= len {
                if is_final {
                    attributes.push((attribute_name, None));
                    break;
                }
                return Err(ParseOutcome::Incomplete);
            }

            let value = if bytes[cursor] == b'=' {
                cursor += 1;
                skip_ascii_whitespace(bytes, &mut cursor);
                let (value, next_cursor) = self.parse_attribute_value(cursor, is_final)?;
                cursor = next_cursor;
                Some(value)
            } else {
                None
            };

            attributes.push((attribute_name, value));
        }

        Ok((attributes, self_closing, cursor))
    }

    fn parse_attribute_value(
        &self,
        mut cursor: usize,
        is_final: bool,
    ) -> Result<(AttributeValue, usize), ParseOutcome> {
        let input = self.source.as_str();
        let bytes = input.as_bytes();
        let len = bytes.len();
        if cursor >= len {
            if is_final {
                return Ok((
                    AttributeValue::Span {
                        range: cursor..cursor,
                    },
                    cursor,
                ));
            }
            return Err(ParseOutcome::Incomplete);
        }

        if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
            let quote = bytes[cursor];
            cursor += 1;
            let value_start = cursor;
            while cursor < len && bytes[cursor] != quote {
                cursor += 1;
            }
            if cursor >= len && !is_final {
                return Err(ParseOutcome::Incomplete);
            }
            let value_end = cursor.min(len);
            let raw = &input[value_start..value_end];
            if cursor < len {
                cursor += 1;
            }
            return Ok((
                self.decode_attribute_value(raw, value_start, value_end),
                cursor,
            ));
        }

        let value_start = cursor;
        while cursor < len && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'>' {
            if bytes[cursor] == b'/' && cursor + 1 < len && bytes[cursor + 1] == b'>' {
                break;
            }
            cursor += 1;
        }
        if cursor == len && !is_final {
            return Err(ParseOutcome::Incomplete);
        }
        if cursor > value_start {
            let raw = &input[value_start..cursor];
            Ok((
                self.decode_attribute_value(raw, value_start, cursor),
                cursor,
            ))
        } else {
            Ok((
                AttributeValue::Span {
                    range: value_start..value_start,
                },
                cursor,
            ))
        }
    }

    fn enter_rawtext_if_needed(
        &mut self,
        name: AtomId,
        content_start: usize,
        self_closing: bool,
        is_final: bool,
    ) -> Option<ParseOutcome> {
        if self_closing {
            return None;
        }

        let input = self.source.as_str();
        let bytes = input.as_bytes();
        let input_len = input.len();
        let close_tag = match self.atoms.resolve(name) {
            "script" => SCRIPT_CLOSE_TAG,
            "style" => STYLE_CLOSE_TAG,
            _ => return None,
        };

        #[cfg(test)]
        let mut ops = 0usize;
        #[cfg(test)]
        let found =
            find_rawtext_close_tag_internal(&bytes[content_start..], close_tag, Some(&mut ops));
        #[cfg(test)]
        {
            self.rawtext_scan_steps = self.rawtext_scan_steps.saturating_add(ops);
        }
        #[cfg(not(test))]
        let found = find_rawtext_close_tag_internal(&bytes[content_start..], close_tag, None);
        if let Some((rel_start, rel_end)) = found {
            let slice_end = content_start + rel_start;
            self.emit_raw_text_span(content_start, slice_end);
            self.tokens.push(Token::EndTag(name));
            self.cursor = content_start + rel_end;
            return Some(ParseOutcome::Complete);
        }
        if is_final {
            self.emit_raw_text_span(content_start, input_len);
            self.tokens.push(Token::EndTag(name));
            self.cursor = input_len;
            return Some(ParseOutcome::Complete);
        }
        let scan_from = clamp_char_boundary(
            input,
            input_len
                .saturating_sub(close_tag.len() + RAWTEXT_TAIL_SLACK)
                .max(content_start),
            content_start,
        );
        // Cursor jumps to the end while rawtext scanning is pending; the close-tag
        // search resumes from `scan_from` on the next chunk.
        self.cursor = input.len();
        self.pending = PendingState::Rawtext {
            tag: name,
            close_tag,
            content_start,
            scan_from,
            prev_len: input_len,
        };
        Some(ParseOutcome::Complete)
    }
}

#[inline]
fn skip_ascii_whitespace(bytes: &[u8], cursor: &mut usize) {
    while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
        *cursor += 1;
    }
}

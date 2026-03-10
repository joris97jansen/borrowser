use super::scan::{
    HTML_COMMENT_END, HTML_COMMENT_START, RAWTEXT_TAIL_SLACK, clamp_char_boundary,
    find_rawtext_close_tag_internal, is_name_char, starts_with_ignore_ascii_case_at, trim_range,
};
use super::{ParseOutcome, PendingState, Tokenizer};
use crate::types::{TextPayload, Token};
use memchr::{memchr, memrchr};

impl Tokenizer {
    pub(crate) fn scan(&mut self, is_final: bool) -> usize {
        let start_len = self.tokens.len();
        loop {
            if !self.resume_pending(is_final) {
                break;
            }
            let input = self.source.as_str();
            let bytes = input.as_bytes();
            let len = bytes.len();
            if self.cursor >= len {
                break;
            }
            if bytes[self.cursor] != b'<' {
                self.pending = PendingState::Text {
                    start: self.cursor,
                    scan_from: self.cursor,
                };
                continue;
            }
            if !is_final && is_partial_markup_prefix(bytes, self.cursor) {
                break;
            }
            if input[self.cursor..].starts_with(HTML_COMMENT_START) {
                let comment_start = self.cursor + HTML_COMMENT_START.len();
                if let Some(end) = input[comment_start..].find(HTML_COMMENT_END) {
                    let comment_end = comment_start + end;
                    self.tokens.push(Token::Comment(TextPayload::Span {
                        range: comment_start..comment_end,
                    }));
                    self.cursor = comment_end + HTML_COMMENT_END.len();
                    continue;
                }
                if is_final {
                    self.tokens.push(Token::Comment(TextPayload::Span {
                        range: comment_start..len,
                    }));
                    self.cursor = len;
                    continue;
                }
                // Scan near the tail to catch "--" + ">" overlaps across chunk boundaries.
                let scan_from = (len.saturating_sub(HTML_COMMENT_END.len() - 1)).max(comment_start);
                self.pending = PendingState::Comment {
                    start: self.cursor,
                    scan_from,
                };
                break;
            }
            if starts_with_ignore_ascii_case_at(bytes, self.cursor, b"<!doctype") {
                let doctype_start = self.cursor + 2;
                if let Some(rel) = memchr(b'>', &bytes[doctype_start..]) {
                    let end = doctype_start + rel;
                    let (tstart, tend) = trim_range(input, doctype_start, end);
                    self.tokens.push(Token::Doctype(TextPayload::Span {
                        range: tstart..tend,
                    }));
                    self.cursor = end + 1;
                    continue;
                }
                if is_final {
                    self.cursor = len;
                } else {
                    let scan_from = len.saturating_sub(1).max(doctype_start);
                    self.pending = PendingState::Doctype {
                        doctype_start,
                        scan_from,
                    };
                }
                break;
            }
            if self.cursor + 2 <= len && bytes[self.cursor + 1] == b'/' {
                let start = self.cursor + 2;
                let mut end = start;
                while end < len && is_name_char(bytes[end]) {
                    end += 1;
                }
                if end == start {
                    if !is_final {
                        break;
                    }
                    self.emit_raw_text_span(self.cursor, (self.cursor + 1).min(len));
                    self.cursor = (self.cursor + 1).min(len);
                    continue;
                }
                if end == len && !is_final {
                    break;
                }
                let name = self.atoms.intern_ascii_lowercase(&input[start..end]);
                // NOTE: we accept `</div foo>` and ignore extra junk until `>`.
                while end < len && bytes[end] != b'>' {
                    end += 1;
                }
                if end == len && !is_final {
                    break;
                }
                if end < len {
                    end += 1;
                }
                self.tokens.push(Token::EndTag(name));
                self.cursor = end;
                continue;
            }
            match self.parse_start_tag(is_final) {
                ParseOutcome::Complete => continue,
                ParseOutcome::Incomplete => break,
            }
        }
        self.tokens.len() - start_len
    }

    fn resume_pending(&mut self, is_final: bool) -> bool {
        match self.pending {
            PendingState::None => true,
            PendingState::Text { start, scan_from } => {
                let input = self.source.as_str();
                let bytes = input.as_bytes();
                let len = bytes.len();
                if let Some(rel) = memchr(b'<', &bytes[scan_from..]) {
                    let end = scan_from + rel;
                    self.emit_text(start, end);
                    self.cursor = end;
                    self.pending = PendingState::None;
                    return true;
                }
                if is_final {
                    self.emit_text(start, len);
                    self.cursor = len;
                    self.pending = PendingState::None;
                    return true;
                }
                self.pending = PendingState::Text {
                    start,
                    scan_from: len,
                };
                false
            }
            PendingState::Comment { start, scan_from } => {
                let input = self.source.as_str();
                let len = input.len();
                let comment_start = start + HTML_COMMENT_START.len();
                if let Some(rel) = input[scan_from..].find(HTML_COMMENT_END) {
                    let comment_end = scan_from + rel;
                    self.tokens.push(Token::Comment(TextPayload::Span {
                        range: comment_start..comment_end,
                    }));
                    self.cursor = comment_end + HTML_COMMENT_END.len();
                    self.pending = PendingState::None;
                    return true;
                }
                if is_final {
                    self.tokens.push(Token::Comment(TextPayload::Span {
                        range: comment_start..len,
                    }));
                    self.cursor = len;
                    self.pending = PendingState::None;
                    return true;
                }
                let scan_from = (len.saturating_sub(HTML_COMMENT_END.len() - 1)).max(comment_start);
                self.pending = PendingState::Comment { start, scan_from };
                false
            }
            PendingState::Doctype {
                doctype_start,
                scan_from,
            } => {
                let input = self.source.as_str();
                let bytes = input.as_bytes();
                let len = bytes.len();
                if let Some(rel) = memchr(b'>', &bytes[scan_from..]) {
                    let end = scan_from + rel;
                    let (tstart, tend) = trim_range(input, doctype_start, end);
                    self.tokens.push(Token::Doctype(TextPayload::Span {
                        range: tstart..tend,
                    }));
                    self.cursor = end + 1;
                    self.pending = PendingState::None;
                    return true;
                }
                if is_final {
                    self.cursor = len;
                    self.pending = PendingState::None;
                    return true;
                }
                let scan_from = len.saturating_sub(1).max(doctype_start);
                self.pending = PendingState::Doctype {
                    doctype_start,
                    scan_from,
                };
                false
            }
            PendingState::Rawtext {
                tag,
                close_tag,
                content_start,
                scan_from,
                prev_len,
            } => {
                let input = self.source.as_str();
                let len = input.len();
                let scan_from = clamp_char_boundary(input, scan_from, content_start);
                let bytes = input.as_bytes();
                #[cfg(test)]
                let mut ops = 0usize;
                #[cfg(test)]
                let found =
                    find_rawtext_close_tag_internal(&bytes[scan_from..], close_tag, Some(&mut ops));
                #[cfg(test)]
                {
                    self.rawtext_scan_steps = self.rawtext_scan_steps.saturating_add(ops);
                }
                #[cfg(not(test))]
                let found = find_rawtext_close_tag_internal(&bytes[scan_from..], close_tag, None);
                if let Some((rel_start, rel_end)) = found {
                    let slice_end = scan_from + rel_start;
                    self.emit_raw_text_span(content_start, slice_end);
                    self.tokens.push(Token::EndTag(tag));
                    self.cursor = scan_from + rel_end;
                    self.pending = PendingState::None;
                    return true;
                }
                if is_final {
                    self.emit_raw_text_span(content_start, len);
                    self.tokens.push(Token::EndTag(tag));
                    self.cursor = len;
                    self.pending = PendingState::None;
                    return true;
                }
                // Rescan from the last possible '<' that could begin a closing tag spanning the
                // previous/new buffer boundary. This bounds overlap to at most (close_tag.len() + 1)
                // bytes from the prior buffer so we stay linear even with tiny chunks.
                let tail_start = prev_len
                    .saturating_sub(close_tag.len() + RAWTEXT_TAIL_SLACK)
                    .max(content_start);
                let scan_from = memrchr(b'<', &bytes[tail_start..len])
                    .map(|rel| tail_start + rel)
                    .unwrap_or(tail_start);
                self.pending = PendingState::Rawtext {
                    tag,
                    close_tag,
                    content_start,
                    scan_from,
                    prev_len: len,
                };
                false
            }
        }
    }
}

fn is_partial_markup_prefix(bytes: &[u8], start: usize) -> bool {
    // Heuristic: avoid consuming '<' when the chunk may end mid-construct.
    // Full parsing still handles other incomplete cases.
    let remaining = bytes.len().saturating_sub(start);
    if remaining < 2 {
        return true;
    }
    is_partial_prefix(bytes, start, HTML_COMMENT_START.as_bytes())
        || is_partial_prefix_case_insensitive(bytes, start, b"<!doctype")
        || is_partial_prefix(bytes, start, b"</")
}

fn is_partial_prefix(bytes: &[u8], start: usize, needle: &[u8]) -> bool {
    let remaining = bytes.len().saturating_sub(start);
    remaining < needle.len() && needle[..remaining] == bytes[start..start + remaining]
}

fn is_partial_prefix_case_insensitive(bytes: &[u8], start: usize, needle: &[u8]) -> bool {
    let remaining = bytes.len().saturating_sub(start);
    remaining < needle.len()
        && needle[..remaining].eq_ignore_ascii_case(&bytes[start..start + remaining])
}

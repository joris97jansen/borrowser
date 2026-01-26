//! Simplified HTML tokenizer with a constrained, practical tag-name character set.
//!
//! Supported tag-name characters (ASCII only): `[A-Za-z0-9:_-]`.
//! Attribute names use the same ASCII character class.
//!
//! This is not a full HTML5 tokenizer/state machine yet. The constraint is intentional to keep
//! tokenization fast and allocation-light while the DOM pipeline is still evolving, and to defer
//! the complexity of the HTML5 parsing algorithm until a dedicated state machine lands.
//!
//! Known limitations (intentional):
//! - Not a full HTML5 tokenizer/state machine (no spec parse-error recovery).
//! - Tag/attribute names are restricted to ASCII `[A-Za-z0-9:_-]`.
//! - Rawtext close-tag scanning accepts only ASCII whitespace before `>` (see
//!   `find_rawtext_close_tag`).
//! - `Token::TextSpan` ranges are stable only while the tokenizer's `source` is
//!   append-only; dropping prefixes will require a different storage model.
//!
//! TODO(html/tokenizer/html5): replace with a full HTML5 tokenizer + tree builder state machine.
use crate::dom_builder::TokenTextResolver;
use crate::entities::decode_entities;
#[cfg(feature = "html5-entities")]
use crate::entities::{decode_entities_html5_in_attribute, decode_entities_html5_in_text};
use crate::types::{AtomId, AtomTable, Token, TokenStream};
use memchr::{memchr, memrchr};
use std::borrow::Cow;
use std::sync::Arc;
use tools::utf8::{finish_utf8, push_utf8_chunk};

const HTML_COMMENT_START: &str = "<!--";
const HTML_COMMENT_END: &str = "-->";

fn starts_with_ignore_ascii_case_at(haystack: &[u8], start: usize, needle: &[u8]) -> bool {
    haystack.len() >= start + needle.len()
        && haystack[start..start + needle.len()].eq_ignore_ascii_case(needle)
}

// it only attempts matches starting at ASCII <
// < cannot appear in UTF-8 continuation bytes
const SCRIPT_CLOSE_TAG: &[u8] = b"</script";
const STYLE_CLOSE_TAG: &[u8] = b"</style";

fn find_rawtext_close_tag(haystack: &str, close_tag: &[u8]) -> Option<(usize, usize)> {
    let hay_bytes = haystack.as_bytes();
    let len = hay_bytes.len();
    let n = close_tag.len();
    debug_assert!(n >= 2);
    debug_assert!(close_tag[0] == b'<' && close_tag[1] == b'/');
    debug_assert!(close_tag.is_ascii());
    debug_assert!(
        close_tag.eq_ignore_ascii_case(SCRIPT_CLOSE_TAG)
            || close_tag.eq_ignore_ascii_case(STYLE_CLOSE_TAG)
    );
    if len < n {
        return None;
    }
    let mut i = 0;
    while i + n <= len {
        let rel = memchr(b'<', &hay_bytes[i..])?;
        i += rel;
        if i + n > len {
            return None;
        }
        if hay_bytes[i + 1] == b'/' && starts_with_ignore_ascii_case_at(hay_bytes, i, close_tag) {
            let mut k = i + n;
            // Spec allows other parse-error paths like `</script foo>`, but we only
            // accept ASCII whitespace before `>` to keep the scan simple/alloc-free.
            while k < len && hay_bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            if k < len && hay_bytes[k] == b'>' {
                return Some((i, k + 1));
            }
        }
        i += 1;
    }
    None
}

fn clamp_char_boundary(input: &str, idx: usize, floor: usize) -> usize {
    let mut idx = idx.min(input.len());
    while idx > floor && !input.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[derive(Debug)]
enum PendingState {
    None,
    Text {
        start: usize,
        scan_from: usize,
    },
    Comment {
        start: usize,
        scan_from: usize,
    },
    Doctype {
        doctype_start: usize,
        scan_from: usize,
    },
    Rawtext {
        tag: AtomId,
        close_tag: &'static [u8],
        content_start: usize,
        scan_from: usize,
    },
}

/// Stateful tokenizer for incremental byte feeds.
#[derive(Debug)]
pub struct Tokenizer {
    atoms: AtomTable,
    text_pool: Vec<String>,
    // NOTE: `source` is currently monolithic; spans are byte ranges into it.
    // This means we cannot drop consumed prefixes yet. A later milestone should
    // move to segmented storage / a sliding window once the parser consumes
    // tokens incrementally.
    source: String,
    carry: Vec<u8>,
    cursor: usize,
    pending: PendingState,
    tokens: Vec<Token>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct TokenizerView<'a> {
    atoms: &'a AtomTable,
    source: &'a str,
    text_pool: &'a [String],
}

#[cfg(test)]
impl<'a> TokenizerView<'a> {
    pub(crate) fn resolve_atom(&self, id: AtomId) -> &str {
        self.atoms.resolve(id)
    }

    pub(crate) fn text(&self, token: &Token) -> Option<&str> {
        match token {
            Token::TextSpan { range } => {
                debug_assert!(
                    self.source.is_char_boundary(range.start)
                        && self.source.is_char_boundary(range.end),
                    "text span must be on UTF-8 boundaries"
                );
                Some(&self.source[range.clone()])
            }
            Token::TextOwned { index } => self.text_pool.get(*index).map(|s| s.as_str()),
            _ => None,
        }
    }
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            atoms: AtomTable::new(),
            text_pool: Vec::new(),
            source: String::new(),
            carry: Vec::new(),
            cursor: 0,
            pending: PendingState::None,
            tokens: Vec::new(),
        }
    }

    pub fn atoms(&self) -> &AtomTable {
        &self.atoms
    }

    /// Append bytes and return any newly emitted tokens.
    ///
    /// For streaming without per-call allocations, prefer `feed()` + `drain_into()`.
    pub fn push(&mut self, input: &[u8]) -> Vec<Token> {
        self.feed(input);
        self.take_tokens()
    }

    /// Append UTF-8 text and return any newly emitted tokens.
    ///
    /// For streaming without per-call allocations, prefer `feed_str()` + `drain_into()`.
    pub fn push_str(&mut self, input: &str) -> Vec<Token> {
        self.feed_str_valid(input);
        self.take_tokens()
    }

    /// Finish tokenization and return any remaining tokens.
    pub fn finish_tokens(&mut self) -> Vec<Token> {
        self.finish();
        self.take_tokens()
    }

    pub fn feed(&mut self, input: &[u8]) -> usize {
        if input.is_empty() {
            return 0;
        }
        push_utf8_chunk(&mut self.source, &mut self.carry, input);
        self.scan(false)
    }

    pub fn feed_str(&mut self, input: &str) -> usize {
        self.feed_str_valid(input)
    }

    /// Append validated UTF-8 text and scan without re-validating.
    pub fn feed_str_valid(&mut self, input: &str) -> usize {
        if input.is_empty() {
            return 0;
        }
        self.source.push_str(input);
        self.scan(false)
    }

    pub fn finish(&mut self) -> usize {
        finish_utf8(&mut self.source, &mut self.carry);
        self.scan(true)
    }

    #[cfg(test)]
    pub(crate) fn view(&self) -> TokenizerView<'_> {
        TokenizerView {
            atoms: &self.atoms,
            source: self.source.as_str(),
            text_pool: &self.text_pool,
        }
    }

    /// Drain any pending tokens into the provided output buffer.
    pub fn drain_into(&mut self, out: &mut Vec<Token>) {
        out.append(&mut self.tokens);
    }

    #[cfg(test)]
    pub fn drain_tokens(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        self.drain_into(&mut out);
        out
    }

    pub fn into_stream(self) -> TokenStream {
        let source: Arc<str> = Arc::from(self.source);
        TokenStream::new(self.tokens, self.atoms, source, self.text_pool)
    }

    pub fn text(&self, token: &Token) -> Option<&str> {
        match token {
            Token::TextSpan { range } => {
                debug_assert!(
                    self.source.is_char_boundary(range.start)
                        && self.source.is_char_boundary(range.end),
                    "text span must be on UTF-8 boundaries"
                );
                Some(&self.source[range.clone()])
            }
            Token::TextOwned { index } => self.text_pool.get(*index).map(|s| s.as_str()),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn into_parts(self) -> (AtomTable, Arc<str>, Vec<String>) {
        let source: Arc<str> = Arc::from(self.source);
        (self.atoms, source, self.text_pool)
    }

    fn take_tokens(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        out.append(&mut self.tokens);
        out
    }

    fn scan(&mut self, is_final: bool) -> usize {
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
                    self.tokens.push(Token::Comment(
                        input[comment_start..comment_end].to_string(),
                    ));
                    self.cursor = comment_end + HTML_COMMENT_END.len();
                    continue;
                }
                if is_final {
                    self.tokens
                        .push(Token::Comment(input[comment_start..].to_string()));
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
                    let doctype = input[doctype_start..end].trim().to_string();
                    self.tokens.push(Token::Doctype(doctype));
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
                let mut j = start;
                while j < len
                    && (bytes[j].is_ascii_alphanumeric()
                        || bytes[j] == b'-'
                        || bytes[j] == b'_'
                        || bytes[j] == b':')
                {
                    j += 1;
                }
                if j == start {
                    if !is_final {
                        break;
                    }
                    self.emit_raw_text_span(self.cursor, (self.cursor + 1).min(len));
                    self.cursor = (self.cursor + 1).min(len);
                    continue;
                }
                if j == len && !is_final {
                    break;
                }
                let name = self.atoms.intern_ascii_lowercase(&input[start..j]);
                // NOTE: we accept `</div foo>` and ignore extra junk until `>`.
                while j < len && bytes[j] != b'>' {
                    j += 1;
                }
                if j == len && !is_final {
                    break;
                }
                if j < len {
                    j += 1;
                }
                self.tokens.push(Token::EndTag(name));
                self.cursor = j;
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
                    self.tokens.push(Token::Comment(
                        input[comment_start..comment_end].to_string(),
                    ));
                    self.cursor = comment_end + HTML_COMMENT_END.len();
                    self.pending = PendingState::None;
                    return true;
                }
                if is_final {
                    self.tokens
                        .push(Token::Comment(input[comment_start..].to_string()));
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
                    let doctype = input[doctype_start..end].trim().to_string();
                    self.tokens.push(Token::Doctype(doctype));
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
            } => {
                let input = self.source.as_str();
                let len = input.len();
                let scan_from = clamp_char_boundary(input, scan_from, content_start);
                if let Some((rel_start, rel_end)) =
                    find_rawtext_close_tag(&input[scan_from..], close_tag)
                {
                    let slice_end = scan_from + rel_start;
                    if slice_end > content_start {
                        self.tokens.push(Token::TextSpan {
                            range: content_start..slice_end,
                        });
                    }
                    self.tokens.push(Token::EndTag(tag));
                    self.cursor = scan_from + rel_end;
                    self.pending = PendingState::None;
                    return true;
                }
                if is_final {
                    if content_start < len {
                        self.tokens.push(Token::TextSpan {
                            range: content_start..len,
                        });
                    }
                    self.tokens.push(Token::EndTag(tag));
                    self.cursor = len;
                    self.pending = PendingState::None;
                    return true;
                }
                let bytes = input.as_bytes();
                let scan_from = memrchr(b'<', &bytes[content_start..])
                    .map(|rel| content_start + rel)
                    .unwrap_or(len);
                self.pending = PendingState::Rawtext {
                    tag,
                    close_tag,
                    content_start,
                    scan_from,
                };
                false
            }
        }
    }

    fn emit_text(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let text = &self.source[start..end];
        #[cfg(feature = "html5-entities")]
        let decoded = decode_entities_html5_in_text(text);
        #[cfg(not(feature = "html5-entities"))]
        let decoded = decode_entities(text);
        if decoded.is_empty() {
            return;
        }
        match decoded {
            Cow::Borrowed(_) => self.tokens.push(Token::TextSpan { range: start..end }),
            Cow::Owned(decoded) => {
                let index = self.text_pool.len();
                self.text_pool.push(decoded);
                self.tokens.push(Token::TextOwned { index });
            }
        }
    }

    fn emit_raw_text_span(&mut self, start: usize, end: usize) {
        if start < end {
            self.tokens.push(Token::TextSpan { range: start..end });
        }
    }

    fn parse_start_tag(&mut self, is_final: bool) -> ParseOutcome {
        let input = self.source.as_str();
        let bytes = input.as_bytes();
        let len = bytes.len();
        let start = self.cursor + 1;
        let mut j = start;
        while j < len
            && (bytes[j].is_ascii_alphanumeric()
                || bytes[j] == b'-'
                || bytes[j] == b'_'
                || bytes[j] == b':')
        {
            j += 1;
        }
        if j == start {
            if !is_final {
                return ParseOutcome::Incomplete;
            }
            self.emit_raw_text_span(self.cursor, (self.cursor + 1).min(len));
            self.cursor = (self.cursor + 1).min(len);
            return ParseOutcome::Complete;
        }
        if j == len && !is_final {
            return ParseOutcome::Incomplete;
        }
        let name = self.atoms.intern_ascii_lowercase(&input[start..j]);
        let mut k = j;
        let mut attributes: Vec<(AtomId, Option<String>)> = Vec::new();
        let mut self_closing = false;

        let skip_whitespace = |k: &mut usize| {
            while *k < len && bytes[*k].is_ascii_whitespace() {
                *k += 1;
            }
        };
        let is_name_char = |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b':';

        loop {
            skip_whitespace(&mut k);
            if k >= len {
                if is_final {
                    break;
                }
                return ParseOutcome::Incomplete;
            }
            if bytes[k] == b'>' {
                k += 1;
                break;
            }
            if bytes[k] == b'/' {
                if k + 1 >= len {
                    if is_final {
                        k += 1;
                        continue;
                    }
                    return ParseOutcome::Incomplete;
                }
                if bytes[k + 1] == b'>' {
                    self_closing = true;
                    k += 2;
                    break;
                }
                k += 1;
                continue;
            }
            let name_start = k;
            while k < len && is_name_char(bytes[k]) {
                k += 1;
            }
            if name_start == k {
                if k >= len && !is_final {
                    return ParseOutcome::Incomplete;
                }
                k += 1;
                continue;
            }
            let attribute_name = self.atoms.intern_ascii_lowercase(&input[name_start..k]);

            skip_whitespace(&mut k);
            if k >= len {
                if is_final {
                    attributes.push((attribute_name, None));
                    break;
                }
                return ParseOutcome::Incomplete;
            }

            let value: Option<String>;
            if bytes[k] == b'=' {
                k += 1;
                skip_whitespace(&mut k);
                if k >= len {
                    if is_final {
                        value = Some(String::new());
                    } else {
                        return ParseOutcome::Incomplete;
                    }
                } else if bytes[k] == b'"' || bytes[k] == b'\'' {
                    let quote = bytes[k];
                    k += 1;
                    let vstart = k;
                    while k < len && bytes[k] != quote {
                        k += 1;
                    }
                    if k >= len && !is_final {
                        return ParseOutcome::Incomplete;
                    }
                    let raw = &input[vstart..k.min(len)];
                    if k < len {
                        k += 1;
                    }
                    #[cfg(feature = "html5-entities")]
                    let decoded = decode_entities_html5_in_attribute(raw);
                    #[cfg(not(feature = "html5-entities"))]
                    let decoded = decode_entities(raw);
                    value = Some(decoded.into_owned());
                } else {
                    let vstart = k;
                    while k < len && !bytes[k].is_ascii_whitespace() && bytes[k] != b'>' {
                        if bytes[k] == b'/' && k + 1 < len && bytes[k + 1] == b'>' {
                            break;
                        }
                        k += 1;
                    }
                    if k == len && !is_final {
                        return ParseOutcome::Incomplete;
                    }
                    if k > vstart {
                        #[cfg(feature = "html5-entities")]
                        let decoded = decode_entities_html5_in_attribute(&input[vstart..k]);
                        #[cfg(not(feature = "html5-entities"))]
                        let decoded = decode_entities(&input[vstart..k]);
                        value = Some(decoded.into_owned());
                    } else {
                        value = Some(String::new());
                    }
                }
            } else {
                value = None;
            }
            attributes.push((attribute_name, value));
        }
        if is_void_element(self.atoms.resolve(name)) {
            self_closing = true;
        }

        if k < len && bytes[k] == b'>' {
            k += 1;
        }
        let content_start = k;

        self.tokens.push(Token::StartTag {
            name,
            attributes,
            self_closing,
        });

        let name_str = self.atoms.resolve(name);
        if (name_str == "script" || name_str == "style") && !self_closing {
            let close_tag = if name_str == "script" {
                SCRIPT_CLOSE_TAG
            } else {
                STYLE_CLOSE_TAG
            };
            if let Some((rel_start, rel_end)) =
                find_rawtext_close_tag(&input[content_start..], close_tag)
            {
                let slice_end = content_start + rel_start;
                if slice_end > content_start {
                    self.tokens.push(Token::TextSpan {
                        range: content_start..slice_end,
                    });
                }
                self.tokens.push(Token::EndTag(name));
                self.cursor = content_start + rel_end;
                return ParseOutcome::Complete;
            }
            if is_final {
                if content_start < input.len() {
                    self.tokens.push(Token::TextSpan {
                        range: content_start..input.len(),
                    });
                }
                self.tokens.push(Token::EndTag(name));
                self.cursor = input.len();
                return ParseOutcome::Complete;
            }
            let scan_from = clamp_char_boundary(
                input,
                input
                    .len()
                    .saturating_sub(close_tag.len() + 1)
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
            };
            return ParseOutcome::Complete;
        }

        self.cursor = content_start;
        ParseOutcome::Complete
    }
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenTextResolver for Tokenizer {
    fn text(&self, token: &Token) -> Option<&str> {
        Tokenizer::text(self, token)
    }
}

#[derive(Debug)]
enum ParseOutcome {
    Complete,
    Incomplete,
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

/// Tokenizes into a token stream with interned tag/attribute names to reduce allocations.
pub fn tokenize(input: &str) -> TokenStream {
    let mut tokenizer = Tokenizer::new();
    tokenizer.feed_str(input);
    tokenizer.finish();
    tokenizer.into_stream()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "count-alloc")]
    use crate::test_alloc;
    use std::fmt::Write;
    #[cfg(feature = "perf-tests")]
    use std::time::{Duration, Instant};

    fn text_eq(stream: &TokenStream, token: &Token, expected: &str) -> bool {
        stream.text(token) == Some(expected)
    }

    fn token_snapshot(stream: &TokenStream) -> Vec<String> {
        let atoms = stream.atoms();
        stream
            .tokens()
            .iter()
            .map(|token| match token {
                Token::Doctype(value) => format!("Doctype({value})"),
                Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let mut line = String::new();
                    let _ = write!(&mut line, "StartTag({}", atoms.resolve(*name));
                    for (attr, value) in attributes {
                        line.push(' ');
                        line.push_str(atoms.resolve(*attr));
                        if let Some(value) = value {
                            line.push_str("=\"");
                            line.push_str(value);
                            line.push('"');
                        }
                    }
                    if *self_closing {
                        line.push_str(" /");
                    }
                    line.push(')');
                    line
                }
                Token::EndTag(name) => format!("EndTag({})", atoms.resolve(*name)),
                Token::Comment(text) => format!("Comment({text})"),
                Token::TextSpan { .. } | Token::TextOwned { .. } => {
                    let text = stream.text(token).unwrap_or("");
                    format!("Text({text})")
                }
            })
            .collect()
    }

    fn token_snapshot_with_view(view: TokenizerView<'_>, tokens: &[Token]) -> Vec<String> {
        tokens
            .iter()
            .map(|token| match token {
                Token::Doctype(value) => format!("Doctype({value})"),
                Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let mut line = String::new();
                    let _ = write!(&mut line, "StartTag({}", view.resolve_atom(*name));
                    for (attr, value) in attributes {
                        line.push(' ');
                        line.push_str(view.resolve_atom(*attr));
                        if let Some(value) = value {
                            line.push_str("=\"");
                            line.push_str(value);
                            line.push('"');
                        }
                    }
                    if *self_closing {
                        line.push_str(" /");
                    }
                    line.push(')');
                    line
                }
                Token::EndTag(name) => format!("EndTag({})", view.resolve_atom(*name)),
                Token::Comment(text) => format!("Comment({text})"),
                Token::TextSpan { .. } | Token::TextOwned { .. } => {
                    let text = view.text(token).unwrap_or("");
                    format!("Text({text})")
                }
            })
            .collect()
    }

    fn tokenize_in_chunks(input: &str, sizes: &[usize]) -> TokenStream {
        let bytes = input.as_bytes();
        let mut tokenizer = Tokenizer::new();
        let mut offset = 0usize;
        for size in sizes {
            if offset >= bytes.len() {
                break;
            }
            let end = (offset + size).min(bytes.len());
            tokenizer.feed(&bytes[offset..end]);
            offset = end;
        }
        if offset < bytes.len() {
            tokenizer.feed(&bytes[offset..]);
        }
        tokenizer.finish();
        tokenizer.into_stream()
    }

    fn tokenize_with_push_str(input: &str, sizes: &[usize]) -> TokenStream {
        let mut tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        let mut offset = 0usize;
        for size in sizes {
            if offset >= input.len() {
                break;
            }
            let end = (offset + size).min(input.len());
            let end = clamp_char_boundary(input, end, offset);
            if end == offset {
                break;
            }
            tokens.extend(tokenizer.push_str(&input[offset..end]));
            offset = end;
        }
        if offset < input.len() {
            tokens.extend(tokenizer.push_str(&input[offset..]));
        }
        tokens.extend(tokenizer.finish_tokens());
        let (atoms, source, text_pool) = tokenizer.into_parts();
        TokenStream::new(tokens, atoms, source, text_pool)
    }

    fn tokenize_with_feed_bytes(bytes: &[u8], split: usize) -> TokenStream {
        let mut tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.feed(&bytes[..split]);
        tokenizer.drain_into(&mut tokens);
        tokenizer.feed(&bytes[split..]);
        tokenizer.finish();
        tokenizer.drain_into(&mut tokens);
        let (atoms, source, text_pool) = tokenizer.into_parts();
        TokenStream::new(tokens, atoms, source, text_pool)
    }

    #[test]
    fn tokenize_preserves_utf8_text_nodes() {
        let stream = tokenize("<p>120×32</p>");
        assert!(
            stream.iter().any(|t| text_eq(&stream, t, "120×32")),
            "expected UTF-8 text token, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_uppercase_doctype() {
        let stream = tokenize("<!DOCTYPE html>");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Doctype(s) if s == "DOCTYPE html")),
            "expected case-insensitive doctype, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_mixed_case_doctype() {
        let stream = tokenize("<!DoCtYpE html>");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Doctype(s) if s == "DoCtYpE html")),
            "expected mixed-case doctype to parse, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_finds_script_end_tag_case_insensitive() {
        let stream = tokenize("<script>let x = 1;</ScRiPt>");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, body, "let x = 1;")
                        && atoms.resolve(*end) == "script"
            ),
            "expected raw script text and matching end tag, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_non_ascii_text_around_tags() {
        let stream = tokenize("¡Hola <b>café</b> 😊");
        assert!(
            stream.iter().any(|t| text_eq(&stream, t, "¡Hola ")),
            "expected leading UTF-8 text token, got: {stream:?}"
        );
        assert!(
            stream.iter().any(|t| text_eq(&stream, t, "café")),
            "expected UTF-8 text inside tag, got: {stream:?}"
        );
        assert!(
            stream.iter().any(|t| text_eq(&stream, t, " 😊")),
            "expected trailing UTF-8 text token, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_large_rawtext_body_without_pathological_slowdown() {
        let mut body = String::new();
        for _ in 0..100_000 {
            body.push_str("let x = 1; < not a tag\n");
        }
        let input = format!("<script>{}</ScRiPt>", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, text, &body)
                        && atoms.resolve(*end) == "script"
            ),
            "expected large rawtext body to tokenize correctly, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_dense_near_match_rawtext_body() {
        let mut body = String::new();
        for _ in 0..50_000 {
            body.push_str("</scripX>");
        }
        let input = format!("<script>{}</ScRiPt>", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, text, &body)
                        && atoms.resolve(*end) == "script"
            ),
            "expected dense rawtext body to tokenize correctly, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_dense_near_match_style_rawtext_body() {
        let mut body = String::new();
        for _ in 0..50_000 {
            body.push_str("</stylX>");
        }
        let input = format!("<style>{}</StYle>", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                    if atoms.resolve(*name) == "style"
                        && text_eq(&stream, text, &body)
                        && atoms.resolve(*end) == "style"
            ),
            "expected dense style rawtext body to tokenize correctly, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_allows_whitespace_before_rawtext_close_gt() {
        let stream = tokenize("<script>let x=1;</script >");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, body, "let x=1;")
                        && atoms.resolve(*end) == "script"
            ),
            "expected script end tag with whitespace before >, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_allows_whitespace_before_rawtext_close_gt_case_insensitive() {
        let stream = tokenize("<style>body{}</STYLE\t>");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                    if atoms.resolve(*name) == "style"
                        && text_eq(&stream, body, "body{}")
                        && atoms.resolve(*end) == "style"
            ),
            "expected style end tag with whitespace before >, got: {stream:?}"
        );
    }

    #[test]
    fn rawtext_close_tag_does_not_accept_near_matches() {
        let stream = tokenize("<script>ok</scriptx >no</script >");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, body, "ok</scriptx >no")
                        && atoms.resolve(*end) == "script"
            ),
            "expected near-match not to close rawtext, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_non_ascii_attribute_values() {
        let stream = tokenize("<p data=naïve>ok</p>");
        let atoms = stream.atoms();
        assert!(
            stream.iter().any(|t| matches!(
                t,
                Token::StartTag { name, attributes, .. }
                    if atoms.resolve(*name) == "p"
                        && attributes.iter().any(|(k, v)| {
                            atoms.resolve(*k) == "data" && v.as_deref() == Some("naïve")
                        })
            )),
            "expected UTF-8 attribute value, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_decodes_entities_in_unquoted_attributes() {
        let stream = tokenize("<p data=Tom&amp;Jerry title=&#x3C;ok&#x3E;>ok</p>");
        let atoms = stream.atoms();
        assert!(
            stream.iter().any(|t| matches!(
                t,
                Token::StartTag { name, attributes, .. }
                    if atoms.resolve(*name) == "p"
                        && attributes.iter().any(|(k, v)| {
                            atoms.resolve(*k) == "data" && v.as_deref() == Some("Tom&Jerry")
                        })
                        && attributes.iter().any(|(k, v)| {
                            atoms.resolve(*k) == "title" && v.as_deref() == Some("<ok>")
                        })
            )),
            "expected entity-decoded unquoted attributes, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_utf8_adjacent_to_angle_brackets() {
        let stream = tokenize("é<b>ï</b>ö");
        assert!(stream.iter().any(|t| text_eq(&stream, t, "é")));
        assert!(stream.iter().any(|t| text_eq(&stream, t, "ï")));
        assert!(stream.iter().any(|t| text_eq(&stream, t, "ö")));
    }

    #[test]
    fn tokenize_interns_case_insensitive_tag_and_attr_names() {
        let stream = tokenize("<DiV id=one></div><div ID=two></DIV>");
        let atoms = stream.atoms();
        let mut div_ids = Vec::new();
        let mut id_ids = Vec::new();

        for token in stream.iter() {
            match token {
                Token::StartTag {
                    name, attributes, ..
                } => {
                    div_ids.push(*name);
                    for (attr_name, _) in attributes {
                        id_ids.push(*attr_name);
                    }
                }
                Token::EndTag(name) => div_ids.push(*name),
                _ => {}
            }
        }

        assert!(
            div_ids.windows(2).all(|w| w[0] == w[1]),
            "expected all div atoms to match, got: {div_ids:?}"
        );
        assert!(
            id_ids.windows(2).all(|w| w[0] == w[1]),
            "expected all id atoms to match, got: {id_ids:?}"
        );
        assert_eq!(atoms.resolve(div_ids[0]), "div");
        assert_eq!(atoms.resolve(id_ids[0]), "id");
        assert_eq!(atoms.len(), 2, "expected only two interned names");
    }

    #[test]
    fn tokenize_allows_custom_element_and_namespaced_tags() {
        let stream = tokenize("<my-component></my-component><svg:rect></svg:rect>");
        let atoms = stream.atoms();
        let mut names = Vec::new();

        for token in stream.iter() {
            match token {
                Token::StartTag { name, .. } | Token::EndTag(name) => names.push(*name),
                _ => {}
            }
        }

        assert_eq!(atoms.resolve(names[0]), "my-component");
        assert_eq!(atoms.resolve(names[1]), "my-component");
        assert_eq!(atoms.resolve(names[2]), "svg:rect");
        assert_eq!(atoms.resolve(names[3]), "svg:rect");
    }

    #[test]
    fn tokenize_handles_many_simple_tags_linearly() {
        let mut input = String::new();
        for _ in 0..20_000 {
            input.push_str("<a></a>");
        }
        let stream = tokenize(&input);
        assert_eq!(stream.tokens().len(), 40_000);
    }

    #[test]
    fn tokenize_handles_rawtext_without_close_tag() {
        let mut body = String::new();
        for _ in 0..100_000 {
            body.push_str("x<y>\n");
        }
        let input = format!("<script>{}", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, text, &body)
                        && atoms.resolve(*end) == "script"
            ),
            "expected rawtext body without close tag to tokenize correctly, got: {stream:?}"
        );
    }

    #[cfg(feature = "count-alloc")]
    #[test]
    fn tokenize_rawtext_allocation_is_bounded() {
        let mut body = String::new();
        for _ in 0..500_000 {
            body.push('x');
        }
        let input = format!("<script>{}</ScRiPt>", body);

        let _guard = test_alloc::AllocGuard::new();
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        let (_, bytes) = test_alloc::counts();

        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                    if atoms.resolve(*name) == "script"
                        && text_eq(&stream, text, &body)
                        && atoms.resolve(*end) == "script"
            ),
            "expected rawtext body to tokenize correctly, got: {stream:?}"
        );

        let overhead = 64 * 1024;
        let expected_source = input.len();
        let extra = bytes.saturating_sub(expected_source);
        assert!(
            extra <= overhead,
            "expected bounded extra allocations; bytes={bytes} input_len={expected_source} extra={extra} overhead={overhead}"
        );
    }

    #[cfg(feature = "count-alloc")]
    #[test]
    fn tokenize_plain_text_avoids_text_allocation() {
        let mut body = String::new();
        for _ in 0..500_000 {
            body.push('x');
        }
        let input = format!("<p>{}</p>", body);

        let _guard = test_alloc::AllocGuard::new();
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        let (_, bytes) = test_alloc::counts();

        assert!(
            matches!(
                stream.tokens(),
                [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                    if atoms.resolve(*name) == "p"
                        && text_eq(&stream, text, &body)
                        && atoms.resolve(*end) == "p"
            ),
            "expected plain text to tokenize correctly, got: {stream:?}"
        );

        let overhead = 128 * 1024;
        let expected_source = input.len();
        let extra = bytes.saturating_sub(expected_source);
        assert!(
            extra <= overhead,
            "expected bounded extra allocations; bytes={bytes} input_len={expected_source} extra={extra} overhead={overhead}"
        );
    }

    #[test]
    fn tokenize_handles_many_comments_and_doctypes() {
        let mut input = String::new();
        for _ in 0..5_000 {
            input.push_str("<!--x-->");
        }
        for _ in 0..5_000 {
            input.push_str("<!DOCTYPE html>");
        }

        let stream = tokenize(&input);
        let mut comment_count = 0;
        let mut doctype_count = 0;
        for token in stream.iter() {
            match token {
                Token::Comment(_) => comment_count += 1,
                Token::Doctype(_) => doctype_count += 1,
                _ => {}
            }
        }

        assert_eq!(comment_count, 5_000);
        assert_eq!(doctype_count, 5_000);
    }

    #[test]
    fn tokenize_handles_tons_of_angle_brackets() {
        let input = "<".repeat(200_000);
        let stream = tokenize(&input);
        assert!(stream.tokens().len() <= input.len());
    }

    #[test]
    fn tokenize_incremental_matches_full_for_small_chunks() {
        let input = "<!DOCTYPE html><!--c--><div class=one>Hi &amp; \
                     <script>let x = 1;</script><style>p{}</style>é</div>";
        let full = tokenize(input);
        let chunked = tokenize_in_chunks(input, &[1, 2, 3, 7, 64]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_matches_full_for_small_chunks() {
        let input = "<!DOCTYPE html><!--c--><div class=one>Hi &amp; \
                     <script>let x = 1;</script><style>p{}</style>é</div>";
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[1, 2, 3, 7, 64]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_incremental_matches_full_for_utf8_splits() {
        let input = "<p>café 😊 &amp; naïve</p>";
        let full = tokenize(input);
        let chunked = tokenize_in_chunks(input, &[1, 1, 1, 2, 1, 4, 1]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_incremental_handles_split_script_end_tag() {
        let input = "<script>hi</script>";
        let split = "<script>hi</scr".len();
        let full = tokenize(input);
        let chunked = tokenize_in_chunks(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_script_end_tag() {
        let input = "<script>hi</script>";
        let split = "<script>hi</scr".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_incremental_handles_split_end_tag_prefix() {
        let input = "<div></div>";
        let split = "<div></".len();
        let full = tokenize(input);
        let chunked = tokenize_in_chunks(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_tag_name() {
        let input = "<div>ok</div>";
        let split = "<d".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_incremental_handles_split_comment_terminator() {
        let input = "<!--x-->";
        let split = "<!--x--".len();
        let full = tokenize(input);
        let chunked = tokenize_in_chunks(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_comment_terminator() {
        let input = "<!--x-->";
        let split = "<!--x--".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_comment_terminator_dash() {
        let input = "<!--x-->";
        let split = "<!--x-".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_comment_terminator_arrow() {
        let input = "<!--x-->";
        let split = "<!--x".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_incremental_handles_split_doctype_end() {
        let input = "<!DOCTYPE html>";
        let split = "<!DOCTYPE html".len();
        let full = tokenize(input);
        let chunked = tokenize_in_chunks(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_doctype_end() {
        let input = "<!DOCTYPE html>";
        let split = "<!DOCTYPE html".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_attribute_name() {
        let input = "<p data-value=ok>hi</p>";
        let split = "<p da".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_attribute_value() {
        let input = "<p data=\"value\">ok</p>";
        let split = "<p data=\"va".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_rawtext_close_tag() {
        let input = "<style>body{}</style>";
        let split = "<style>body{}</sty".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_handles_split_rawtext_close_tag_with_whitespace() {
        let input = "<style>body{}</style \t>";
        let split = "<style>body{}</style \t".len();
        let full = tokenize(input);
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(token_snapshot(&full), token_snapshot(&chunked));
    }

    #[test]
    fn tokenize_push_str_fuzz_boundaries_matches_full() {
        let input = "<!DOCTYPE html><!--c--><div class=one data-x=\"y\">Hi &amp; é \
                     <script>let x = 1;</script><style>p{}</style></div>";
        let full = tokenize(input);
        let expected = token_snapshot(&full);

        for split in 0..=input.len() {
            let chunked = tokenize_with_push_str(input, &[split]);
            assert_eq!(
                expected,
                token_snapshot(&chunked),
                "boundary split at {split} should match full tokenization"
            );
        }
    }

    #[test]
    fn tokenize_feed_bytes_fuzz_boundaries_matches_full() {
        let input = "<!DOCTYPE html><!--c--><div class=one data-x=\"y\">Hi &amp; é \
                     <script>let x = 1;</script><style>p{}</style></div>";
        let full = tokenize(input);
        let expected = token_snapshot(&full);
        let bytes = input.as_bytes();

        for split in 0..=bytes.len() {
            let chunked = tokenize_with_feed_bytes(bytes, split);
            assert_eq!(
                expected,
                token_snapshot(&chunked),
                "byte boundary split at {split} should match full tokenization"
            );
        }
    }

    #[test]
    fn tokenize_incremental_drain_view_matches_full() {
        let input = "<!DOCTYPE html><!--c--><div class=one>Tom&amp;Jerry\
                     <script>let x = 1;</script><style>p{}</style>é</div>";
        let full = tokenize(input);
        let expected = token_snapshot(&full);

        let bytes = input.as_bytes();
        let sizes = [1, 2, 3, 7, 64];
        let mut tokenizer = Tokenizer::new();
        let mut offset = 0usize;
        let mut drained = Vec::new();
        let mut snapshot = Vec::new();

        for size in sizes {
            if offset >= bytes.len() {
                break;
            }
            let end = (offset + size).min(bytes.len());
            tokenizer.feed(&bytes[offset..end]);
            offset = end;
            drained.clear();
            tokenizer.drain_into(&mut drained);
            let view = tokenizer.view();
            snapshot.extend(token_snapshot_with_view(view, &drained));
        }

        if offset < bytes.len() {
            tokenizer.feed(&bytes[offset..]);
        }
        tokenizer.finish();
        drained.clear();
        tokenizer.drain_into(&mut drained);
        let view = tokenizer.view();
        snapshot.extend(token_snapshot_with_view(view, &drained));

        assert_eq!(expected, snapshot);
    }

    #[cfg(feature = "perf-tests")]
    #[test]
    fn tokenize_scales_roughly_linearly_on_repeated_tags() {
        fn build_input(repeats: usize) -> String {
            let mut input = String::new();
            for _ in 0..repeats {
                input.push_str("<a></a>");
            }
            input
        }

        fn measure_total(input: &str) -> Duration {
            let _ = tokenize(input);
            let mut total = Duration::ZERO;
            for _ in 0..5 {
                let start = Instant::now();
                let _ = tokenize(input);
                total += start.elapsed();
            }
            total
        }

        let small = build_input(5_000);
        let large = build_input(20_000);

        let t_small = measure_total(&small);
        let t_large = measure_total(&large);
        assert!(!t_small.is_zero(), "timer resolution too coarse for test");
        // Allow generous slack to avoid flakiness while still catching quadratic regressions.
        assert!(
            t_large <= t_small.saturating_mul(12),
            "expected near-linear scaling; t_small={t_small:?} t_large={t_large:?}"
        );
    }

    #[cfg(feature = "perf-tests")]
    #[test]
    fn tokenize_scales_roughly_linearly_on_comment_scan() {
        fn build_input(repeats: usize, body_len: usize) -> String {
            let mut input = String::new();
            for _ in 0..repeats {
                input.push_str("<!--");
                input.extend(std::iter::repeat_n('-', body_len));
                input.push('x');
                input.push_str("-->");
            }
            input
        }

        fn measure_total(input: &str) -> Duration {
            let _ = tokenize(input);
            let mut total = Duration::ZERO;
            for _ in 0..5 {
                let start = Instant::now();
                let _ = tokenize(input);
                total += start.elapsed();
            }
            total
        }

        let small = build_input(500, 400);
        let large = build_input(2_000, 400);

        let t_small = measure_total(&small);
        let t_large = measure_total(&large);
        assert!(!t_small.is_zero(), "timer resolution too coarse for test");
        assert!(
            t_large <= t_small.saturating_mul(12),
            "expected near-linear comment scan; t_small={t_small:?} t_large={t_large:?}"
        );
    }
}

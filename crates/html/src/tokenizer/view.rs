use super::Tokenizer;
use crate::types::{AtomId, AtomTable, AttributeValue, TextPayload, Token};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TokenizerView<'a> {
    atoms: &'a AtomTable,
    source: &'a str,
    text_pool: &'a [String],
}

impl<'a> TokenizerView<'a> {
    #[inline]
    pub(crate) fn resolve_atom(&self, id: AtomId) -> &str {
        self.atoms.resolve(id)
    }

    #[inline]
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

    #[inline]
    pub(crate) fn attr_value(&self, value: &'a AttributeValue) -> &'a str {
        value.as_str(self.source)
    }

    #[inline]
    pub(crate) fn payload_text(&self, payload: &'a TextPayload) -> &'a str {
        payload.as_str(self.source)
    }
}

impl Tokenizer {
    pub(crate) fn view(&self) -> TokenizerView<'_> {
        TokenizerView {
            atoms: &self.atoms,
            source: self.source.as_str(),
            text_pool: &self.text_pool,
        }
    }

    pub(crate) fn reset_rawtext_scan_steps(&mut self) {
        self.rawtext_scan_steps = 0;
    }

    pub(crate) fn rawtext_scan_steps(&self) -> usize {
        self.rawtext_scan_steps
    }

    pub(crate) fn tokens_capacity(&self) -> usize {
        self.tokens.capacity()
    }

    pub(crate) fn into_parts(self) -> (AtomTable, Arc<str>, Vec<String>) {
        let source: Arc<str> = Arc::from(self.source);
        (self.atoms, source, self.text_pool)
    }
}

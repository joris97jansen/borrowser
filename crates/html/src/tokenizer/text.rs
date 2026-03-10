use super::Tokenizer;
use crate::entities::decode_entities;
#[cfg(feature = "html5-entities")]
use crate::entities::{decode_entities_html5_in_attribute, decode_entities_html5_in_text};
use crate::types::{AttributeValue, Token};
use memchr::memchr;
use std::borrow::Cow;

impl Tokenizer {
    pub(crate) fn emit_text(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let text = &self.source[start..end];
        let decoded = decode_text_fragment(text);
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

    pub(crate) fn emit_raw_text_span(&mut self, start: usize, end: usize) {
        if start < end {
            self.tokens.push(Token::TextSpan { range: start..end });
        }
    }

    pub(crate) fn decode_attribute_value(
        &self,
        raw: &str,
        start: usize,
        end: usize,
    ) -> AttributeValue {
        match decode_attribute_fragment(raw) {
            Cow::Borrowed(_) => AttributeValue::Span { range: start..end },
            Cow::Owned(decoded) => AttributeValue::Owned(decoded),
        }
    }
}

fn decode_text_fragment(text: &str) -> Cow<'_, str> {
    #[cfg(feature = "html5-entities")]
    {
        if memchr(b'&', text.as_bytes()).is_some() {
            decode_entities_html5_in_text(text)
        } else {
            Cow::Borrowed(text)
        }
    }
    #[cfg(not(feature = "html5-entities"))]
    {
        if memchr(b'&', text.as_bytes()).is_some() {
            decode_entities(text)
        } else {
            Cow::Borrowed(text)
        }
    }
}

fn decode_attribute_fragment(raw: &str) -> Cow<'_, str> {
    #[cfg(feature = "html5-entities")]
    {
        if memchr(b'&', raw.as_bytes()).is_some() {
            decode_entities_html5_in_attribute(raw)
        } else {
            Cow::Borrowed(raw)
        }
    }
    #[cfg(not(feature = "html5-entities"))]
    {
        if memchr(b'&', raw.as_bytes()).is_some() {
            decode_entities(raw)
        } else {
            Cow::Borrowed(raw)
        }
    }
}

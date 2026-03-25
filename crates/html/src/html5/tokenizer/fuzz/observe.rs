use super::super::{TextResolveError, TextResolver};
use super::digest::{mix_atom_name, mix_bytes, mix_u64, token_discriminant};
use crate::html5::shared::{AtomTable, AttributeValue, TextValue, Token};

pub(super) struct TokenObserver {
    max_tokens_observed: usize,
    pub(super) saw_eof: bool,
    pub(super) tokens_observed: usize,
    pub(super) span_resolve_count: usize,
    pub(super) digest: u64,
}

impl TokenObserver {
    pub(super) fn new(max_tokens_observed: usize) -> Self {
        Self {
            max_tokens_observed: max_tokens_observed.max(1),
            saw_eof: false,
            tokens_observed: 0,
            span_resolve_count: 0,
            digest: 0,
        }
    }

    pub(super) fn observe(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        resolver: &dyn TextResolver,
    ) -> Result<(), ObserveError> {
        if self.tokens_observed >= self.max_tokens_observed {
            return Err(ObserveError::TokenBudgetReached);
        }
        self.tokens_observed = self.tokens_observed.saturating_add(1);
        self.digest = mix_u64(self.digest, token_discriminant(token));
        match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                if let Some(name) = name {
                    self.digest = mix_atom_name(self.digest, atoms, *name);
                }
                if let Some(public_id) = public_id {
                    self.digest = mix_bytes(self.digest, public_id.as_bytes());
                }
                if let Some(system_id) = system_id {
                    self.digest = mix_bytes(self.digest, system_id.as_bytes());
                }
                self.digest = mix_u64(self.digest, u64::from(*force_quirks));
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                self.digest = mix_atom_name(self.digest, atoms, *name);
                self.digest = mix_u64(self.digest, u64::from(*self_closing));
                for attr in attrs {
                    self.digest = mix_atom_name(self.digest, atoms, attr.name);
                    if let Some(value) = &attr.value {
                        self.observe_attr_value(value, resolver)?;
                    }
                }
            }
            Token::EndTag { name } => {
                self.digest = mix_atom_name(self.digest, atoms, *name);
            }
            Token::Comment { text } | Token::Text { text } => {
                self.observe_text_value(text, resolver)?;
            }
            Token::Eof => {
                if self.saw_eof {
                    return Err(ObserveError::DuplicateEof);
                }
                self.saw_eof = true;
            }
        }
        Ok(())
    }

    fn observe_attr_value(
        &mut self,
        value: &AttributeValue,
        resolver: &dyn TextResolver,
    ) -> Result<(), ObserveError> {
        match value {
            AttributeValue::Span(span) => {
                let text = resolver
                    .resolve_span(*span)
                    .map_err(ObserveError::InvalidSpan)?;
                self.span_resolve_count = self.span_resolve_count.saturating_add(1);
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
            AttributeValue::Owned(text) => {
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
        }
        Ok(())
    }

    fn observe_text_value(
        &mut self,
        value: &TextValue,
        resolver: &dyn TextResolver,
    ) -> Result<(), ObserveError> {
        match value {
            TextValue::Span(span) => {
                let text = resolver
                    .resolve_span(*span)
                    .map_err(ObserveError::InvalidSpan)?;
                self.span_resolve_count = self.span_resolve_count.saturating_add(1);
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
            TextValue::Owned(text) => {
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
        }
        Ok(())
    }
}

pub(super) enum ObserveError {
    InvalidSpan(TextResolveError),
    DuplicateEof,
    TokenBudgetReached,
}

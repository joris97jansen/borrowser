use crate::html5::shared::{Input, TextSpan, Token};

/// Resolve text spans into `&str` for the current batch epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextResolveError {
    InvalidSpan { span: TextSpan },
}

pub trait TextResolver {
    fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError>;
}

/// Token batch bound to a single epoch.
///
/// Invariant: spans inside tokens are only valid for as long as this `TokenBatch`
/// exists (the batch holds an exclusive borrow of the decoded `Input`).
pub struct TokenBatch<'t> {
    pub(in crate::html5::tokenizer) tokens: Vec<Token>,
    pub(in crate::html5::tokenizer) input: &'t mut Input,
}

impl<'t> TokenBatch<'t> {
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Token> {
        self.tokens.iter()
    }

    pub fn into_tokens(self) -> Vec<Token> {
        self.tokens
    }

    pub fn resolver(&self) -> impl TextResolver + '_ {
        InputResolver {
            input: &*self.input,
        }
    }
}

struct InputResolver<'t> {
    input: &'t Input,
}

impl<'t> TextResolver for InputResolver<'t> {
    fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
        let text = self.input.as_str();
        if !(span.start <= span.end
            && span.end <= text.len()
            && text.is_char_boundary(span.start)
            && text.is_char_boundary(span.end))
        {
            return Err(TextResolveError::InvalidSpan { span });
        }
        Ok(&text[span.start..span.end])
    }
}

use crate::types::{Token, TokenStream};

/// Resolves text for tokens produced by the tokenizer.
///
/// Contract: the returned `&str` must correspond to the provided token (TextSpan/TextOwned)
/// and remain valid for the duration of the `push_token` call.
pub trait TokenTextResolver {
    fn text(&self, token: &Token) -> Option<&str>;
    fn source(&self) -> &str;
}

impl TokenTextResolver for TokenStream {
    fn text(&self, token: &Token) -> Option<&str> {
        TokenStream::text(self, token)
    }

    fn source(&self) -> &str {
        self.source()
    }
}

//! Token emission helpers.

use crate::html5::shared::Token;
use crate::html5::tokenizer::Html5Tokenizer;

impl Html5Tokenizer {
    pub(super) fn emit_token(&mut self, token: Token) {
        #[cfg(any(test, feature = "debug-stats"))]
        log::trace!(target: "html5.tokenizer", "emit token: {token:?}");
        self.tokens.push(token);
        self.stats_inc_tokens_emitted();
    }
}

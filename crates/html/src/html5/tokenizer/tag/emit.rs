use super::super::Html5Tokenizer;
use crate::html5::shared::{DocumentParseContext, Input, Token};

impl Html5Tokenizer {
    pub(super) fn emit_current_tag(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        let (name_start, end) = match (self.tag_name_start.take(), self.tag_name_end.take()) {
            (Some(start), Some(end)) => (start, end),
            _ => return,
        };
        if name_start > end || end > input.as_str().len() {
            return;
        }
        let raw = &input.as_str()[name_start..end];
        // Canonicalization policy: HTML tag names are interned with ASCII
        // folding (`A-Z` -> `a-z`) and preserve non-ASCII bytes.
        let name = self.intern_atom_or_invariant(ctx, raw, "tag name");
        if self.current_tag_is_end {
            self.current_tag_self_closing = false;
            self.current_tag_attrs.clear();
            self.clear_current_attribute();
            self.emit_token(Token::EndTag { name });
        } else {
            let attrs = std::mem::take(&mut self.current_tag_attrs);
            let self_closing = self.current_tag_self_closing;
            self.current_tag_self_closing = false;
            self.clear_current_attribute();
            self.emit_token(Token::StartTag {
                name,
                attrs,
                self_closing,
            });
        }
    }
}

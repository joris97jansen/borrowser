use html::html5::{AtomId, DocumentParseContext, TextModeSpec, Token, TokenizerControl};

/// Token-driven text-mode control for tokenizer-only harnesses.
///
/// The html5 tokenizer itself remains in `Data` mode unless its caller applies
/// explicit `TokenizerControl`. Tokenizer-only harnesses therefore mirror the
/// tree-builder/session contract by switching modes after the relevant emitted
/// start and end tags.
pub struct TokenizerTextModeSupport {
    style: AtomId,
    title: AtomId,
    textarea: AtomId,
    script: AtomId,
}

impl TokenizerTextModeSupport {
    pub fn new(ctx: &mut DocumentParseContext) -> Self {
        let style = ctx
            .atoms
            .intern_ascii_folded("style")
            .expect("style atom interning in tokenizer harness must succeed");
        let title = ctx
            .atoms
            .intern_ascii_folded("title")
            .expect("title atom interning in tokenizer harness must succeed");
        let textarea = ctx
            .atoms
            .intern_ascii_folded("textarea")
            .expect("textarea atom interning in tokenizer harness must succeed");
        let script = ctx
            .atoms
            .intern_ascii_folded("script")
            .expect("script atom interning in tokenizer harness must succeed");
        Self {
            style,
            title,
            textarea,
            script,
        }
    }

    pub fn control_for_token(
        &self,
        token: &Token,
        active_text_mode: &mut Option<AtomId>,
    ) -> Option<TokenizerControl> {
        let style = self.style;
        let title = self.title;
        let textarea = self.textarea;
        let script = self.script;
        match token {
            Token::StartTag { name, .. } if active_text_mode.is_none() => {
                let spec = if *name == style {
                    Some(TextModeSpec::rawtext_style(style))
                } else if *name == title {
                    Some(TextModeSpec::rcdata_title(title))
                } else if *name == textarea {
                    Some(TextModeSpec::rcdata_textarea(textarea))
                } else if *name == script {
                    Some(TextModeSpec::script_data(script))
                } else {
                    None
                }?;
                *active_text_mode = Some(*name);
                Some(TokenizerControl::EnterTextMode(spec))
            }
            Token::EndTag { name } if *active_text_mode == Some(*name) => {
                *active_text_mode = None;
                Some(TokenizerControl::ExitTextMode)
            }
            _ => None,
        }
    }
}

use crate::html5::shared::{AtomId, Token};
use crate::html5::tokenizer::{Html5Tokenizer, TextModeSpec, TokenizerControl};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TargetedTextModeHarnessKind {
    RawTextStyle,
    RcdataTitle,
    RcdataTextarea,
    ScriptData,
}

impl TargetedTextModeHarnessKind {
    pub(super) fn end_tag_name_literal(self) -> &'static str {
        match self {
            Self::RawTextStyle => "style",
            Self::RcdataTitle => "title",
            Self::RcdataTextarea => "textarea",
            Self::ScriptData => "script",
        }
    }

    pub(super) fn spec(self, tag_name: AtomId) -> TextModeSpec {
        match self {
            Self::RawTextStyle => TextModeSpec::rawtext_style(tag_name),
            Self::RcdataTitle => TextModeSpec::rcdata_title(tag_name),
            Self::RcdataTextarea => TextModeSpec::rcdata_textarea(tag_name),
            Self::ScriptData => TextModeSpec::script_data(tag_name),
        }
    }
}

pub(super) struct TextModeFuzzController {
    spec: TextModeSpec,
    text_mode_active: bool,
}

impl TextModeFuzzController {
    pub(super) fn new(spec: TextModeSpec) -> Self {
        Self {
            spec,
            text_mode_active: false,
        }
    }

    pub(super) fn enter_initial(&mut self, tokenizer: &mut Html5Tokenizer) {
        assert!(
            !self.text_mode_active,
            "text-mode fuzz controller cannot enter initial mode twice"
        );
        tokenizer.apply_control(TokenizerControl::EnterTextMode(self.spec));
        self.text_mode_active = true;
        self.assert_consistent(tokenizer);
    }

    pub(super) fn note_token(&mut self, token: &Token) -> Option<TokenizerControl> {
        match token {
            Token::StartTag { name, .. }
                if *name == self.spec.end_tag_name && !self.text_mode_active =>
            {
                self.text_mode_active = true;
                Some(TokenizerControl::EnterTextMode(self.spec))
            }
            Token::EndTag { name } if *name == self.spec.end_tag_name && self.text_mode_active => {
                self.text_mode_active = false;
                Some(TokenizerControl::ExitTextMode)
            }
            _ => None,
        }
    }

    pub(super) fn assert_consistent(&self, tokenizer: &Html5Tokenizer) {
        let tokenizer_in_expected_text_mode = tokenizer.active_text_mode == Some(self.spec);
        assert_eq!(
            tokenizer_in_expected_text_mode, self.text_mode_active,
            "text-mode fuzz controller drifted from tokenizer text-mode state"
        );
    }
}

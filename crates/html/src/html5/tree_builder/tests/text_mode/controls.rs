use super::helpers::{Prelude, TextModeHarness};

#[test]
fn tree_builder_emits_explicit_tokenizer_text_mode_controls() {
    use crate::html5::shared::Token;
    use crate::html5::tokenizer::{TextModeSpec, TokenizerControl};
    use crate::html5::tree_builder::modes::InsertionMode;

    let mut h = TextModeHarness::new();
    let textarea = h.atom("textarea");

    for token in h.prelude_tokens(Prelude::Body) {
        let step = h.process(token);
        assert!(
            step.tokenizer_control.is_none(),
            "ordinary body setup must not issue tokenizer text-mode controls"
        );
    }

    let enter = h.process(Token::StartTag {
        name: textarea,
        attrs: Vec::new(),
        self_closing: false,
    });
    assert_eq!(
        enter.tokenizer_control,
        Some(TokenizerControl::EnterTextMode(
            TextModeSpec::rcdata_textarea(textarea),
        )),
        "text container start tag must emit explicit tokenizer entry control"
    );
    let in_text = h.state();
    assert_eq!(in_text.insertion_mode, InsertionMode::Text);
    assert_eq!(
        in_text.active_text_mode,
        Some(TextModeSpec::rcdata_textarea(textarea)),
        "builder must track the exact active text-mode element"
    );

    let exit = h.process(Token::EndTag { name: textarea });
    assert_eq!(
        exit.tokenizer_control,
        Some(TokenizerControl::ExitTextMode),
        "matching text container end tag must emit explicit tokenizer exit control"
    );
    assert_eq!(h.state().insertion_mode, InsertionMode::InBody);
    assert_eq!(
        h.state().active_text_mode,
        None,
        "matching text-mode close must clear the exact active text-mode element"
    );
}

#[test]
fn tree_builder_emits_controls_for_all_supported_text_mode_containers() {
    use crate::html5::shared::Token;
    use crate::html5::tokenizer::{TextModeSpec, TokenizerControl};
    use crate::html5::tree_builder::modes::InsertionMode;

    let cases = [
        ("style", Prelude::Head, InsertionMode::InHead),
        ("title", Prelude::Head, InsertionMode::InHead),
        ("textarea", Prelude::Body, InsertionMode::InBody),
        ("script", Prelude::Body, InsertionMode::InBody),
    ];

    for (tag_name, prelude, restored_mode) in cases {
        let mut h = TextModeHarness::new();
        let tag = h.atom(tag_name);

        for token in h.prelude_tokens(prelude) {
            let step = h.process(token);
            assert!(
                step.tokenizer_control.is_none(),
                "prelude setup must not emit text-mode controls for {tag_name}"
            );
        }

        let enter = h.process(Token::StartTag {
            name: tag,
            attrs: Vec::new(),
            self_closing: false,
        });
        let expected_spec = if tag_name == "style" {
            TextModeSpec::rawtext_style(tag)
        } else if tag_name == "title" {
            TextModeSpec::rcdata_title(tag)
        } else if tag_name == "textarea" {
            TextModeSpec::rcdata_textarea(tag)
        } else {
            TextModeSpec::script_data(tag)
        };
        assert_eq!(
            enter.tokenizer_control,
            Some(TokenizerControl::EnterTextMode(expected_spec)),
            "{tag_name} start tag must emit explicit tokenizer entry control"
        );
        let in_text = h.state();
        assert_eq!(in_text.insertion_mode, InsertionMode::Text);
        assert_eq!(
            in_text.active_text_mode,
            Some(expected_spec),
            "{tag_name} must become the active text-mode element"
        );

        let exit = h.process(Token::EndTag { name: tag });
        assert_eq!(
            exit.tokenizer_control,
            Some(TokenizerControl::ExitTextMode),
            "{tag_name} close tag must emit explicit tokenizer exit control"
        );
        let after_close = h.state();
        assert_eq!(
            after_close.insertion_mode, restored_mode,
            "{tag_name} close must restore the previous insertion mode"
        );
        assert_eq!(
            after_close.active_text_mode, None,
            "{tag_name} close must clear the active text-mode element"
        );
    }
}

#[test]
fn tree_builder_self_closing_text_mode_container_does_not_enter_text_mode() {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let mut h = TextModeHarness::new();
    let textarea = h.atom("textarea");
    h.process_prelude(Prelude::Body);

    let step = h.process(Token::StartTag {
        name: textarea,
        attrs: Vec::new(),
        self_closing: true,
    });
    let state = h.state();
    assert_eq!(
        step.tokenizer_control, None,
        "self-closing syntax must not enter text mode without a corresponding open element"
    );
    assert_eq!(
        state.insertion_mode,
        InsertionMode::InBody,
        "self-closing text-mode containers must leave the builder in the surrounding insertion mode"
    );
    assert_eq!(
        state.active_text_mode, None,
        "self-closing text-mode containers must not become the active text-mode element"
    );
    assert_ne!(
        state.open_element_names.last().copied(),
        Some(textarea),
        "self-closing text-mode container syntax must not leave an open element on the stack"
    );
}

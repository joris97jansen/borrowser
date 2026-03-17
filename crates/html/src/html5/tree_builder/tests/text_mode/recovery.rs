use super::helpers::{Prelude, TextModeHarness};

#[test]
fn tree_builder_text_mode_unexpected_start_tag_does_not_push_stack() {
    use crate::html5::shared::Token;

    let mut h = TextModeHarness::new();
    let textarea = h.enter_text_mode_container(Prelude::Head, "textarea").tag;
    let div = h.atom("div");

    let before_unexpected = h.state();
    let before_depth = before_unexpected.open_element_names.len();
    assert_eq!(
        before_unexpected.open_element_names.last().copied(),
        Some(textarea)
    );

    h.process_ok(Token::StartTag {
        name: div,
        attrs: Vec::new(),
        self_closing: false,
    });

    let after_unexpected = h.state();
    assert_eq!(
        after_unexpected.open_element_names.len(),
        before_depth,
        "unexpected start tag in Text mode must not push SOE"
    );
    assert_eq!(
        after_unexpected.open_element_names.last().copied(),
        Some(textarea),
        "unexpected start tag in Text mode must keep current text node context"
    );

    h.process_ok(Token::EndTag { name: textarea });
    let after_close = h.state();
    assert!(
        !after_close.open_element_names.contains(&div),
        "unexpected start tag in Text mode must not leave pushed element behind"
    );
    assert!(
        after_close.open_element_names.len() <= before_depth,
        "closing text node context should not increase SOE depth"
    );
}

#[test]
fn tree_builder_text_mode_end_tag_for_other_container_literalizes_and_stays_in_text_mode() {
    use crate::html5::shared::Token;
    use crate::html5::tokenizer::TextModeSpec;
    use crate::html5::tree_builder::modes::InsertionMode;

    let mut h = TextModeHarness::new();
    let textarea = h.enter_text_mode_container(Prelude::Head, "textarea").tag;
    let script = h.atom("script");

    let before = h.state();
    assert_eq!(before.insertion_mode, InsertionMode::Text);
    assert_eq!(before.open_element_names.last().copied(), Some(textarea));
    assert_eq!(
        before.active_text_mode,
        Some(TextModeSpec::rcdata_textarea(textarea))
    );

    h.process_ok(Token::EndTag { name: script });

    let after_stray = h.state();
    assert_eq!(after_stray.insertion_mode, InsertionMode::Text);
    assert_eq!(
        after_stray.open_element_names.last().copied(),
        Some(textarea)
    );
    assert_eq!(
        after_stray.active_text_mode,
        Some(TextModeSpec::rcdata_textarea(textarea)),
        "mismatched end tags must keep the exact active text-mode element"
    );

    assert!(
        h.text_patches().iter().any(|text| text == "</script>"),
        "failed text-mode close should literalize the end tag"
    );
}

#[test]
fn tree_builder_text_mode_failed_container_close_reports_single_text_mode_error() {
    use crate::html5::shared::Token;

    let mut h = TextModeHarness::new();
    let _textarea = h.enter_text_mode_container(Prelude::Head, "textarea").tag;
    let script = h.atom("script");

    let _ = h.parse_error_kinds();
    h.process_ok(Token::EndTag { name: script });

    let errors = h.parse_error_kinds();
    assert!(
        errors
            .iter()
            .copied()
            .any(|kind| kind == "unexpected-end-tag-in-text-mode"),
        "failed text-mode close should emit unexpected-end-tag-in-text-mode"
    );
    assert!(
        !errors
            .iter()
            .copied()
            .any(|kind| kind == "end-tag-not-in-scope"),
        "failed text-mode close should suppress generic end-tag-not-in-scope reporting"
    );
    assert_eq!(
        errors
            .iter()
            .copied()
            .filter(|kind| *kind == "unexpected-end-tag-in-text-mode")
            .count(),
        1,
        "failed text-mode close should record exactly one text-mode end-tag error"
    );
}

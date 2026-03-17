use super::helpers::{Prelude, TextModeHarness};

#[test]
fn tree_builder_text_mode_literalization_does_not_coalesce_with_real_text() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::TreeBuilderConfig;

    let mut h = TextModeHarness::with_config(TreeBuilderConfig {
        coalesce_text: true,
    });
    let textarea = h.enter_text_mode_container(Prelude::Head, "textarea").tag;
    let div = h.atom("div");

    h.process_all([
        Token::Text {
            text: TextValue::Owned("a".to_string()),
        },
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::EndTag { name: textarea },
        Token::Eof,
    ]);

    assert_eq!(
        h.text_patches(),
        vec!["a".to_string(), "<div>".to_string(), "b".to_string()]
    );
}

#[test]
fn tree_builder_text_mode_unexpected_end_tag_literalization_normalizes_name() {
    use crate::html5::shared::Token;

    let mut h = TextModeHarness::new();
    let textarea = h.enter_text_mode_container(Prelude::Head, "textarea").tag;
    let mixed_div = h.atom("DiV");

    h.process_all([
        Token::EndTag { name: mixed_div },
        Token::EndTag { name: textarea },
        Token::Eof,
    ]);

    let text_values = h.text_patches();
    assert!(
        text_values.iter().any(|text| text == "</div>"),
        "unexpected end-tag literalization should use folded tag name"
    );
    assert!(
        !text_values.iter().any(|text| text == "</DiV>"),
        "unexpected end-tag literalization must not preserve mixed-case source"
    );
}

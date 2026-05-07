use super::*;

#[test]
fn initial_display_matches_css_initial_value_while_bridge_applies_element_defaults_later() {
    assert!(matches!(
        ComputedStyle::initial().display(),
        Display::Inline
    ));
}

#[test]
fn min_width_auto_clears_previous_length_but_none_is_not_accepted() {
    let style = compute_style(
        Some("div"),
        &[
            ("min-width".to_string(), "10px".to_string()),
            ("min-width".to_string(), "auto".to_string()),
        ],
        None,
    );
    assert!(style.min_width().is_none());

    let style = compute_style(
        Some("div"),
        &[
            ("min-width".to_string(), "10px".to_string()),
            ("min-width".to_string(), "none".to_string()),
        ],
        None,
    );
    assert!(
        matches!(style.min_width(), Some(LengthPercentage::Length(Length::Px(px))) if px == 10.0)
    );
}

#[test]
fn max_width_none_clears_previous_length_but_auto_is_not_accepted() {
    let style = compute_style(
        Some("div"),
        &[
            ("max-width".to_string(), "10px".to_string()),
            ("max-width".to_string(), "none".to_string()),
        ],
        None,
    );
    assert!(style.max_width().is_none());

    let style = compute_style(
        Some("div"),
        &[
            ("max-width".to_string(), "10px".to_string()),
            ("max-width".to_string(), "auto".to_string()),
        ],
        None,
    );
    assert!(
        matches!(style.max_width(), Some(LengthPercentage::Length(Length::Px(px))) if px == 10.0)
    );
}

#[test]
fn legacy_compute_style_uses_property_pipeline_for_invalid_values() {
    let style = compute_style(
        Some("div"),
        &[
            ("padding-left".to_string(), "8px".to_string()),
            ("padding-left".to_string(), "-4px".to_string()),
            ("margin-left".to_string(), "-3px".to_string()),
            ("width".to_string(), "-1px".to_string()),
        ],
        None,
    );

    assert_eq!(style.box_metrics().padding_left, 8.0);
    assert_eq!(style.box_metrics().margin_left, -3.0);
    assert_eq!(style.width(), None);
}

#[test]
fn legacy_compute_style_ignores_invalid_link_color_for_ua_fallback() {
    let invalid = compute_style(
        Some("a"),
        &[("color".to_string(), "nonsense".to_string())],
        None,
    );
    assert_eq!(invalid.color(), (0, 0, 238, 255));

    let valid = compute_style(Some("a"), &[("color".to_string(), "red".to_string())], None);
    assert_eq!(valid.color(), (255, 0, 0, 255));
}

#[test]
fn legacy_build_style_tree_ignores_invalid_display_for_default_bridge() {
    let dom = Node::Element {
        id: Id::INVALID,
        name: Arc::from("div"),
        attributes: Vec::new(),
        style: vec![("display".to_string(), "nonsense".to_string())],
        children: Vec::new(),
    };

    let styled = build_style_tree(&dom, None);

    assert_eq!(styled.style.display(), Display::Block);
}

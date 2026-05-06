use super::super::*;
use super::support::*;
use html::internal::Id;

#[test]
fn layout_projection_accepts_anonymous_boxes_as_layout_participants() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "p", Vec::new(), vec![text(5, "block")]),
            text(6, "after"),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");

    assert_eq!(div.children.len(), 3);
    assert!(div.children[0].is_anonymous());
    assert_eq!(div.children[0].direct_node_id(), None);
    assert_eq!(div.children[0].node_id(), Id(2));
    assert_eq!(div.children[0].children[0].direct_node_id(), Some(Id(3)));
    assert_eq!(div.children[1].direct_node_id(), Some(Id(4)));
    assert!(div.children[2].is_anonymous());
    assert_eq!(div.children[2].children[0].direct_node_id(), Some(Id(6)));
}

#[test]
fn anonymous_layout_boxes_do_not_inherit_anchor_padding_for_sizing() {
    let dom = doc(vec![element(
        2,
        "div",
        vec![
            ("width", "200px"),
            ("padding-left", "10px"),
            ("padding-right", "10px"),
        ],
        vec![
            text(3, "before"),
            element(4, "p", Vec::new(), vec![text(5, "block")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");
    let anonymous = &div.children[0];

    assert!(anonymous.is_anonymous());
    assert_eq!(div.content_x_and_width(), (10.0, 200.0));
    assert_eq!(anonymous.rect.x, 10.0);
    assert_eq!(anonymous.rect.width, 200.0);
    assert_eq!(anonymous.content_x_and_width(), (10.0, 200.0));
}

#[test]
fn layout_resolves_explicit_width_as_content_box_with_padding() {
    let dom = doc(vec![element(
        2,
        "div",
        vec![
            ("width", "100px"),
            ("padding-left", "10px"),
            ("padding-right", "15px"),
        ],
        Vec::new(),
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");

    assert_eq!(div.rect.width, 125.0);
    assert_eq!(div.content_x_and_width(), (10.0, 100.0));
}

#[test]
fn layout_resolves_auto_width_as_available_border_box_after_padding() {
    let dom = doc(vec![element(
        2,
        "div",
        vec![("padding-left", "10px"), ("padding-right", "15px")],
        Vec::new(),
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 200.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");

    assert_eq!(div.rect.width, 200.0);
    assert_eq!(div.content_x_and_width(), (10.0, 175.0));
}

#[test]
fn layout_applies_max_width_to_content_box_before_padding() {
    let dom = doc(vec![element(
        2,
        "div",
        vec![
            ("width", "200px"),
            ("max-width", "120px"),
            ("padding-left", "10px"),
            ("padding-right", "10px"),
        ],
        Vec::new(),
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");

    assert_eq!(div.rect.width, 140.0);
    assert_eq!(div.content_x_and_width(), (10.0, 120.0));
}

#[test]
fn layout_applies_min_width_to_content_box_before_padding() {
    let dom = doc(vec![element(
        2,
        "div",
        vec![
            ("width", "80px"),
            ("min-width", "120px"),
            ("padding-left", "10px"),
            ("padding-right", "10px"),
        ],
        Vec::new(),
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");

    assert_eq!(div.rect.width, 140.0);
    assert_eq!(div.content_x_and_width(), (10.0, 120.0));
}

#[test]
fn layout_resolves_explicit_height_as_content_box_with_padding() {
    let dom = doc(vec![element(
        2,
        "div",
        vec![
            ("height", "40px"),
            ("padding-top", "5px"),
            ("padding-bottom", "7px"),
        ],
        vec![text(3, "ignored by explicit height")],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");

    assert_eq!(div.rect.height, 52.0);
    assert_eq!(div.content_height(), 40.0);
}

#[test]
fn layout_derives_nested_auto_width_from_parent_content_box() {
    let dom = doc(vec![element(
        2,
        "section",
        vec![
            ("width", "200px"),
            ("padding-left", "10px"),
            ("padding-right", "10px"),
        ],
        vec![element(3, "div", Vec::new(), Vec::new())],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let section = find_layout_by_direct_node_id(&layout, Id(2)).expect("section layout box");
    let div = find_layout_by_direct_node_id(&layout, Id(3)).expect("div layout box");

    assert_eq!(section.rect.width, 220.0);
    assert_eq!(section.content_x_and_width(), (10.0, 200.0));
    assert_eq!(div.rect.x, 10.0);
    assert_eq!(div.rect.width, 200.0);
}

#[test]
fn layout_clamps_atomic_inline_width_to_available_content_space() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(
            3,
            "span",
            vec![("display", "inline-block"), ("width", "200px")],
            vec![text(4, "atomic")],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 100.0, &TestMeasurer, None);
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline-block layout box");

    assert_eq!(inline_block.rect.width, 100.0);
    assert_eq!(inline_block.content_x_and_width(), (0.0, 100.0));
}

#[test]
fn layout_uses_intrinsic_width_for_auto_inline_block_content() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(
            3,
            "span",
            vec![
                ("display", "inline-block"),
                ("padding-left", "5px"),
                ("padding-right", "7px"),
            ],
            vec![text(4, "wide")],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline-block layout box");

    assert_eq!(inline_block.rect.width, 44.0);
    assert_eq!(inline_block.content_x_and_width(), (5.0, 32.0));
}

#[test]
fn intrinsic_auto_inline_block_applies_replaced_control_padding_once() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(
            3,
            "span",
            vec![("display", "inline-block")],
            vec![element(
                4,
                "button",
                vec![
                    ("padding-left", "20px"),
                    ("padding-right", "30px"),
                    ("font-size", "16px"),
                ],
                vec![text(5, "go")],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline-block layout box");

    // TestMeasurer: "go" = 16px, button internal chrome = 18px,
    // button intrinsic content width = 34px, CSS padding = 20 + 30,
    // outer inline-block content width = 84px.
    assert_eq!(inline_block.rect.width, 84.0);
    assert_eq!(inline_block.content_x_and_width(), (0.0, 84.0));
}

#[test]
fn layout_shrink_to_fits_auto_inline_block_between_min_and_max_content() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(
            3,
            "span",
            vec![("display", "inline-block")],
            vec![text(4, "hello world")],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 60.0, &TestMeasurer, None);
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline-block layout box");

    assert_eq!(inline_block.rect.width, 60.0);
    assert_eq!(inline_block.content_x_and_width(), (0.0, 60.0));
}

#[test]
fn layout_applies_max_width_after_intrinsic_auto_inline_block_width() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(
            3,
            "span",
            vec![("display", "inline-block"), ("max-width", "50px")],
            vec![text(4, "hello world")],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline-block layout box");

    assert_eq!(inline_block.rect.width, 50.0);
    assert_eq!(inline_block.content_x_and_width(), (0.0, 50.0));
}

#[test]
fn layout_explicit_inline_block_width_overrides_intrinsic_content_width() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(
            3,
            "span",
            vec![("display", "inline-block"), ("width", "100px")],
            vec![text(4, "wide")],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline-block layout box");

    assert_eq!(inline_block.rect.width, 100.0);
    assert_eq!(inline_block.content_x_and_width(), (0.0, 100.0));
}

#[test]
fn layout_projection_preserves_containing_block_metadata() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            element(3, "span", Vec::new(), vec![text(4, "inline")]),
            element(
                5,
                "span",
                vec![("display", "inline-block")],
                vec![text(6, "atomic")],
            ),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);

    let div = find_layout_by_direct_node_id(&layout, Id(2)).expect("div layout box");
    let inline_span =
        find_layout_by_direct_node_id(&layout, Id(3)).expect("inline span layout box");
    let inline_text =
        find_layout_by_direct_node_id(&layout, Id(4)).expect("inline text layout box");
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(5)).expect("inline-block layout box");
    let inline_block_text =
        find_layout_by_direct_node_id(&layout, Id(6)).expect("inline-block text layout box");

    assert!(div.establishes_containing_block());

    assert_eq!(
        inline_span
            .containing_block()
            .map(ContainingBlockId::box_id),
        Some(div.box_id())
    );
    assert!(!inline_span.establishes_containing_block());

    assert_eq!(
        inline_text
            .containing_block()
            .map(ContainingBlockId::box_id),
        Some(div.box_id())
    );
    assert!(!inline_text.establishes_containing_block());

    assert_eq!(
        inline_block
            .containing_block()
            .map(ContainingBlockId::box_id),
        Some(div.box_id())
    );
    assert!(inline_block.establishes_containing_block());

    assert_eq!(
        inline_block_text
            .containing_block()
            .map(ContainingBlockId::box_id),
        Some(inline_block.box_id())
    );
    assert!(!inline_block_text.establishes_containing_block());
}

#[test]
fn layout_projection_preserves_block_formatting_context_metadata() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![element(
                4,
                "div",
                Vec::new(),
                vec![
                    element(5, "span", Vec::new(), vec![text(6, "inline")]),
                    element(
                        7,
                        "span",
                        vec![("display", "inline-block")],
                        vec![text(8, "atomic")],
                    ),
                ],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);

    let html = find_layout_by_direct_node_id(&layout, Id(2)).expect("html layout box");
    let div = find_layout_by_direct_node_id(&layout, Id(4)).expect("div layout box");
    let inline_span =
        find_layout_by_direct_node_id(&layout, Id(5)).expect("inline span layout box");
    let inline_text =
        find_layout_by_direct_node_id(&layout, Id(6)).expect("inline text layout box");
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(7)).expect("inline-block layout box");
    let inline_block_text =
        find_layout_by_direct_node_id(&layout, Id(8)).expect("inline-block text layout box");

    assert_eq!(
        html.establishes_formatting_context(),
        Some(FormattingContextKind::Block)
    );

    assert_eq!(
        div.formatting_context().map(FormattingContextId::box_id),
        Some(html.box_id())
    );
    assert_eq!(div.establishes_formatting_context(), None);
    assert_eq!(
        div.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(
        inline_span
            .formatting_context()
            .map(FormattingContextId::box_id),
        Some(html.box_id())
    );
    assert_eq!(inline_span.establishes_formatting_context(), None);
    assert_eq!(
        inline_span.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel
    );

    assert_eq!(
        inline_text
            .formatting_context()
            .map(FormattingContextId::box_id),
        Some(html.box_id())
    );
    assert_eq!(inline_text.establishes_formatting_context(), None);

    assert_eq!(
        inline_block
            .formatting_context()
            .map(FormattingContextId::box_id),
        Some(html.box_id())
    );
    assert_eq!(
        inline_block.establishes_formatting_context(),
        Some(FormattingContextKind::Block)
    );
    assert_eq!(
        inline_block.block_formatting_participation(),
        BlockFormattingParticipation::AtomicInline
    );

    assert_eq!(
        inline_block_text
            .formatting_context()
            .map(FormattingContextId::box_id),
        Some(inline_block.box_id())
    );
    assert_eq!(inline_block_text.establishes_formatting_context(), None);
}

#[test]
fn layout_projection_preserves_inline_formatting_context_metadata() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![element(
                4,
                "div",
                Vec::new(),
                vec![
                    element(5, "span", Vec::new(), vec![text(6, "inline")]),
                    element(
                        7,
                        "span",
                        vec![("display", "inline-block")],
                        vec![text(8, "atomic")],
                    ),
                ],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);

    let div = find_layout_by_direct_node_id(&layout, Id(4)).expect("div layout box");
    let inline_span =
        find_layout_by_direct_node_id(&layout, Id(5)).expect("inline span layout box");
    let inline_text =
        find_layout_by_direct_node_id(&layout, Id(6)).expect("inline text layout box");
    let inline_block =
        find_layout_by_direct_node_id(&layout, Id(7)).expect("inline-block layout box");
    let inline_block_text =
        find_layout_by_direct_node_id(&layout, Id(8)).expect("inline-block text layout box");

    assert!(div.establishes_inline_formatting_context());
    assert_eq!(div.inline_formatting_context(), None);
    assert_eq!(
        div.inline_formatting_participation(),
        InlineFormattingParticipation::None
    );

    assert_eq!(
        inline_span
            .inline_formatting_context()
            .map(InlineFormattingContextId::box_id),
        Some(div.box_id())
    );
    assert_eq!(
        inline_span.inline_formatting_participation(),
        InlineFormattingParticipation::InlineContainer
    );

    assert_eq!(
        inline_text
            .inline_formatting_context()
            .map(InlineFormattingContextId::box_id),
        Some(div.box_id())
    );
    assert_eq!(
        inline_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );

    assert_eq!(
        inline_block
            .inline_formatting_context()
            .map(InlineFormattingContextId::box_id),
        Some(div.box_id())
    );
    assert!(inline_block.establishes_inline_formatting_context());
    assert_eq!(
        inline_block.inline_formatting_participation(),
        InlineFormattingParticipation::AtomicInline
    );

    assert_eq!(
        inline_block_text
            .inline_formatting_context()
            .map(InlineFormattingContextId::box_id),
        Some(inline_block.box_id())
    );
    assert_eq!(
        inline_block_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );
}

#[test]
fn layout_debug_snapshot_distinguishes_anonymous_box_sources() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "p", Vec::new(), vec![text(5, "block")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let output = crate::layout_document(crate::LayoutPhaseInput::new(
        &styled,
        500.0,
        &TestMeasurer,
        None,
    ));
    let snapshot = output.to_debug_snapshot();

    assert!(snapshot.contains("source=anonymous-block(anchor=2) node=element(\"div\") kind=block"));
    assert!(snapshot.contains("source=dom(3) node=text(\"before\")"));
    assert!(snapshot.contains("source=dom(4) node=element(\"p\")"));
}

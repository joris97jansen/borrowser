use super::super::*;
use super::helpers::*;
use crate::text_measurer::EguiTextMeasurer;
use css::build_style_tree;
use egui::{Context, Event, Modifiers, PointerButton, Vec2};

#[test]
fn link_click_clears_focus_and_returns_navigation() {
    let ctx = Context::default();
    init_context(&ctx);
    let measurer = EguiTextMeasurer::new(&ctx);

    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        Vec::new(),
        vec![
            input_text(2),
            link(3, "https://example.com/next", vec![text(4, "next")]),
        ],
    )]);
    let style_root = build_style_tree(&dom, None);
    let layout_root = layout::layout_block_tree(&style_root, 600.0, &measurer, None);
    let content_size = Vec2::new(600.0, layout_root.rect.height.max(200.0));
    let origin = content_origin(&ctx, content_size);

    let input_rect = find_fragment_rect_for_node(&layout_root, &measurer, Id(2)).unwrap();
    let input_pos = pos_in_rect(origin, input_rect, 2.0, 2.0);
    let link_rect = find_link_fragment_rect(&layout_root, &measurer, Id(3)).unwrap();
    let link_pos = pos_in_rect(origin, link_rect, 1.0, 1.0);

    let mut store = Store::new();
    store.ensure_initial(to_input_id(Id(2)), "hello".to_string());

    let mut interaction = InteractionState::default();
    let form_controls = TestFormControls;

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(input_pos),
            Event::PointerButton {
                pos: input_pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });
    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![Event::PointerButton {
            pos: input_pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(link_pos),
            Event::PointerButton {
                pos: link_pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });
    let action = run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![Event::PointerButton {
            pos: link_pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });

    assert!(interaction.focused_node_id.is_none());
    match action {
        Some(PageAction::Navigate(url)) => {
            assert_eq!(url, "https://example.com/next".to_string());
        }
        _ => panic!("expected PageAction::Navigate"),
    }
}

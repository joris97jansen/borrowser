use super::super::*;
use super::helpers::*;
use crate::text_measurer::EguiTextMeasurer;
use css::build_style_tree;
use egui::{Context, Event, Modifiers, PointerButton, Vec2};

#[test]
fn checkbox_and_radio_activate_on_mouse_and_space() {
    let ctx = Context::default();
    init_context(&ctx);
    let measurer = EguiTextMeasurer::new(&ctx);

    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        Vec::new(),
        vec![input_checkbox(2), input_radio(3)],
    )]);
    let style_root = build_style_tree(&dom, None);
    let layout_root = layout::layout_block_tree(&style_root, 400.0, &measurer, None);
    let content_size = Vec2::new(400.0, layout_root.rect.height.max(200.0));
    let origin = content_origin(&ctx, content_size);

    let rect_checkbox = find_fragment_rect_for_node(&layout_root, &measurer, Id(2)).unwrap();
    let rect_radio = find_fragment_rect_for_node(&layout_root, &measurer, Id(3)).unwrap();
    let pos_checkbox = pos_center(origin, rect_checkbox);
    let pos_radio = pos_center(origin, rect_radio);

    let mut store = Store::new();
    store.ensure_initial_checked(to_input_id(Id(2)), false);
    store.ensure_initial_checked(to_input_id(Id(3)), false);

    let mut interaction = InteractionState::default();
    let form_controls = TestFormControls;

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_checkbox),
            Event::PointerButton {
                pos: pos_checkbox,
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
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_checkbox),
            Event::PointerButton {
                pos: pos_checkbox,
                button: PointerButton::Primary,
                pressed: false,
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
    assert!(store.is_checked(to_input_id(Id(2))));

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_radio),
            Event::PointerButton {
                pos: pos_radio,
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
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_radio),
            Event::PointerButton {
                pos: pos_radio,
                button: PointerButton::Primary,
                pressed: false,
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
    assert!(store.is_checked(to_input_id(Id(3))));

    store.set_checked(to_input_id(Id(2)), false);
    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_checkbox),
            Event::PointerButton {
                pos: pos_checkbox,
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
        raw_input: raw_input(vec![Event::Text(" ".to_string())]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });
    assert!(store.is_checked(to_input_id(Id(2))));

    store.set_checked(to_input_id(Id(3)), false);
    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_radio),
            Event::PointerButton {
                pos: pos_radio,
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
        raw_input: raw_input(vec![Event::Text(" ".to_string())]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });
    assert!(store.is_checked(to_input_id(Id(3))));
}

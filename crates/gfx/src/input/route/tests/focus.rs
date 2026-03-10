use super::super::*;
use super::helpers::*;
use crate::text_measurer::EguiTextMeasurer;
use css::build_style_tree;
use egui::{Context, Event, Modifiers, PointerButton, Vec2};

#[test]
fn clicking_input_focuses_and_blurs_previous_input() {
    let ctx = Context::default();
    init_context(&ctx);
    let measurer = EguiTextMeasurer::new(&ctx);

    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        Vec::new(),
        vec![input_text(2), input_text(3)],
    )]);
    let style_root = build_style_tree(&dom, None);
    let layout_root = layout::layout_block_tree(&style_root, 400.0, &measurer, None);
    let content_size = Vec2::new(400.0, layout_root.rect.height.max(200.0));
    let origin = content_origin(&ctx, content_size);

    let rect_a = find_fragment_rect_for_node(&layout_root, &measurer, Id(2)).unwrap();
    let rect_b = find_fragment_rect_for_node(&layout_root, &measurer, Id(3)).unwrap();
    let pos_a = pos_in_rect(origin, rect_a, 2.0, 2.0);
    let pos_b = pos_in_rect(origin, rect_b, 2.0, 2.0);

    let mut store = Store::new();
    store.ensure_initial(to_input_id(Id(2)), "hello".to_string());
    store.ensure_initial(to_input_id(Id(3)), "world".to_string());

    let mut interaction = InteractionState::default();
    let form_controls = TestFormControls;

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_a),
            Event::PointerButton {
                pos: pos_a,
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
            pos: pos_a,
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

    store.set_caret(to_input_id(Id(2)), 0, false);
    store.set_caret(to_input_id(Id(2)), 5, true);
    assert!(store.get_state(to_input_id(Id(2))).unwrap().2.is_some());

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(pos_b),
            Event::PointerButton {
                pos: pos_b,
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

    assert_eq!(interaction.focused_node_id, Some(Id(3)));
    assert!(store.get_state(to_input_id(Id(2))).unwrap().2.is_none());
}

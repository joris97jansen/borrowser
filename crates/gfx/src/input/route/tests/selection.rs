use super::super::*;
use super::helpers::*;
use crate::text_measurer::EguiTextMeasurer;
use crate::util::input_text_padding;
use css::build_style_tree;
use egui::{Context, Event, Modifiers, PointerButton, Vec2};
use input_core::SelectionRange;

#[test]
fn drag_selection_updates_caret_and_selection() {
    let ctx = Context::default();
    init_context(&ctx);
    let measurer = EguiTextMeasurer::new(&ctx);

    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        Vec::new(),
        vec![input_text(2)],
    )]);
    let style_root = build_style_tree(&dom, None);
    let layout_root = layout::layout_block_tree(&style_root, 500.0, &measurer, None);
    let content_size = Vec2::new(500.0, layout_root.rect.height.max(200.0));
    let origin = content_origin(&ctx, content_size);

    let rect = find_fragment_rect_for_node(&layout_root, &measurer, Id(2)).unwrap();
    let start_local_x = 4.0;
    let end_local_x = (rect.width * 0.8).max(start_local_x + 1.0);
    let start_pos = pos_in_rect(origin, rect, start_local_x, 2.0);
    let end_pos = pos_in_rect(origin, rect, end_local_x, 2.0);

    let mut store = Store::new();
    let value = "hello world";
    store.ensure_initial(to_input_id(Id(2)), value.to_string());

    let mut interaction = InteractionState::default();
    let form_controls = TestFormControls;

    run_frame(FrameRun {
        ctx: &ctx,
        raw_input: raw_input(vec![
            Event::PointerMoved(start_pos),
            Event::PointerButton {
                pos: start_pos,
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
        raw_input: raw_input(vec![Event::PointerMoved(end_pos)]),
        layout_root: &layout_root,
        measurer: &measurer,
        base_url: None,
        input_values: &mut store,
        form_controls: &form_controls,
        interaction: &mut interaction,
        content_size,
        layout_changed: false,
    });

    let lb = crate::text_control::find_layout_box_by_id(&layout_root, Id(2)).unwrap();
    let style = lb.style;
    let (pad_l, _pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let expected_start =
        expected_caret_for_x(&measurer, style, value, (start_local_x - pad_l).max(0.0));
    let expected_end =
        expected_caret_for_x(&measurer, style, value, (end_local_x - pad_l).max(0.0));
    let expected_sel = SelectionRange {
        start: expected_start.min(expected_end),
        end: expected_start.max(expected_end),
    };

    let (_, caret, selection, _, _) = store.get_state(to_input_id(Id(2))).unwrap();
    assert_eq!(caret, expected_end);
    assert_eq!(selection, Some(expected_sel));
}

use crate::input::InputValueStore;
use css::ComputedStyle;
use html::Id;
use layout::{LayoutBox, TextMeasurer};

pub(crate) fn clamp_caret_to_boundary(value: &str, caret: usize) -> usize {
    let mut caret = caret.min(value.len());
    while caret > 0 && !value.is_char_boundary(caret) {
        caret -= 1;
    }
    caret
}

pub(crate) fn input_text_padding(style: &ComputedStyle) -> (f32, f32, f32, f32) {
    let bm = style.box_metrics;
    let pad_l = bm.padding_left.max(4.0);
    let pad_r = bm.padding_right.max(4.0);
    let pad_t = bm.padding_top.max(2.0);
    let pad_b = bm.padding_bottom.max(2.0);
    (pad_l, pad_r, pad_t, pad_b)
}

pub(crate) fn sync_input_scroll_for_caret(
    input_values: &mut InputValueStore,
    input_id: Id,
    input_rect_w: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) {
    let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (input_rect_w - pad_l - pad_r).max(0.0);

    let (caret_px, text_w) = match input_values.get_state(input_id) {
        Some((value, caret, _sel, _scroll_x, _scroll_y)) => {
            let caret = clamp_caret_to_boundary(value, caret);
            (
                measurer.measure(&value[..caret], style),
                measurer.measure(value, style),
            )
        }
        None => (0.0, 0.0),
    };

    input_values.update_scroll_for_caret(input_id, caret_px, text_w, available_text_w);
}

pub(crate) fn consume_focus_nav_keys(i: &mut egui::InputState) {
    // Prevent egui / other widgets from hijacking these while a DOM input is focused:
    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
}

pub(crate) fn find_layout_box_by_id<'a>(
    root: &'a LayoutBox<'a>,
    id: Id,
) -> Option<&'a LayoutBox<'a>> {
    if root.node_id() == id {
        return Some(root);
    }
    for c in &root.children {
        if let Some(found) = find_layout_box_by_id(c, id) {
            return Some(found);
        }
    }
    None
}

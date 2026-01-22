use crate::input::to_input_id;
use crate::util::{clamp_to_char_boundary, input_text_padding};
use css::ComputedStyle;
use html::types::Id;
use input_core::{InputStore, caret_from_x_with_boundaries, rebuild_cursor_boundaries};
use layout::{LayoutBox, TextMeasurer};

/// Update scroll position to keep the caret visible in an input field.
///
/// Takes `html::types::Id` and converts to `InputId` internally for store operations.
pub(crate) fn sync_input_scroll_for_caret<S: InputStore + ?Sized>(
    input_values: &mut S,
    input_id: Id,
    input_rect_w: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) {
    let core_id = to_input_id(input_id);
    let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (input_rect_w - pad_l - pad_r).max(0.0);

    let (caret_px, text_w) = match input_values.get_state(core_id) {
        Some((value, caret, _sel, _scroll_x, _scroll_y)) => {
            let caret = clamp_to_char_boundary(value, caret);
            (
                measurer.measure(&value[..caret], style),
                measurer.measure(value, style),
            )
        }
        None => (0.0, 0.0),
    };

    input_values.update_scroll_for_caret(core_id, caret_px, text_w, available_text_w);
}

/// Set the caret based on a viewport x-coordinate for single-line inputs.
pub(crate) fn set_input_caret_from_viewport_x<S: InputStore + ?Sized>(
    input_values: &mut S,
    input_id: Id,
    x_in_viewport: f32,
    selecting: bool,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> usize {
    let core_id = to_input_id(input_id);
    let Some((value, _caret, _sel, scroll_x, _scroll_y)) = input_values.get_state(core_id) else {
        input_values.set_caret(core_id, 0, selecting);
        return 0;
    };

    let mut boundaries = Vec::new();
    rebuild_cursor_boundaries(value, &mut boundaries);

    let x_in_viewport = x_in_viewport.max(0.0);
    let x_in_text = x_in_viewport + scroll_x.max(0.0);

    let caret = caret_from_x_with_boundaries(value, &boundaries, x_in_text, |s| {
        measurer.measure(s, style)
    });

    input_values.set_caret(core_id, caret, selecting);
    caret
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

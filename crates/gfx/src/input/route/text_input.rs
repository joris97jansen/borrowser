use super::super::to_input_id;
use crate::EguiTextMeasurer;
use crate::text_control::{set_input_caret_from_viewport_x, sync_input_scroll_for_caret};
use crate::util::input_text_padding;
use input_core::InputStore;

pub(super) fn place_caret_from_pointer_hit<S: InputStore + ?Sized>(
    input_values: &mut S,
    hit: &layout::hit_test::HitResult,
    measurer: &EguiTextMeasurer,
    style: &css::ComputedStyle,
    selecting: bool,
) {
    let (pad_l, _pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let x_in_viewport = (hit.local_pos.0 - pad_l).max(0.0);
    set_input_caret_from_viewport_x(
        input_values,
        hit.node_id,
        x_in_viewport,
        selecting,
        measurer,
        style,
    );
    sync_input_scroll_for_caret(
        input_values,
        hit.node_id,
        hit.fragment_rect.width.max(1.0),
        measurer,
        style,
    );
}

pub(super) fn drag_selection<S: InputStore + ?Sized>(
    input_values: &mut S,
    input_id: html::internal::Id,
    local_x: f32,
    viewport_width: f32,
    measurer: &EguiTextMeasurer,
    style: &css::ComputedStyle,
) {
    let (pad_l, _pad_r, _pad_t, _pad_b) = input_text_padding(style);
    set_input_caret_from_viewport_x(
        input_values,
        input_id,
        (local_x - pad_l).max(0.0),
        true,
        measurer,
        style,
    );
    sync_input_scroll_for_caret(
        input_values,
        input_id,
        viewport_width.max(1.0),
        measurer,
        style,
    );
}

pub(super) fn sync_after_edit<S: InputStore + ?Sized>(
    input_values: &mut S,
    input_id: html::internal::Id,
    viewport_width: f32,
    measurer: &EguiTextMeasurer,
    style: &css::ComputedStyle,
) {
    sync_input_scroll_for_caret(
        input_values,
        input_id,
        viewport_width.max(1.0),
        measurer,
        style,
    );
}

pub(super) fn handle_key_event<S: InputStore + ?Sized>(
    input_values: &mut S,
    focus_id: html::internal::Id,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> (bool, bool) {
    match key {
        egui::Key::Backspace => {
            input_values.backspace(to_input_id(focus_id));
            (true, false)
        }
        egui::Key::Delete => {
            input_values.delete(to_input_id(focus_id));
            (true, false)
        }
        egui::Key::ArrowLeft => {
            input_values.move_caret_left(to_input_id(focus_id), modifiers.shift);
            (false, true)
        }
        egui::Key::ArrowRight => {
            input_values.move_caret_right(to_input_id(focus_id), modifiers.shift);
            (false, true)
        }
        egui::Key::Home => {
            input_values.move_caret_to_start(to_input_id(focus_id), modifiers.shift);
            (false, true)
        }
        egui::Key::End => {
            input_values.move_caret_to_end(to_input_id(focus_id), modifiers.shift);
            (false, true)
        }
        egui::Key::A if modifiers.command || modifiers.ctrl => {
            input_values.select_all(to_input_id(focus_id));
            (false, true)
        }
        _ => (false, false),
    }
}

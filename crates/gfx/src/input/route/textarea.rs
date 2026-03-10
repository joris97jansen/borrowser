use super::super::{InteractionState, to_input_id};
use crate::EguiTextMeasurer;
use crate::textarea::{
    TextareaVerticalMoveCtx, sync_textarea_scroll_for_caret, textarea_caret_for_x_in_lines,
    textarea_line_index_from_y, textarea_move_caret_vertically,
};
use crate::util::input_text_padding;
use input_core::InputStore;
use layout::TextMeasurer;

pub(super) struct TextareaDragParams<'a> {
    pub(super) input_id: html::internal::Id,
    pub(super) local_x: f32,
    pub(super) local_y: f32,
    pub(super) viewport_width: f32,
    pub(super) viewport_height: f32,
    pub(super) style: &'a css::ComputedStyle,
}

pub(super) fn place_caret_from_pointer_hit<S: InputStore + ?Sized>(
    input_values: &mut S,
    interaction: &mut InteractionState,
    hit: &layout::hit_test::HitResult,
    measurer: &EguiTextMeasurer,
    style: &css::ComputedStyle,
    selecting: bool,
) {
    let (pad_l, pad_r, pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (hit.fragment_rect.width - pad_l - pad_r).max(0.0);
    let lines = interaction.textarea.ensure_layout_cache(
        &*input_values,
        hit.node_id,
        available_text_w,
        measurer,
        style,
    );

    let caret = {
        let (value, scroll_y) = input_values
            .get_state(to_input_id(hit.node_id))
            .map(|(v, _c, _sel, _sx, sy)| (v, sy))
            .unwrap_or(("", 0.0));

        let y_in_viewport = (hit.local_pos.1 - pad_t).max(0.0);
        let y_in_text = y_in_viewport + scroll_y;
        let line_h = measurer.line_height(style);
        let line_idx = textarea_line_index_from_y(lines, y_in_text, line_h);
        let x_in_viewport = (hit.local_pos.0 - pad_l).max(0.0);
        textarea_caret_for_x_in_lines(lines, value, line_idx, x_in_viewport)
    };

    input_values.set_caret(to_input_id(hit.node_id), caret, selecting);
    sync_textarea_scroll_for_caret(
        input_values,
        hit.node_id,
        hit.fragment_rect.height.max(1.0),
        lines,
        measurer,
        style,
    );
}

pub(super) fn drag_selection<S: InputStore + ?Sized>(
    input_values: &mut S,
    interaction: &mut InteractionState,
    params: TextareaDragParams<'_>,
    measurer: &EguiTextMeasurer,
) {
    let TextareaDragParams {
        input_id,
        local_x,
        local_y,
        viewport_width,
        viewport_height,
        style,
    } = params;
    interaction.textarea.clear_preferred_x();
    let (pad_l, pad_r, pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (viewport_width - pad_l - pad_r).max(0.0);
    let lines = interaction.textarea.ensure_layout_cache(
        &*input_values,
        input_id,
        available_text_w,
        measurer,
        style,
    );

    let caret = {
        let (value, scroll_y) = input_values
            .get_state(to_input_id(input_id))
            .map(|(v, _c, _sel, _sx, sy)| (v, sy))
            .unwrap_or(("", 0.0));

        let y_in_viewport = (local_y - pad_t).max(0.0);
        let y_in_text = y_in_viewport + scroll_y;
        let line_h = measurer.line_height(style);
        let line_idx = textarea_line_index_from_y(lines, y_in_text, line_h);
        let x_in_viewport = (local_x - pad_l).max(0.0);
        textarea_caret_for_x_in_lines(lines, value, line_idx, x_in_viewport)
    };

    input_values.set_caret(to_input_id(input_id), caret, true);
    sync_textarea_scroll_for_caret(
        input_values,
        input_id,
        viewport_height.max(1.0),
        lines,
        measurer,
        style,
    );
}

pub(super) fn move_caret_vertically<S: InputStore + ?Sized>(
    input_values: &mut S,
    interaction: &mut InteractionState,
    layout_root: &layout::LayoutBox<'_>,
    focus_id: html::internal::Id,
    delta: i32,
    measurer: &EguiTextMeasurer,
    modifiers: egui::Modifiers,
) -> bool {
    let Some(lb) = crate::text_control::find_layout_box_by_id(layout_root, focus_id)
        .filter(|lb| matches!(lb.replaced, Some(layout::ReplacedKind::TextArea)))
    else {
        return false;
    };

    let viewport = interaction.focused_input_rect.unwrap_or(lb.rect);
    let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(lb.style);
    let available_text_w = (viewport.width - pad_l - pad_r).max(0.0);
    let preferred_x = interaction.textarea.preferred_x();
    let new_preferred_x = {
        let lines = interaction.textarea.ensure_layout_cache(
            &*input_values,
            focus_id,
            available_text_w,
            measurer,
            lb.style,
        );
        let ctx = TextareaVerticalMoveCtx {
            lines,
            measurer,
            style: lb.style,
        };
        textarea_move_caret_vertically(
            input_values,
            focus_id,
            delta,
            preferred_x,
            ctx,
            modifiers.shift,
        )
    };
    interaction.textarea.set_preferred_x(new_preferred_x);
    true
}

pub(super) fn sync_after_edit<S: InputStore + ?Sized>(
    input_values: &mut S,
    interaction: &mut InteractionState,
    focus_id: html::internal::Id,
    viewport: layout::Rectangle,
    measurer: &EguiTextMeasurer,
    style: &css::ComputedStyle,
) {
    let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (viewport.width - pad_l - pad_r).max(0.0);
    let lines = interaction.textarea.ensure_layout_cache(
        &*input_values,
        focus_id,
        available_text_w,
        measurer,
        style,
    );
    sync_textarea_scroll_for_caret(
        input_values,
        focus_id,
        viewport.height.max(1.0),
        lines,
        measurer,
        style,
    );
}

pub(super) fn handle_linear_key_event<S: InputStore + ?Sized>(
    input_values: &mut S,
    interaction: &mut InteractionState,
    focus_id: html::internal::Id,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> (bool, bool, bool) {
    match key {
        egui::Key::Backspace => {
            interaction.textarea.clear_preferred_x();
            input_values.backspace(to_input_id(focus_id));
            (true, false, false)
        }
        egui::Key::Delete => {
            interaction.textarea.clear_preferred_x();
            input_values.delete(to_input_id(focus_id));
            (true, false, false)
        }
        egui::Key::ArrowLeft => {
            interaction.textarea.clear_preferred_x();
            input_values.move_caret_left(to_input_id(focus_id), modifiers.shift);
            (false, true, false)
        }
        egui::Key::ArrowRight => {
            interaction.textarea.clear_preferred_x();
            input_values.move_caret_right(to_input_id(focus_id), modifiers.shift);
            (false, true, false)
        }
        egui::Key::Home => {
            interaction.textarea.clear_preferred_x();
            input_values.move_caret_to_start(to_input_id(focus_id), modifiers.shift);
            (false, true, false)
        }
        egui::Key::End => {
            interaction.textarea.clear_preferred_x();
            input_values.move_caret_to_end(to_input_id(focus_id), modifiers.shift);
            (false, true, false)
        }
        egui::Key::A if modifiers.command || modifiers.ctrl => {
            interaction.textarea.clear_preferred_x();
            input_values.select_all(to_input_id(focus_id));
            (false, true, false)
        }
        egui::Key::Enter => (false, false, true),
        _ => (false, false, false),
    }
}

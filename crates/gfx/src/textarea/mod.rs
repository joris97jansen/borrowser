mod cache;
mod caret;
mod selection;

use crate::EguiTextMeasurer;
use crate::input::to_input_id;
use crate::util::{clamp_to_char_boundary, input_text_padding};
use css::{ComputedStyle, Length};
use html::internal::Id;
use input_core::InputStore;

pub use cache::{TextareaCachedLine, TextareaCachedTextFragment, TextareaLayoutCache};

pub(crate) use cache::{layout_textarea_cached_lines, textarea_text_height};
pub(crate) use caret::{
    TextareaVerticalMoveCtx, textarea_caret_for_x_in_lines, textarea_caret_geometry,
    textarea_line_index_from_y, textarea_move_caret_vertically,
};
pub(crate) use selection::{TextareaSelectionPaintParams, paint_textarea_selection};

#[derive(Default, Debug)]
pub(crate) struct TextareaState {
    layout_cache: Option<TextareaLayoutCache>,
    preferred_x: Option<f32>,
}

impl TextareaState {
    pub(crate) fn clear_focus(&mut self) {
        self.preferred_x = None;
    }

    pub(crate) fn clear_for_navigation(&mut self) {
        self.layout_cache = None;
        self.preferred_x = None;
    }

    pub(crate) fn clear_preferred_x(&mut self) {
        self.preferred_x = None;
    }

    pub(crate) fn preferred_x(&self) -> Option<f32> {
        self.preferred_x
    }

    pub(crate) fn set_preferred_x(&mut self, x: Option<f32>) {
        self.preferred_x = x;
    }

    pub(crate) fn focused_lines(&self, input_id: Id) -> Option<&[TextareaCachedLine]> {
        self.layout_cache
            .as_ref()
            .filter(|c| c.input_id == input_id)
            .map(|c| c.lines.as_slice())
    }

    pub(crate) fn ensure_layout_cache<'a, S: InputStore + ?Sized>(
        &'a mut self,
        input_values: &S,
        input_id: Id,
        available_text_w: f32,
        measurer: &EguiTextMeasurer,
        style: &ComputedStyle,
    ) -> &'a [TextareaCachedLine] {
        let core_id = to_input_id(input_id);
        let available_text_w = available_text_w.max(0.0);
        let value_rev = input_values.value_revision(core_id);
        let Length::Px(font_px) = style.font_size;

        let cache_valid = self.layout_cache.as_ref().is_some_and(|c| {
            c.input_id == input_id
                && (c.available_text_w - available_text_w).abs() <= 0.5
                && (c.font_px - font_px).abs() <= 0.01
                && c.value_rev == value_rev
        });

        if !cache_valid {
            let value = input_values.get(core_id).unwrap_or("");
            let lines =
                layout_textarea_cached_lines(measurer, style, available_text_w, value, true);
            self.layout_cache = Some(TextareaLayoutCache {
                input_id,
                available_text_w,
                font_px,
                value_rev,
                lines,
            });
        }

        self.focused_lines(input_id).unwrap_or(&[])
    }
}

/// Update scroll position to keep the caret visible in a textarea.
///
/// Takes `html::internal::Id` and converts to `InputId` internally for store operations.
pub(crate) fn sync_textarea_scroll_for_caret<S: InputStore + ?Sized>(
    input_values: &mut S,
    input_id: Id,
    control_rect_h: f32,
    lines: &[TextareaCachedLine],
    measurer: &dyn layout::TextMeasurer,
    style: &ComputedStyle,
) {
    let core_id = to_input_id(input_id);
    let (_pad_l, _pad_r, pad_t, pad_b) = input_text_padding(style);
    let available_text_h = (control_rect_h - pad_t - pad_b).max(0.0);

    let (caret_y, caret_h, text_h) = {
        let Some((value, caret, _sel, _scroll_x, _scroll_y)) = input_values.get_state(core_id)
        else {
            return;
        };

        let caret = clamp_to_char_boundary(value, caret);
        let (_cx, caret_y, caret_h) = textarea_caret_geometry(lines, value, caret, measurer, style);
        let text_h = cache::textarea_text_height(lines, measurer.line_height(style));

        (caret_y, caret_h, text_h)
    };

    input_values.update_scroll_for_caret_y(core_id, caret_y, caret_h, text_h, available_text_h);
}

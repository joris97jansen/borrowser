use super::InputValueStore;
use crate::id::InputId;

impl InputValueStore {
    /// Update horizontal scroll to keep the caret visible.
    ///
    /// # Arguments
    ///
    /// * `id` - The input ID
    /// * `caret_px` - The caret's x position in text coordinates
    /// * `text_w` - Total width of the text content
    /// * `available_w` - Width of the visible viewport
    pub fn update_scroll_for_caret(
        &mut self,
        id: InputId,
        caret_px: f32,
        text_w: f32,
        available_w: f32,
    ) {
        self.with_state_mut(id, |state| {
            let available_w = available_w.max(0.0);
            let text_w = text_w.max(0.0);
            let caret_px = caret_px.clamp(0.0, text_w);

            if available_w <= 0.0 || text_w <= available_w {
                state.scroll_x = 0.0;
                return;
            }

            let max_scroll = (text_w - available_w).max(0.0);
            let mut scroll_x = state.scroll_x.clamp(0.0, max_scroll);

            // Keep the caret visible with a small margin, but don't re-center unless needed.
            let margin: f32 = 4.0;
            let left_limit = margin.min(available_w);
            let right_limit = (available_w - margin).max(left_limit);

            let caret_in_view = caret_px - scroll_x;
            if caret_in_view < left_limit {
                scroll_x = (caret_px - left_limit).max(0.0);
            } else if caret_in_view > right_limit {
                scroll_x = (caret_px - right_limit).min(max_scroll);
            }

            state.scroll_x = scroll_x;
        });
    }

    /// Update vertical scroll to keep the caret visible (for multi-line inputs).
    ///
    /// # Arguments
    ///
    /// * `id` - The input ID
    /// * `caret_y` - The caret's y position in text coordinates
    /// * `caret_h` - Height of the caret/line
    /// * `text_h` - Total height of the text content
    /// * `available_h` - Height of the visible viewport
    pub fn update_scroll_for_caret_y(
        &mut self,
        id: InputId,
        caret_y: f32,
        caret_h: f32,
        text_h: f32,
        available_h: f32,
    ) {
        self.with_state_mut(id, |state| {
            let available_h = available_h.max(0.0);
            let text_h = text_h.max(0.0);
            let caret_h = caret_h.max(0.0);
            let caret_y = caret_y.clamp(0.0, text_h);

            if available_h <= 0.0 || text_h <= available_h {
                state.scroll_y = 0.0;
                return;
            }

            let max_scroll = (text_h - available_h).max(0.0);
            let mut scroll_y = state.scroll_y.clamp(0.0, max_scroll);

            // Keep the caret visible with a small margin, but don't re-center unless needed.
            let margin: f32 = 4.0;
            let top_limit = margin.min(available_h);
            let bottom_limit = (available_h - margin).max(top_limit);

            let caret_top_in_view = caret_y - scroll_y;
            let caret_bottom_in_view = caret_top_in_view + caret_h;

            if caret_top_in_view < top_limit {
                scroll_y = (caret_y - top_limit).max(0.0);
            } else if caret_bottom_in_view > bottom_limit {
                scroll_y = (caret_y + caret_h - bottom_limit).min(max_scroll);
            }

            state.scroll_y = scroll_y;
        });
    }
}

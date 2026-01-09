//! Internal input state representation.
//!
//! This module contains the per-input state that is stored in the InputValueStore.

/// Internal state for a single input element.
///
/// This is not exposed publicly; it is managed by [`InputValueStore`](crate::InputValueStore).
#[derive(Clone, Debug)]
pub(crate) struct InputState {
    /// The current text value.
    pub value: String,

    /// Monotonic revision counter, incremented on any text change.
    /// Useful for cache invalidation.
    pub value_rev: u64,

    /// For checkbox/radio inputs: whether the control is checked.
    pub checked: bool,

    /// Caret position as a byte index into `value` (always on a UTF-8 char boundary).
    pub caret: usize,

    /// Selection anchor as a byte index into `value` (UTF-8 char boundary).
    ///
    /// When `Some(anchor)`, the selection range is `min(anchor, caret)..max(anchor, caret)`.
    pub selection_anchor: Option<usize>,

    /// Horizontal scroll offset in px for single-line inputs.
    pub scroll_x: f32,

    /// Vertical scroll offset in px for multi-line text controls (e.g. `<textarea>`).
    pub scroll_y: f32,

    /// Cached cursor boundary positions for efficient caret-from-x calculations.
    pub cursor_boundaries: Vec<usize>,

    /// Whether the cursor boundaries need to be rebuilt.
    pub cursor_boundaries_dirty: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            value: String::new(),
            value_rev: 0,
            checked: false,
            caret: 0,
            selection_anchor: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            cursor_boundaries: Vec::new(),
            cursor_boundaries_dirty: true,
        }
    }
}

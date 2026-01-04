use html::Id;
use layout::{HitKind, Rectangle};

#[derive(Clone, Debug)]
pub struct TextareaCachedTextFragment {
    pub rect: Rectangle,
    pub source_range: Option<(usize, usize)>,
    /// Absolute byte indices (UTF-8) into the textarea value at each caret boundary.
    /// Length is `chars_in_fragment + 1`.
    pub byte_positions: Vec<usize>,
    /// X-advance (in px) at each caret boundary, relative to the fragment's left edge.
    /// Same length as `byte_positions`.
    pub x_advances: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct TextareaCachedLine {
    pub rect: Rectangle,
    pub source_range: Option<(usize, usize)>,
    pub fragments: Vec<TextareaCachedTextFragment>,
}

#[derive(Clone, Debug)]
pub struct TextareaLayoutCache {
    pub input_id: Id,
    pub available_text_w: f32,
    pub font_px: f32,
    pub value_rev: u64,
    pub lines: Vec<TextareaCachedLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTarget {
    pub id: Id,
    pub kind: HitKind,
}

#[derive(Debug, Clone)]
pub struct InputDragState {
    pub input_id: Id,
    pub rect: Rectangle,
}

#[derive(Default, Debug)]
pub struct InteractionState {
    pub hover: Option<Id>,
    pub hover_kind: Option<HitKind>,
    pub active: Option<ActiveTarget>,
    pub focused_node_id: Option<Id>,
    pub focused_kind: Option<HitKind>,
    pub input_drag: Option<InputDragState>,
    pub focused_input_rect: Option<Rectangle>,
    pub textarea_layout_cache: Option<TextareaLayoutCache>,
    /// Preferred horizontal caret position (in px) for `<textarea>` vertical navigation (ArrowUp/Down).
    /// This is cleared on any non-vertical caret movement.
    pub textarea_preferred_x: Option<f32>,
    pub last_viewport_width: Option<f32>,
    pub last_layout_root_size: Option<(f32, f32)>,
}

impl InteractionState {
    pub fn clear_focus(&mut self) {
        self.focused_node_id = None;
        self.focused_kind = None;
        self.focused_input_rect = None;
        self.textarea_preferred_x = None;
    }

    pub fn set_focus(&mut self, id: Id, kind: HitKind, rect: Rectangle) {
        self.focused_node_id = Some(id);
        self.focused_kind = Some(kind);
        self.focused_input_rect = Some(rect);
        self.textarea_preferred_x = None;
    }

    pub fn clear_for_navigation(&mut self) {
        self.hover = None;
        self.hover_kind = None;
        self.active = None;
        self.clear_focus();
        self.input_drag = None;
        self.textarea_layout_cache = None;
        self.textarea_preferred_x = None;
        self.last_viewport_width = None;
        self.last_layout_root_size = None;
    }
}

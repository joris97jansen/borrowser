use html::Id;
use layout::{HitKind, Rectangle};

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

#[derive(Default, Debug, Clone)]
pub struct InteractionState {
    pub hover: Option<Id>,
    pub hover_kind: Option<HitKind>,
    pub active: Option<ActiveTarget>,
    pub focused_node_id: Option<Id>,
    pub focused_kind: Option<HitKind>,
    pub input_drag: Option<InputDragState>,
    pub focused_input_rect: Option<Rectangle>,
    pub last_viewport_width: Option<f32>,
    pub last_layout_root_size: Option<(f32, f32)>,
}

impl InteractionState {
    pub fn clear_focus(&mut self) {
        self.focused_node_id = None;
        self.focused_kind = None;
        self.focused_input_rect = None;
    }

    pub fn set_focus(&mut self, id: Id, kind: HitKind, rect: Rectangle) {
        self.focused_node_id = Some(id);
        self.focused_kind = Some(kind);
        self.focused_input_rect = Some(rect);
    }

    pub fn clear_for_navigation(&mut self) {
        self.hover = None;
        self.hover_kind = None;
        self.active = None;
        self.clear_focus();
        self.input_drag = None;
        self.last_viewport_width = None;
        self.last_layout_root_size = None;
    }
}

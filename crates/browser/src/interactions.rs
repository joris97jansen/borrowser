use html::Id;

#[derive(Default, Debug, Clone)]
pub struct InteractionState {
    pub hover: Option<Id>,
    pub active: Option<Id>,
    pub focused_node_id: Option<Id>,
}

impl InteractionState {
    pub fn clear_for_navigation(&mut self) {
        self.hover = None;
        self.active = None;
        self.focused_node_id = None;
    }
}

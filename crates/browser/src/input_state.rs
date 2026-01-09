use gfx::input::{InputValueStore, InteractionState};

/// Document-scoped input state owned by the browser layer.
///
/// Lifecycle policy:
/// - Cleared on full document navigations (new URL, reload, back/forward).
/// - Preserved on same-document navigations (e.g. fragment-only).
#[derive(Debug, Default)]
pub struct DocumentInputState {
    pub input_values: InputValueStore,
    pub interaction: InteractionState,
}

impl DocumentInputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_for_navigation(&mut self) {
        self.input_values.clear();
        self.interaction.clear_for_navigation();
    }
}

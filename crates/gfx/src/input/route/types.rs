use super::super::InteractionState;
use crate::EguiTextMeasurer;
use egui::{Pos2, Rect, Ui};
use html::internal::Id;
use input_core::{InputId, InputStore};
use layout::{LayoutBox, Rectangle};
use std::cell::RefCell;
use std::collections::HashMap;

/// Handler for form control interactions that require DOM-level coordination.
///
/// The store parameter `S` is any `InputStore` implementor using `InputId`.
/// Implementors are responsible for converting `html::internal::Id` to `InputId` as needed.
pub trait FormControlHandler<S: InputStore + ?Sized> {
    fn on_radio_clicked(&self, store: &mut S, radio_id: InputId) -> bool;
}

pub(crate) struct FrameInputCtx<'a, 'layout, S: InputStore + ?Sized, F> {
    pub ui: &'a mut Ui,
    pub resp: egui::Response,
    pub content_rect: Rect,
    pub origin: Pos2,
    pub layout_root: &'a LayoutBox<'layout>,
    pub measurer: &'a EguiTextMeasurer,
    pub layout_changed: bool,
    pub fragment_rects: &'a RefCell<HashMap<Id, Rectangle>>,
    pub base_url: Option<&'a str>,
    pub input_values: &'a mut S,
    pub form_controls: &'a F,
    pub interaction: &'a mut InteractionState,
}

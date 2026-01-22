use crate::EguiTextMeasurer;
use crate::input::{ActiveTarget, InputValueStore};
use crate::textarea::TextareaCachedLine;
use egui::{Color32, Painter, Pos2, Stroke};
use html::internal::Id;
use layout::Rectangle;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(crate) struct PaintCtx<'a> {
    pub(crate) painter: &'a Painter,
    pub(crate) origin: Pos2,
    pub(crate) measurer: &'a EguiTextMeasurer,
    pub(crate) base_url: Option<&'a str>,
    pub(crate) resources: &'a dyn super::ImageProvider,
    pub(crate) input_values: &'a InputValueStore,
    pub(crate) focused: Option<Id>,
    pub(crate) focused_textarea_lines: Option<&'a [TextareaCachedLine]>,
    pub(crate) active: Option<ActiveTarget>,
    pub(crate) selection_bg_fill: Color32,
    pub(crate) selection_stroke: Stroke,
    pub(crate) fragment_rects: Option<&'a RefCell<HashMap<Id, Rectangle>>>,
}

impl<'a> PaintCtx<'a> {
    pub(crate) fn with_origin(self, origin: Pos2) -> Self {
        Self { origin, ..self }
    }
}

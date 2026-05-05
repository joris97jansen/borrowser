use crate::rendering::RenderPipelineDebugSnapshot;
#[cfg(test)]
use css::ComputedStyleReuseStats;

use super::PageState;
#[cfg(test)]
use super::{PageStyleGenerations, RestyleHint, RestyleTrigger, StyleRecalcKind};

impl PageState {
    /// Reports the retained/rebuilt policy for rendering artifacts owned or
    /// coordinated by the current page state.
    ///
    /// For frame-local layout and immediate paint output, this snapshot records
    /// the rebuild policy used when the viewport renders a frame. It does not
    /// imply that `PageState` currently retains a live layout tree or paint
    /// artifact between frames.
    pub fn render_pipeline_debug_snapshot(&self) -> RenderPipelineDebugSnapshot {
        self.rendering.debug_snapshot(self.dom.is_some())
    }

    #[cfg(test)]
    pub(crate) fn style_generations(&self) -> PageStyleGenerations {
        self.rendering.generations
    }

    #[cfg(test)]
    pub(crate) fn style_dirty(&self) -> bool {
        self.rendering.style_dirty
    }

    #[cfg(test)]
    pub(crate) fn layout_dirty(&self) -> bool {
        self.rendering.layout_dirty
    }

    #[cfg(test)]
    pub(crate) fn clear_layout_dirty_for_tests(&mut self) {
        self.rendering.layout_dirty = false;
    }

    #[cfg(test)]
    pub(crate) fn mark_dom_changed_for_tests(&mut self, hint: RestyleHint) {
        let _ = self.mark_dom_changed(hint);
    }

    #[cfg(test)]
    pub(crate) fn last_restyle_trigger(&self) -> Option<RestyleTrigger> {
        self.rendering.last_restyle_trigger
    }

    #[cfg(test)]
    pub(crate) fn last_style_recalc(&self) -> Option<StyleRecalcKind> {
        self.rendering.last_style_recalc
    }

    #[cfg(test)]
    pub(crate) fn last_style_reuse(&self) -> Option<ComputedStyleReuseStats> {
        self.rendering.last_style_reuse
    }
}

#[cfg(test)]
use crate::rendering::RenderInvalidationEntryPoint;
use crate::rendering::{RenderPipelineDebugSnapshot, RetainedRenderStateDebugSnapshot};
#[cfg(test)]
use css::ComputedStyleReuseStats;

use super::PageState;
#[cfg(test)]
use super::{PageStyleGenerations, RestyleHint, RestyleTrigger, StyleRecalcKind};

impl PageState {
    /// Reports the retained/rebuilt policy for rendering artifacts owned or
    /// coordinated by the current page state.
    ///
    /// For retained layout and immediate paint output, this snapshot records
    /// the current retained layout artifact state and the immediate paint
    /// output policy. It does not imply retained paint artifacts.
    pub fn render_pipeline_debug_snapshot(&self) -> RenderPipelineDebugSnapshot {
        self.rendering.debug_snapshot(self.dom.is_some())
    }

    /// Reports browser/runtime-owned retained render state for incremental
    /// rendering contracts.
    ///
    /// The render epoch advances when retained runtime rendering state changes.
    /// It is not a frame counter, layout pass counter, paint pass counter,
    /// cache-hit proof, artifact-reuse proof, or stable layout/paint identity.
    pub fn retained_render_state_debug_snapshot(&self) -> RetainedRenderStateDebugSnapshot {
        self.rendering.retained_debug_snapshot(self.dom.is_some())
    }

    #[cfg(test)]
    pub(crate) fn style_generations(&self) -> PageStyleGenerations {
        self.rendering.generations
    }

    #[cfg(test)]
    pub(crate) fn style_dirty(&self) -> bool {
        self.rendering.style_dirty()
    }

    #[cfg(test)]
    pub(crate) fn layout_dirty(&self) -> bool {
        self.rendering.layout_dirty()
    }

    #[cfg(test)]
    pub(crate) fn clear_layout_dirty_for_tests(&mut self) {
        self.rendering.clear_layout_dirty_for_tests();
    }

    #[cfg(test)]
    pub(crate) fn clear_all_dirty_for_tests(&mut self) {
        self.rendering.clear_all_dirty_for_tests();
    }

    #[cfg(test)]
    pub(crate) fn mark_dom_changed_for_tests(&mut self, hint: RestyleHint) {
        let _ = self.mark_dom_changed(hint);
    }

    #[cfg(test)]
    pub(crate) fn mark_render_entry_point_for_tests(
        &mut self,
        entry_point: RenderInvalidationEntryPoint,
    ) {
        self.rendering.mark_dirty_for_entry_point(entry_point);
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

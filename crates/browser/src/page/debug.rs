#[cfg(test)]
use crate::rendering::RenderInvalidationEntryPoint;
#[cfg(test)]
use crate::rendering::{
    CssStyleInvalidationSource, RenderInvalidationRequest, render_css_style_invalidation_request,
};
use crate::rendering::{RenderPipelineDebugSnapshot, RetainedRenderStateDebugSnapshot};
#[cfg(test)]
use css::ComputedStyleReuseStats;

use super::{DomMutationFacts, PageState};
#[cfg(test)]
use super::{PageStyleGenerations, StyleRecalcKind};

impl PageState {
    /// Runs the bounded CSS-owned AF5 collection/match diagnostic over the
    /// exact stylesheet handoff used by production style resolution.
    pub fn rule_collection_debug_snapshot(
        &self,
        style_limits: &css::StyleResolutionLimits,
        diagnostic_limits: css::RuleCollectionDiagnosticLimits,
    ) -> Result<Option<css::RuleCollectionDiagnostic>, css::ComputedStyleResolutionError> {
        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };
        let Some(document_mode) = self.document_mode else {
            return Ok(None);
        };
        let inputs = self
            .rendering
            .document_styles
            .stylesheet_collection_inputs()
            .map_err(|error| {
                css::ComputedStyleResolutionError::StyleResolution(
                    css::StyleResolutionError::StylesheetInputBuild(error),
                )
            })?;
        Ok(Some(css::rule_collection_diagnostic(
            dom,
            css::SelectorMatchingEnvironment::new(document_mode),
            &inputs,
            style_limits,
            diagnostic_limits,
        )))
    }

    /// Runs the bounded CSS-owned AF4 matching diagnostic over the currently
    /// retained document and its real cascade stylesheet inputs.
    pub fn selector_matching_debug_snapshot(
        &self,
        limits: css::DocumentSelectorMatchingDiagnosticLimits,
    ) -> Result<Option<css::DocumentSelectorMatchingDiagnostic>, css::ComputedStyleResolutionError>
    {
        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };
        let Some(document_mode) = self.document_mode else {
            return Ok(None);
        };
        let inputs = self
            .rendering
            .document_styles
            .stylesheet_collection_inputs()
            .map_err(|error| {
                css::ComputedStyleResolutionError::StyleResolution(
                    css::StyleResolutionError::StylesheetInputBuild(error),
                )
            })?;
        Ok(Some(css::document_selector_matching_diagnostic(
            dom,
            css::SelectorMatchingEnvironment::new(document_mode),
            &inputs,
            limits,
        )))
    }

    /// Stable neutral publication facts retained for Browser/runtime
    /// diagnostics. This does not expose or infer selector dependencies.
    pub fn last_dom_mutation_debug_snapshot(&self) -> Option<String> {
        self.rendering
            .last_dom_mutation_facts
            .as_ref()
            .map(DomMutationFacts::to_debug_snapshot)
    }

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
    pub(crate) fn clear_style_cache_for_tests(&mut self) {
        self.rendering.clear_style_cache_for_tests();
    }

    #[cfg(test)]
    pub(crate) fn mark_dom_changed_for_tests(&mut self, facts: DomMutationFacts) {
        let _ = self.mark_dom_changed(facts);
    }

    #[cfg(test)]
    pub(crate) fn mark_render_entry_point_for_tests(
        &mut self,
        entry_point: RenderInvalidationEntryPoint,
    ) {
        self.rendering.mark_dirty_for_entry_point(entry_point);
    }

    #[cfg(test)]
    pub(crate) fn last_dom_mutation_facts(&self) -> Option<&DomMutationFacts> {
        self.rendering.last_dom_mutation_facts.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn css_authorized_request_for_tests(
        &mut self,
        source: CssStyleInvalidationSource,
    ) -> RenderInvalidationRequest {
        match source {
            CssStyleInvalidationSource::DomPublication => self
                .mark_dom_changed(DomMutationFacts::text_changed_for_tests(Vec::new()))
                .requests()
                .find(|request| {
                    request.entry_point()
                        == RenderInvalidationEntryPoint::DomPublicationStyleInvalidated
                })
                .expect("text publication must receive CSS Style authorization"),
            CssStyleInvalidationSource::StylesheetSetChanged => {
                let authorization = self.rendering.mark_stylesheets_changed();
                render_css_style_invalidation_request(source, authorization)
            }
        }
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

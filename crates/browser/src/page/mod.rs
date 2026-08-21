mod debug;
mod dom_mutation;
mod retained_render_state;
mod style_cache;
mod style_phase;
mod stylesheets;

pub(crate) use dom_mutation::{DomMutationFacts, PendingDomMutationFacts};
#[cfg(test)]
pub(crate) use style_cache::{PageStyleGenerations, StyleRecalcKind};
#[cfg(test)]
pub(crate) use style_cache::{
    reset_rule_collection_build_count, rule_collection_build_count, style_execution_build_count,
};
#[allow(unused_imports)]
pub(crate) use stylesheets::PageStylesheetReconcile;

use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use crate::rendering::{
    CssStyleInvalidationSource, IntrinsicRenderInvalidationSource, PendingRenderWork,
    RenderInvalidationRequest, RenderWorkPlan, RenderWorkPlanInput, RetainedPaintArtifactKeySeed,
    RetainedPaintFrameResult, render_css_style_invalidation_request,
    render_intrinsic_invalidation_request,
};
use gfx::input::InputValueStore;
use gfx::paint::PaintArtifact;
use html::{
    Node,
    dom_utils::outline_from_dom,
    head::{HeadMetadata, extract_head_metadata},
};
use layout::{RetainedLayoutArtifact, RetainedLayoutFrameResult, RetainedLayoutKeySeed};

pub(crate) use retained_render_state::AppliedCssStyleInvalidation;
use retained_render_state::RetainedRenderState;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DomPublicationRenderInvalidation {
    intrinsic_requests: Vec<RenderInvalidationRequest>,
    css_style_request: Option<RenderInvalidationRequest>,
}

impl DomPublicationRenderInvalidation {
    #[cfg(test)]
    pub(crate) fn intrinsic_requests(&self) -> &[RenderInvalidationRequest] {
        &self.intrinsic_requests
    }

    #[cfg(test)]
    pub(crate) fn css_style_request(&self) -> Option<RenderInvalidationRequest> {
        self.css_style_request
    }

    /// Iterates intrinsic causes first, followed by the optional single
    /// publication-level CSS Style authorization.
    pub(crate) fn requests(&self) -> impl Iterator<Item = RenderInvalidationRequest> + '_ {
        self.intrinsic_requests
            .iter()
            .copied()
            .chain(self.css_style_request)
    }
}

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Box<Node>>,
    pub(crate) document_mode: Option<html::DocumentMode>,
    pub head: HeadMetadata,

    pub visible_text_cache: String,
    pub form_controls: FormControlIndex,

    rendering: RetainedRenderState,
}

impl PageState {
    pub fn new() -> Self {
        Self {
            base_url: None,
            dom: None,
            document_mode: None,
            head: HeadMetadata::default(),
            visible_text_cache: String::new(),
            form_controls: FormControlIndex::default(),
            rendering: RetainedRenderState::new(),
        }
    }

    // Clear all state for new navigation
    pub fn start_nav(&mut self, final_url: &str) {
        self.base_url = Some(final_url.to_string());
        self.dom = None;
        self.document_mode = None;
        self.head = HeadMetadata::default();
        self.visible_text_cache.clear();
        self.form_controls = FormControlIndex::default();
        self.rendering.reset_for_navigation();
    }

    pub fn update_head_metadata(&mut self) {
        if let Some(dom) = self.dom.as_deref() {
            self.head = extract_head_metadata(dom);
        } else {
            self.head = HeadMetadata::default();
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_dom(
        &mut self,
        dom: Box<Node>,
        facts: DomMutationFacts,
    ) -> DomPublicationRenderInvalidation {
        self.dom = Some(dom);
        if facts.document_replaced() {
            self.rendering
                .reset_retained_identities_for_document_replacement();
        }
        self.mark_dom_changed(facts)
    }

    pub(crate) fn commit_dom_publication(
        &mut self,
        dom: Box<Node>,
        document_mode: html::DocumentMode,
        facts: DomMutationFacts,
    ) -> DomPublicationRenderInvalidation {
        self.document_mode = Some(document_mode);
        self.dom = Some(dom);
        if facts.document_replaced() {
            self.rendering
                .reset_retained_identities_for_document_replacement();
        }
        self.mark_dom_changed(facts)
    }

    pub(crate) fn mark_dom_changed(
        &mut self,
        facts: DomMutationFacts,
    ) -> DomPublicationRenderInvalidation {
        self.rendering.mark_dom_generation_changed();

        if let Some(dom) = self.dom.as_deref() {
            self.rendering.reconcile_retained_identities_from_dom(dom);
        }

        let intrinsic_requests = intrinsic_dom_mutation_requests(&facts);
        let css_facts = facts.to_css_style_change_facts();
        let css_style_request =
            self.rendering
                .apply_style_input_change(&css_facts)
                .map(|authorization| {
                    render_css_style_invalidation_request(
                        CssStyleInvalidationSource::DomPublication,
                        authorization,
                    )
                });
        for request in intrinsic_requests.iter().copied().chain(css_style_request) {
            self.rendering.mark_dirty_for_request(request);
        }
        self.rendering.last_dom_mutation_facts = Some(facts);
        DomPublicationRenderInvalidation {
            intrinsic_requests,
            css_style_request,
        }
    }

    pub(crate) fn derive_render_work_plan(
        &self,
        pending_work: &PendingRenderWork,
    ) -> RenderWorkPlan {
        RenderWorkPlan::derive(RenderWorkPlanInput {
            has_dom: self.dom.is_some(),
            retained_style_artifacts: self.rendering.retained_style_artifact_state(),
            retained_layout_artifacts: self.rendering.retained_layout_artifact_state(),
            retained_paint_artifacts: self.rendering.retained_paint_artifact_state(),
            retained_dirty_state: self.rendering.dirty_state(),
            pending_work,
        })
    }

    pub(crate) fn retained_layout_key_seed(&self) -> RetainedLayoutKeySeed {
        self.rendering.retained_layout_key_seed()
    }

    pub(crate) fn retained_layout_artifact(&self) -> Option<&RetainedLayoutArtifact> {
        self.rendering.retained_layout_artifact()
    }

    pub(crate) fn retained_paint_artifact(&self) -> Option<&PaintArtifact> {
        self.rendering.retained_paint_artifact()
    }

    pub(crate) fn retained_paint_key_seed(&self) -> RetainedPaintArtifactKeySeed {
        self.rendering.retained_paint_key_seed()
    }

    pub(crate) fn record_layout_frame_result(&mut self, result: RetainedLayoutFrameResult) {
        self.rendering.record_layout_frame_result(result);
    }

    pub(crate) fn record_paint_frame_result(&mut self, result: RetainedPaintFrameResult) {
        self.rendering.record_paint_frame_result(result);
    }

    pub(crate) fn style_dirty_for_rendering(&self) -> bool {
        self.rendering.style_dirty()
    }

    pub fn outline(&self, cap: usize) -> Vec<String> {
        if let Some(dom_ref) = self.dom.as_deref() {
            outline_from_dom(dom_ref, cap)
        } else {
            Vec::new()
        }
    }

    pub fn update_visible_text_cache(&mut self) {
        self.visible_text_cache.clear();
        if let Some(dom) = self.dom.as_deref() {
            html::dom_utils::collect_visible_text(dom, &mut self.visible_text_cache);
        }
    }

    pub fn seed_input_values_from_dom(&mut self, store: &mut InputValueStore) {
        let Some(dom) = self.dom.as_deref() else {
            return;
        };
        self.form_controls = seed_input_state_from_dom(store, dom);
    }
}

fn intrinsic_dom_mutation_requests(facts: &DomMutationFacts) -> Vec<RenderInvalidationRequest> {
    let mut requests = Vec::new();
    if facts.document_replaced() {
        requests.push(render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::DocumentReplaced,
        ));
    }
    if facts.tree_topology_or_order_operation() {
        requests.push(render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::DomStructureChanged,
        ));
    }
    if facts.attributes().changed() {
        requests.push(render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::DomAttributesChanged,
        ));
    }
    if facts.text().changed() {
        requests.push(render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::DomTextChanged,
        ));
    }
    if facts.unclassified_patch_count() > 0 {
        requests.push(render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::DomMutationUnclassified,
        ));
    }
    requests
}

impl Default for PageState {
    fn default() -> Self {
        Self::new()
    }
}

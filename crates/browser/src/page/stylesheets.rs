use crate::rendering::{
    CssStyleInvalidationSource, RenderInvalidationRequest, render_css_style_invalidation_request,
};
use core_types::StylesheetSlotId;
use css::StylesheetParse;

use super::PageState;

pub(crate) struct PageStylesheetReconcile {
    pub(crate) fetches: Vec<StylesheetSlotId>,
    pub(crate) retired: Vec<StylesheetSlotId>,
    pub(crate) render_invalidation: Option<RenderInvalidationRequest>,
}

impl PageState {
    pub(crate) fn stylesheet_lifecycle_slots(
        &self,
    ) -> impl ExactSizeIterator<Item = crate::document_style::StylesheetLifecycleView<'_>> + '_
    {
        self.rendering.document_styles.lifecycle_slots()
    }

    // --- CSS ---
    pub(crate) fn prepare_document_stylesheets(
        &self,
        dom: &html::Node,
        base: Option<&addressing::BrowserUrl>,
        replaced: bool,
    ) -> Result<
        crate::document_style::PreparedStylesheetReconcile,
        crate::stylesheet_fetch::StylesheetInventoryError,
    > {
        self.rendering
            .document_styles
            .prepare_reconcile(dom, base, replaced)
    }
    pub(crate) fn commit_document_stylesheets(
        &mut self,
        prepared: crate::document_style::PreparedStylesheetReconcile,
    ) -> PageStylesheetReconcile {
        let result = self.rendering.document_styles.commit_reconcile(prepared);
        let render_invalidation = result.changed.then(|| self.apply_stylesheet_set_change());
        PageStylesheetReconcile {
            fetches: result.fetches,
            retired: result.retired,
            render_invalidation,
        }
    }
    #[cfg(test)]
    pub(crate) fn reconcile_document_stylesheets(&mut self) -> PageStylesheetReconcile {
        let Some(dom) = self.dom.as_deref() else {
            return PageStylesheetReconcile {
                fetches: Vec::new(),
                retired: Vec::new(),
                render_invalidation: None,
            };
        };
        let prepared = self
            .prepare_document_stylesheets(dom, self.active_base_url(), false)
            .expect("valid fixture inventory");
        self.commit_document_stylesheets(prepared)
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub(crate) fn stylesheet_source_observation(
        &self,
        id: StylesheetSlotId,
    ) -> Option<crate::document_style::StylesheetSourceObservation<'_>> {
        self.rendering.document_styles.source_observation(id)
    }
    pub(crate) fn stylesheet_fetch_result(
        &self,
        id: StylesheetSlotId,
    ) -> Option<&crate::stylesheet_fetch::StylesheetFetchResult> {
        self.rendering.document_styles.fetch_result(id)
    }
    pub(crate) fn stylesheet_installation_result(
        &self,
        id: StylesheetSlotId,
    ) -> Option<Result<(), crate::stylesheet_fetch::StylesheetInstallationFailure>> {
        self.rendering.document_styles.installation_result(id)
    }
    pub(crate) fn stylesheet_address(
        &self,
        id: StylesheetSlotId,
    ) -> Option<&addressing::ResourceAddress> {
        self.rendering.document_styles.selected_address(id)
    }
    pub(crate) fn stylesheet_body_available(&self) -> bool {
        self.rendering.document_styles.body_available()
    }
    pub(crate) fn reserve_stylesheet_metadata(
        &mut self,
        id: StylesheetSlotId,
    ) -> Result<(), crate::stylesheet_fetch::StylesheetRetentionFailure> {
        self.rendering.document_styles.reserve_metadata(id)
    }
    pub(crate) fn release_stylesheet_metadata(&mut self, id: StylesheetSlotId) {
        self.rendering.document_styles.release_unused_metadata(id)
    }
    pub(crate) fn provision_stylesheet_body(
        &mut self,
        id: StylesheetSlotId,
    ) -> Result<(), crate::stylesheet_fetch::StylesheetRetentionFailure> {
        self.rendering.document_styles.provision_body(id)
    }
    pub(crate) fn issue_stylesheet_body(&mut self, id: StylesheetSlotId) {
        self.rendering.document_styles.issue_body(id)
    }
    pub(crate) fn rollback_stylesheet_body(&mut self, id: StylesheetSlotId) {
        self.rendering.document_styles.rollback_body(id)
    }
    pub(crate) fn finish_stylesheet_fetch(
        &mut self,
        id: StylesheetSlotId,
        result: crate::stylesheet_fetch::StylesheetFetchResult,
    ) {
        self.rendering.document_styles.finish_fetch(id, result)
    }
    #[cfg(test)]
    pub(crate) fn stylesheet_fetch_mut_for_tests(
        &mut self,
        id: StylesheetSlotId,
    ) -> Option<&mut crate::stylesheet_fetch::StylesheetFetchResult> {
        self.rendering.document_styles.fetch_mut_for_tests(id)
    }
    #[cfg(test)]
    pub(crate) fn corrupt_stylesheet_installation_for_tests(&mut self, id: StylesheetSlotId) {
        self.rendering
            .document_styles
            .corrupt_installation_for_tests(id);
    }
    pub(crate) fn finish_stylesheet_installation(
        &mut self,
        id: StylesheetSlotId,
        result: Result<(), crate::stylesheet_fetch::StylesheetInstallationFailure>,
    ) {
        self.rendering
            .document_styles
            .finish_installation(id, result)
    }

    #[cfg(test)]
    pub(crate) fn take_stylesheet_fetch_for_tests(
        &mut self,
        id: StylesheetSlotId,
    ) -> Option<crate::stylesheet_fetch::StylesheetFetchResult> {
        self.rendering.document_styles.take_fetch_for_tests(id)
    }
    pub(crate) fn stylesheet_slot_id_at(&self, index: usize) -> Option<StylesheetSlotId> {
        self.rendering.document_styles.slot_id_at(index)
    }
    pub(crate) fn stylesheet_slot_count(&self) -> usize {
        self.rendering.document_styles.slot_count()
    }
    pub(crate) fn stylesheet_retention_usage(&self) -> (usize, usize, usize) {
        self.rendering.document_styles.retention_usage()
    }

    pub(crate) fn has_external_stylesheet_slot(&self, id: StylesheetSlotId) -> bool {
        self.rendering.document_styles.has_external_slot(id)
    }

    #[cfg(test)]
    pub(crate) fn active_external_stylesheet_slots(
        &self,
    ) -> std::collections::HashSet<StylesheetSlotId> {
        self.rendering.document_styles.active_external_slots()
    }

    #[cfg(test)]
    pub(crate) fn stylesheet_slot_lookup_count(&self) -> usize {
        self.rendering.document_styles.slot_lookup_count()
    }

    #[cfg(test)]
    pub(crate) fn register_css(&mut self, absolute_url: &str) -> StylesheetSlotId {
        self.rendering
            .document_styles
            .register_external_for_tests(absolute_url)
    }

    pub(crate) fn apply_css_block(
        &mut self,
        slot_id: StylesheetSlotId,
        block: &str,
        response: &net::NetworkResponseInfo,
    ) -> Option<RenderInvalidationRequest> {
        let changed = self
            .rendering
            .document_styles
            .install_external_stylesheet(slot_id, block, response);
        if changed {
            Some(self.apply_stylesheet_set_change())
        } else {
            None
        }
    }

    pub(crate) fn mark_css_done(
        &mut self,
        slot_id: StylesheetSlotId,
    ) -> Option<RenderInvalidationRequest> {
        if self.rendering.document_styles.mark_external_done(slot_id) {
            Some(self.apply_stylesheet_set_change())
        } else {
            None
        }
    }

    pub(crate) fn mark_css_failed(
        &mut self,
        slot_id: StylesheetSlotId,
    ) -> Option<RenderInvalidationRequest> {
        if self.rendering.document_styles.mark_external_failed(slot_id) {
            Some(self.apply_stylesheet_set_change())
        } else {
            None
        }
    }

    pub(crate) fn mark_css_aborted(
        &mut self,
        slot_id: StylesheetSlotId,
    ) -> Option<RenderInvalidationRequest> {
        if self
            .rendering
            .document_styles
            .mark_external_aborted(slot_id)
        {
            Some(self.apply_stylesheet_set_change())
        } else {
            None
        }
    }

    pub fn pending_count(&self) -> usize {
        self.rendering.document_styles.pending_count()
    }

    pub fn css_stylesheets(&self) -> &[StylesheetParse] {
        self.rendering.document_styles.stylesheets()
    }

    fn apply_stylesheet_set_change(&mut self) -> RenderInvalidationRequest {
        let authorization = self.rendering.mark_stylesheets_changed();
        let request = render_css_style_invalidation_request(
            CssStyleInvalidationSource::StylesheetSetChanged,
            authorization,
        );
        self.rendering.mark_dirty_for_request(request);
        request
    }
}

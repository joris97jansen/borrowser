use crate::document_style::StylesheetFetch;
use crate::rendering::{
    CssStyleInvalidationSource, RenderInvalidationRequest, render_css_style_invalidation_request,
};
use core_types::StylesheetSlotId;
use css::StylesheetParse;

use super::PageState;

pub(crate) struct PageStylesheetReconcile {
    pub(crate) fetches: Vec<StylesheetFetch>,
    pub(crate) retired: Vec<StylesheetFetch>,
    pub(crate) render_invalidation: Option<RenderInvalidationRequest>,
}

impl PageState {
    // --- CSS ---
    pub(crate) fn reconcile_document_stylesheets(&mut self) -> PageStylesheetReconcile {
        let Some(dom) = self.dom.as_deref() else {
            return PageStylesheetReconcile {
                fetches: Vec::new(),
                retired: Vec::new(),
                render_invalidation: None,
            };
        };
        let base_url = self
            .addresses
            .as_ref()
            .map(super::DocumentAddressState::active_base_url);
        let result = self
            .rendering
            .document_styles
            .reconcile_from_dom(dom, base_url);
        let render_invalidation = result.changed.then(|| self.apply_stylesheet_set_change());
        PageStylesheetReconcile {
            fetches: result.fetches,
            retired: result.retired,
            render_invalidation,
        }
    }

    pub(crate) fn external_stylesheet_requests(&self) -> Vec<StylesheetFetch> {
        self.rendering
            .document_styles
            .external_stylesheet_requests()
    }

    pub(crate) fn abort_pending_external_stylesheets(
        &mut self,
        completed: &std::collections::HashSet<StylesheetSlotId>,
    ) -> Vec<StylesheetFetch> {
        self.rendering
            .document_styles
            .abort_pending_external(completed)
    }

    pub(crate) fn active_external_stylesheet_slots(
        &self,
    ) -> std::collections::HashSet<StylesheetSlotId> {
        self.rendering.document_styles.active_external_slots()
    }
    pub(crate) fn has_external_stylesheet_slot(&self, id: StylesheetSlotId) -> bool {
        self.rendering.document_styles.has_external_slot(id)
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
    ) -> Option<RenderInvalidationRequest> {
        let changed = self
            .rendering
            .document_styles
            .install_external_stylesheet(slot_id, block);
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

use crate::document_style::StylesheetFetch;
use crate::rendering::{
    CssStyleInvalidationSource, RenderInvalidationRequest, render_css_style_invalidation_request,
};
use core_types::StylesheetSlotId;
use css::StylesheetParse;

use super::PageState;

pub(crate) struct PageStylesheetReconcile {
    pub(crate) fetches: Vec<StylesheetFetch>,
    pub(crate) render_invalidation: Option<RenderInvalidationRequest>,
}

impl PageState {
    // --- CSS ---
    pub(crate) fn reconcile_document_stylesheets(&mut self) -> PageStylesheetReconcile {
        let Some(dom) = self.dom.as_deref() else {
            return PageStylesheetReconcile {
                fetches: Vec::new(),
                render_invalidation: None,
            };
        };
        let result = self
            .rendering
            .document_styles
            .reconcile_from_dom(dom, self.base_url.as_deref());
        let render_invalidation = result.changed.then(|| self.apply_stylesheet_set_change());
        PageStylesheetReconcile {
            fetches: result.fetches,
            render_invalidation,
        }
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
        let css_requested_style_work = self.rendering.mark_stylesheets_changed();
        let request = render_css_style_invalidation_request(
            CssStyleInvalidationSource::StylesheetSetChanged,
            css_requested_style_work,
        );
        self.rendering.mark_dirty_for_request(request);
        request
    }
}

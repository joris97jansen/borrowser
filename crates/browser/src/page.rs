use crate::document_style::{DocumentStyleSet, StylesheetFetch};
use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use core_types::StylesheetSlotId;
use css::StylesheetParse;
use gfx::input::InputValueStore;
use html::{
    Node,
    dom_utils::outline_from_dom,
    head::{HeadMetadata, extract_head_metadata},
};

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Box<Node>>,
    pub head: HeadMetadata,

    pub visible_text_cache: String,
    pub form_controls: FormControlIndex,

    document_styles: DocumentStyleSet,
}

impl PageState {
    pub fn new() -> Self {
        Self {
            base_url: None,
            dom: None,
            head: HeadMetadata::default(),
            visible_text_cache: String::new(),
            form_controls: FormControlIndex::default(),
            document_styles: DocumentStyleSet::default(),
        }
    }

    // Clear all state for new navigation
    pub fn start_nav(&mut self, final_url: &str) {
        self.base_url = Some(final_url.to_string());
        self.dom = None;
        self.head = HeadMetadata::default();
        self.visible_text_cache.clear();
        self.form_controls = FormControlIndex::default();
        self.document_styles.clear();
    }

    pub fn update_head_metadata(&mut self) {
        if let Some(dom) = self.dom.as_deref() {
            self.head = extract_head_metadata(dom);
        } else {
            self.head = HeadMetadata::default();
        }
    }

    // --- CSS ---
    pub(crate) fn reconcile_document_stylesheets(&mut self) -> Vec<StylesheetFetch> {
        let Some(dom) = self.dom.as_deref() else {
            return Vec::new();
        };
        self.document_styles
            .reconcile_from_dom(dom, self.base_url.as_deref())
    }

    #[cfg(test)]
    pub(crate) fn register_css(&mut self, absolute_url: &str) -> StylesheetSlotId {
        self.document_styles
            .register_external_for_tests(absolute_url)
    }

    pub(crate) fn apply_css_block(&mut self, slot_id: StylesheetSlotId, block: &str) -> bool {
        self.document_styles
            .install_external_stylesheet(slot_id, block)
    }

    pub(crate) fn mark_css_done(&mut self, slot_id: StylesheetSlotId) {
        self.document_styles.mark_external_done(slot_id);
    }

    pub(crate) fn mark_css_failed(&mut self, slot_id: StylesheetSlotId) {
        self.document_styles.mark_external_failed(slot_id);
    }

    pub(crate) fn mark_css_aborted(&mut self, slot_id: StylesheetSlotId) {
        self.document_styles.mark_external_aborted(slot_id);
    }

    pub fn pending_count(&self) -> usize {
        self.document_styles.pending_count()
    }

    pub fn css_stylesheets(&self) -> &[StylesheetParse] {
        self.document_styles.stylesheets()
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

impl Default for PageState {
    fn default() -> Self {
        Self::new()
    }
}

use crate::document_style::{DocumentStyleSet, StylesheetFetch};
use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use core_types::StylesheetSlotId;
use css::{
    ComputedDocumentStyle, ComputedStyleResolutionError, StyledNode, StylesheetParse,
    build_style_tree_from_computed_styles, compute_document_styles,
};
use gfx::input::InputValueStore;
use html::{
    DomPatch, Node,
    dom_utils::outline_from_dom,
    head::{HeadMetadata, extract_head_metadata},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestyleTrigger {
    DocumentReplaced,
    TreeMutated,
    AttributesChanged,
    TextMutated,
}

impl RestyleTrigger {
    pub(crate) fn from_patches(patches: &[DomPatch]) -> Option<Self> {
        let mut trigger = None;
        for patch in patches {
            let candidate = match patch {
                DomPatch::Clear | DomPatch::CreateDocument { .. } => Self::DocumentReplaced,
                DomPatch::SetAttributes { .. } => Self::AttributesChanged,
                DomPatch::SetText { .. } | DomPatch::AppendText { .. } => Self::TextMutated,
                DomPatch::CreateElement { .. }
                | DomPatch::CreateText { .. }
                | DomPatch::CreateComment { .. }
                | DomPatch::AppendChild { .. }
                | DomPatch::InsertBefore { .. }
                | DomPatch::RemoveNode { .. } => Self::TreeMutated,
                _ => Self::TreeMutated,
            };
            trigger = Some(match (trigger, candidate) {
                (Some(Self::DocumentReplaced), _) | (_, Self::DocumentReplaced) => {
                    Self::DocumentReplaced
                }
                (Some(Self::TreeMutated), _) | (_, Self::TreeMutated) => Self::TreeMutated,
                (Some(Self::AttributesChanged), _) | (_, Self::AttributesChanged) => {
                    Self::AttributesChanged
                }
                _ => Self::TextMutated,
            });
        }
        trigger
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PageStyleGenerations {
    pub(crate) dom: u64,
    pub(crate) style_inputs: u64,
    pub(crate) stylesheets: u64,
}

#[derive(Clone, Debug)]
struct PageStyleCache {
    style_input_generation: u64,
    stylesheet_generation: u64,
    computed: ComputedDocumentStyle,
}

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Box<Node>>,
    pub head: HeadMetadata,

    pub visible_text_cache: String,
    pub form_controls: FormControlIndex,

    document_styles: DocumentStyleSet,
    generations: PageStyleGenerations,
    style_cache: Option<PageStyleCache>,
    style_dirty: bool,
    layout_dirty: bool,
    last_restyle_trigger: Option<RestyleTrigger>,
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
            generations: PageStyleGenerations::default(),
            style_cache: None,
            style_dirty: true,
            layout_dirty: true,
            last_restyle_trigger: None,
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
        self.generations = PageStyleGenerations::default();
        self.style_cache = None;
        self.style_dirty = true;
        self.layout_dirty = true;
        self.last_restyle_trigger = None;
    }

    pub fn update_head_metadata(&mut self) {
        if let Some(dom) = self.dom.as_deref() {
            self.head = extract_head_metadata(dom);
        } else {
            self.head = HeadMetadata::default();
        }
    }

    pub(crate) fn replace_dom(&mut self, dom: Box<Node>, trigger: RestyleTrigger) {
        self.dom = Some(dom);
        self.mark_dom_changed(trigger);
    }

    pub(crate) fn mark_dom_changed(&mut self, trigger: RestyleTrigger) {
        self.last_restyle_trigger = Some(trigger);
        self.generations.dom = self
            .generations
            .dom
            .checked_add(1)
            .expect("page DOM generation exhausted");

        match trigger {
            RestyleTrigger::TextMutated => {
                // Text node content affects layout and paint, but not selector
                // matching or computed values in the currently supported CSS
                // model. <style> text changes are handled by stylesheet
                // reconciliation, which separately invalidates style.
                self.layout_dirty = true;
            }
            RestyleTrigger::DocumentReplaced
            | RestyleTrigger::TreeMutated
            | RestyleTrigger::AttributesChanged => self.mark_style_inputs_changed(),
        }
    }

    fn mark_style_inputs_changed(&mut self) {
        self.generations.style_inputs = self
            .generations
            .style_inputs
            .checked_add(1)
            .expect("page style-input generation exhausted");
        self.invalidate_style();
    }

    fn mark_stylesheets_changed(&mut self) {
        self.generations.stylesheets = self
            .generations
            .stylesheets
            .checked_add(1)
            .expect("page stylesheet generation exhausted");
        self.invalidate_style();
    }

    fn invalidate_style(&mut self) {
        self.style_dirty = true;
        self.layout_dirty = true;
        self.style_cache = None;
    }

    // --- CSS ---
    pub(crate) fn reconcile_document_stylesheets(&mut self) -> Vec<StylesheetFetch> {
        let Some(dom) = self.dom.as_deref() else {
            return Vec::new();
        };
        let result = self
            .document_styles
            .reconcile_from_dom(dom, self.base_url.as_deref());
        if result.changed {
            self.mark_stylesheets_changed();
        }
        result.fetches
    }

    #[cfg(test)]
    pub(crate) fn register_css(&mut self, absolute_url: &str) -> StylesheetSlotId {
        self.document_styles
            .register_external_for_tests(absolute_url)
    }

    pub(crate) fn apply_css_block(&mut self, slot_id: StylesheetSlotId, block: &str) -> bool {
        let changed = self
            .document_styles
            .install_external_stylesheet(slot_id, block);
        if changed {
            self.mark_stylesheets_changed();
        }
        changed
    }

    pub(crate) fn mark_css_done(&mut self, slot_id: StylesheetSlotId) {
        if self.document_styles.mark_external_done(slot_id) {
            self.mark_stylesheets_changed();
        }
    }

    pub(crate) fn mark_css_failed(&mut self, slot_id: StylesheetSlotId) {
        if self.document_styles.mark_external_failed(slot_id) {
            self.mark_stylesheets_changed();
        }
    }

    pub(crate) fn mark_css_aborted(&mut self, slot_id: StylesheetSlotId) {
        if self.document_styles.mark_external_aborted(slot_id) {
            self.mark_stylesheets_changed();
        }
    }

    pub fn pending_count(&self) -> usize {
        self.document_styles.pending_count()
    }

    pub fn css_stylesheets(&self) -> &[StylesheetParse] {
        self.document_styles.stylesheets()
    }

    pub(crate) fn build_style_tree(
        &mut self,
    ) -> Result<Option<StyledNode<'_>>, ComputedStyleResolutionError> {
        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };

        let needs_recompute = self.style_dirty
            || self.style_cache.as_ref().is_none_or(|cache| {
                cache.style_input_generation != self.generations.style_inputs
                    || cache.stylesheet_generation != self.generations.stylesheets
            });

        if needs_recompute {
            let computed = compute_document_styles(dom, self.css_stylesheets())?;
            self.style_cache = Some(PageStyleCache {
                style_input_generation: self.generations.style_inputs,
                stylesheet_generation: self.generations.stylesheets,
                computed,
            });
            self.style_dirty = false;
        }

        let cache = self
            .style_cache
            .as_ref()
            .expect("style cache must exist after successful style computation");
        build_style_tree_from_computed_styles(dom, &cache.computed).map(Some)
    }

    #[cfg(test)]
    pub(crate) fn style_generations(&self) -> PageStyleGenerations {
        self.generations
    }

    #[cfg(test)]
    pub(crate) fn style_dirty(&self) -> bool {
        self.style_dirty
    }

    #[cfg(test)]
    pub(crate) fn layout_dirty(&self) -> bool {
        self.layout_dirty
    }

    #[cfg(test)]
    pub(crate) fn clear_layout_dirty_for_tests(&mut self) {
        self.layout_dirty = false;
    }

    #[cfg(test)]
    pub(crate) fn last_restyle_trigger(&self) -> Option<RestyleTrigger> {
        self.last_restyle_trigger
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

mod debug;
mod restyle;
mod retained_render_state;
mod style_cache;
mod style_phase;
mod stylesheets;

pub(crate) use restyle::{RestyleHint, RestyleTrigger};
#[cfg(test)]
pub(crate) use style_cache::{PageStyleGenerations, StyleRecalcKind};
#[allow(unused_imports)]
pub(crate) use stylesheets::PageStylesheetReconcile;

use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use crate::rendering::{RenderInvalidationRequest, render_invalidation_request};
use gfx::input::InputValueStore;
use html::{
    Node,
    dom_utils::outline_from_dom,
    head::{HeadMetadata, extract_head_metadata},
};

use restyle::StyleInvalidationScope;
use retained_render_state::RetainedRenderState;

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Box<Node>>,
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

    pub(crate) fn replace_dom(
        &mut self,
        dom: Box<Node>,
        hint: RestyleHint,
    ) -> RenderInvalidationRequest {
        self.dom = Some(dom);
        self.mark_dom_changed(hint)
    }

    pub(crate) fn mark_dom_changed(&mut self, hint: RestyleHint) -> RenderInvalidationRequest {
        let retained = &mut self.rendering;
        let trigger = hint.trigger;
        retained.last_restyle_trigger = Some(trigger);
        retained.generations.dom = retained
            .generations
            .dom
            .checked_add(1)
            .expect("page DOM generation exhausted");

        match trigger {
            RestyleTrigger::TextMutated => {
                // Text node content affects layout and paint, but not selector
                // matching or computed values in the currently supported CSS
                // model. <style> text changes are handled by stylesheet
                // reconciliation, which separately invalidates style. Future
                // text-sensitive selector or generated-content support must
                // widen this contract if text content becomes style-relevant.
                retained.layout_dirty = true;
            }
            RestyleTrigger::DocumentReplaced | RestyleTrigger::TreeMutated => self
                .rendering
                .mark_style_inputs_changed(StyleInvalidationScope::Full),
            RestyleTrigger::AttributesChanged => {
                let node_ids = hint.attribute_dirty_nodes;
                let scope = if node_ids.is_empty() {
                    StyleInvalidationScope::Full
                } else {
                    StyleInvalidationScope::AttributeSuffix { node_ids }
                };
                self.rendering.mark_style_inputs_changed(scope);
            }
        }

        render_invalidation_request(trigger.render_invalidation_entry_point())
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

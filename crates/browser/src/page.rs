use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use css::{ParseOptions, StylesheetParse, attach_styles, parse_stylesheet_with_options};
use gfx::input::InputValueStore;
use html::{
    Node,
    dom_utils::{collect_style_texts, outline_from_dom},
    head::{HeadMetadata, extract_head_metadata},
};
use std::collections::HashSet;

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Box<Node>>,
    pub head: HeadMetadata,

    pub visible_text_cache: String,
    pub form_controls: FormControlIndex,

    css_pending: HashSet<String>,
    css_stylesheets: Vec<StylesheetParse>,
}

impl PageState {
    pub fn new() -> Self {
        Self {
            base_url: None,
            dom: None,
            head: HeadMetadata::default(),
            visible_text_cache: String::new(),
            form_controls: FormControlIndex::default(),
            css_pending: HashSet::new(),
            css_stylesheets: Vec::new(),
        }
    }

    // Clear all state for new navigation
    pub fn start_nav(&mut self, final_url: &str) {
        self.base_url = Some(final_url.to_string());
        self.dom = None;
        self.head = HeadMetadata::default();
        self.visible_text_cache.clear();
        self.form_controls = FormControlIndex::default();
        self.css_pending.clear();
        self.css_stylesheets.clear();
    }

    pub fn update_head_metadata(&mut self) {
        if let Some(dom) = self.dom.as_deref() {
            self.head = extract_head_metadata(dom);
        } else {
            self.head = HeadMetadata::default();
        }
    }

    // --- CSS ---
    pub fn register_css(&mut self, absolute_url: &str) -> bool {
        self.css_pending.insert(absolute_url.to_string())
    }

    pub fn apply_css_block(&mut self, block: &str) {
        let parsed = parse_stylesheet_with_options(block, &ParseOptions::stylesheet());
        self.css_stylesheets.push(parsed);
        if let Some(dom_mut) = self.dom.as_deref_mut() {
            attach_styles(dom_mut, &self.css_stylesheets);
        }
    }

    pub fn mark_css_done(&mut self, url: &str) {
        self.css_pending.remove(url);
    }

    pub fn pending_count(&self) -> usize {
        self.css_pending.len()
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

    pub fn apply_inline_style_blocks(&mut self) {
        if let Some(dom_mut) = self.dom.as_deref_mut() {
            let mut css_text = String::new();
            collect_style_texts(dom_mut, &mut css_text);

            if !css_text.trim().is_empty() {
                let parsed = parse_stylesheet_with_options(&css_text, &ParseOptions::stylesheet());
                self.css_stylesheets.push(parsed);
            }

            // Apply all known stylesheets + inline style="" attrs
            attach_styles(dom_mut, &self.css_stylesheets);
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

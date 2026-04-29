use crate::document_style::{DocumentStyleSet, StylesheetFetch};
use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use core_types::StylesheetSlotId;
use css::{
    ComputedDocumentStyle, ComputedStyleResolutionError, ResolvedDocumentStyle,
    StyleResolutionLimits, StyledNode, StylesheetParse, build_style_tree_from_computed_styles,
    compute_document_styles_from_resolved_styles,
    compute_document_styles_incremental_suffix_with_limits, resolve_document_styles,
};
use gfx::input::InputValueStore;
use html::{
    DomPatch, Node, PatchKey,
    dom_utils::outline_from_dom,
    head::{HeadMetadata, extract_head_metadata},
    internal::Id,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestyleHint {
    trigger: RestyleTrigger,
    attribute_dirty_keys: Vec<PatchKey>,
}

impl RestyleHint {
    pub(crate) fn document_replaced() -> Self {
        Self {
            trigger: RestyleTrigger::DocumentReplaced,
            attribute_dirty_keys: Vec::new(),
        }
    }

    pub(crate) fn from_patches(patches: &[DomPatch]) -> Option<Self> {
        let trigger = RestyleTrigger::from_patches(patches)?;
        let attribute_dirty_keys = patches
            .iter()
            .filter_map(|patch| match patch {
                DomPatch::SetAttributes { key, .. } => Some(*key),
                _ => None,
            })
            .collect();

        Some(Self {
            trigger,
            attribute_dirty_keys,
        })
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
    resolved: ResolvedDocumentStyle,
    computed: ComputedDocumentStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StyleInvalidationScope {
    /// Conservative full restyle. Used for document replacement, structural
    /// mutations, stylesheet changes, and any partial-reuse proof failure.
    Full,
    /// Minimal U4 partial strategy: attribute changes preserve selector element
    /// order, so reuse the computed prefix before the earliest changed element
    /// and recompute that element plus the document-order suffix. The suffix is
    /// deliberately conservative because sibling selectors can affect following
    /// siblings and inheritance can affect descendants.
    ///
    /// This proof assumes the supported selector model has no selector that lets
    /// later or descendant elements affect an earlier ancestor or sibling, such
    /// as `:has()`. Adding that kind of selector must either widen this scope to
    /// `Full` or add selector-aware invalidation dependencies.
    AttributeSuffix { node_ids: Vec<Id> },
}

impl StyleInvalidationScope {
    fn merge(self, next: Self) -> Self {
        match (self, next) {
            (Self::Full, _) | (_, Self::Full) => Self::Full,
            (
                Self::AttributeSuffix { mut node_ids },
                Self::AttributeSuffix {
                    node_ids: next_node_ids,
                },
            ) => {
                node_ids.extend(next_node_ids);
                node_ids.sort_by_key(|id| id.0);
                node_ids.dedup();
                Self::AttributeSuffix { node_ids }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StyleRecalcKind {
    ReusedCache,
    Full {
        elements: usize,
    },
    IncrementalSuffix {
        reused_prefix_len: usize,
        recomputed_len: usize,
    },
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
    pending_style_invalidation: Option<StyleInvalidationScope>,
    last_style_recalc: Option<StyleRecalcKind>,
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
            pending_style_invalidation: Some(StyleInvalidationScope::Full),
            last_style_recalc: None,
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
        self.pending_style_invalidation = Some(StyleInvalidationScope::Full);
        self.last_style_recalc = None;
    }

    pub fn update_head_metadata(&mut self) {
        if let Some(dom) = self.dom.as_deref() {
            self.head = extract_head_metadata(dom);
        } else {
            self.head = HeadMetadata::default();
        }
    }

    pub(crate) fn replace_dom(&mut self, dom: Box<Node>, hint: RestyleHint) {
        self.dom = Some(dom);
        self.mark_dom_changed(hint);
    }

    pub(crate) fn mark_dom_changed(&mut self, hint: RestyleHint) {
        let trigger = hint.trigger;
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
            RestyleTrigger::DocumentReplaced | RestyleTrigger::TreeMutated => {
                self.mark_style_inputs_changed(StyleInvalidationScope::Full)
            }
            RestyleTrigger::AttributesChanged => {
                let node_ids = hint
                    .attribute_dirty_keys
                    .into_iter()
                    // DomStore materializes patch-created nodes with
                    // Node::id() == Id(PatchKey.0). Patch-derived restyle
                    // hints rely on that identity contract; if it changes,
                    // dirty keys must be resolved through DomStore before
                    // reaching PageState.
                    .map(|key| Id(key.0))
                    .collect::<Vec<_>>();
                let scope = if node_ids.is_empty() {
                    StyleInvalidationScope::Full
                } else {
                    StyleInvalidationScope::AttributeSuffix { node_ids }
                };
                self.mark_style_inputs_changed(scope);
            }
        }
    }

    fn mark_style_inputs_changed(&mut self, scope: StyleInvalidationScope) {
        self.generations.style_inputs = self
            .generations
            .style_inputs
            .checked_add(1)
            .expect("page style-input generation exhausted");
        self.invalidate_style(scope);
    }

    fn mark_stylesheets_changed(&mut self) {
        self.generations.stylesheets = self
            .generations
            .stylesheets
            .checked_add(1)
            .expect("page stylesheet generation exhausted");
        self.invalidate_style(StyleInvalidationScope::Full);
    }

    fn invalidate_style(&mut self, scope: StyleInvalidationScope) {
        self.style_dirty = true;
        self.layout_dirty = true;

        let merged = match self.pending_style_invalidation.take() {
            Some(existing) => existing.merge(scope),
            None => scope,
        };

        if matches!(merged, StyleInvalidationScope::Full) {
            self.style_cache = None;
        }
        self.pending_style_invalidation = Some(merged);
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

        let Self {
            document_styles,
            generations,
            style_cache,
            style_dirty,
            pending_style_invalidation,
            last_style_recalc,
            ..
        } = self;
        let needs_recompute = *style_dirty
            || style_cache.as_ref().is_none_or(|cache| {
                cache.style_input_generation != generations.style_inputs
                    || cache.stylesheet_generation != generations.stylesheets
            });

        if needs_recompute {
            Self::recompute_styles(
                dom,
                document_styles.stylesheets(),
                *generations,
                style_cache,
                pending_style_invalidation,
                style_dirty,
                last_style_recalc,
            )?;
        } else {
            *last_style_recalc = Some(StyleRecalcKind::ReusedCache);
        }

        let cache = style_cache
            .as_ref()
            .expect("style cache must exist after successful style computation");
        build_style_tree_from_computed_styles(dom, &cache.computed).map(Some)
    }

    fn recompute_styles(
        dom: &Node,
        sheets: &[StylesheetParse],
        generations: PageStyleGenerations,
        style_cache: &mut Option<PageStyleCache>,
        pending_style_invalidation: &mut Option<StyleInvalidationScope>,
        style_dirty: &mut bool,
        last_style_recalc: &mut Option<StyleRecalcKind>,
    ) -> Result<(), ComputedStyleResolutionError> {
        let pending = pending_style_invalidation
            .take()
            .unwrap_or(StyleInvalidationScope::Full);

        if let StyleInvalidationScope::AttributeSuffix { node_ids } = &pending
            && let Some(cache) = style_cache.as_ref()
            && cache.stylesheet_generation == generations.stylesheets
        {
            let limits = StyleResolutionLimits::default();
            if let Some(incremental) = compute_document_styles_incremental_suffix_with_limits(
                dom,
                sheets,
                &cache.resolved,
                &cache.computed,
                node_ids,
                &limits,
            )? {
                *last_style_recalc = Some(StyleRecalcKind::IncrementalSuffix {
                    reused_prefix_len: incremental.reused_prefix_len,
                    recomputed_len: incremental.recomputed_len,
                });
                *style_cache = Some(PageStyleCache {
                    style_input_generation: generations.style_inputs,
                    stylesheet_generation: generations.stylesheets,
                    resolved: incremental.resolved,
                    computed: incremental.computed,
                });
                *style_dirty = false;
                return Ok(());
            }
        }

        let resolved = resolve_document_styles(dom, sheets)
            .map_err(ComputedStyleResolutionError::StyleResolution)?;
        let computed = compute_document_styles_from_resolved_styles(dom, &resolved)?;
        let elements = computed.entries().len();
        *last_style_recalc = Some(StyleRecalcKind::Full { elements });
        *style_cache = Some(PageStyleCache {
            style_input_generation: generations.style_inputs,
            stylesheet_generation: generations.stylesheets,
            resolved,
            computed,
        });
        *style_dirty = false;
        Ok(())
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

    #[cfg(test)]
    pub(crate) fn last_style_recalc(&self) -> Option<StyleRecalcKind> {
        self.last_style_recalc
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

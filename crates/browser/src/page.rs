use crate::document_style::{DocumentStyleSet, StylesheetFetch};
use crate::form_controls::{FormControlIndex, seed_input_state_from_dom};
use crate::rendering::{
    RenderArtifactState, RenderInvalidationEntryPoint, RenderInvalidationRequest,
    RenderPipelineDebugSnapshot, StyleInvalidationState, render_invalidation_request,
};
use core_types::StylesheetSlotId;
use css::{
    ComputedDocumentStyle, ComputedStyleResolutionError, ComputedStyleReuseStats,
    ResolvedDocumentStyle, StylePhaseOutput, StyleResolutionLimits, StylesheetParse,
    build_style_tree_from_computed_styles,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_incremental_suffix_with_limits, resolve_document_styles,
};
use gfx::input::InputValueStore;
use html::{
    DomPatch, Node,
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

    fn render_invalidation_entry_point(self) -> RenderInvalidationEntryPoint {
        match self {
            Self::DocumentReplaced => RenderInvalidationEntryPoint::DocumentReplaced,
            Self::TreeMutated => RenderInvalidationEntryPoint::DomStructureChanged,
            Self::AttributesChanged => RenderInvalidationEntryPoint::DomAttributesChanged,
            Self::TextMutated => RenderInvalidationEntryPoint::DomTextChanged,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestyleHint {
    trigger: RestyleTrigger,
    attribute_dirty_nodes: Vec<Id>,
}

impl RestyleHint {
    pub(crate) fn document_replaced() -> Self {
        Self {
            trigger: RestyleTrigger::DocumentReplaced,
            attribute_dirty_nodes: Vec::new(),
        }
    }

    pub(crate) fn from_dom_patch_batch(
        patches: &[DomPatch],
        attribute_dirty_nodes: Vec<Id>,
    ) -> Option<Self> {
        let trigger = RestyleTrigger::from_patches(patches)?;

        Some(Self {
            trigger,
            attribute_dirty_nodes,
        })
    }

    #[cfg(test)]
    pub(crate) fn attributes_changed(attribute_dirty_nodes: Vec<Id>) -> Self {
        Self {
            trigger: RestyleTrigger::AttributesChanged,
            attribute_dirty_nodes,
        }
    }

    #[cfg(test)]
    pub(crate) fn text_mutated() -> Self {
        Self {
            trigger: RestyleTrigger::TextMutated,
            attribute_dirty_nodes: Vec::new(),
        }
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

/// Retained rendering state owned by `PageState`.
///
/// This groups the page-local rendering artifacts and invalidation metadata
/// that survive across updates. Borrow-backed style trees, layout trees, and
/// paint output remain outside this struct by contract.
#[derive(Clone, Debug)]
struct RetainedRenderState {
    document_styles: DocumentStyleSet,
    generations: PageStyleGenerations,
    style_cache: Option<PageStyleCache>,
    style_dirty: bool,
    layout_dirty: bool,
    last_restyle_trigger: Option<RestyleTrigger>,
    pending_style_invalidation: Option<StyleInvalidationScope>,
    last_style_recalc: Option<StyleRecalcKind>,
    last_style_reuse: Option<ComputedStyleReuseStats>,
}

impl RetainedRenderState {
    fn new() -> Self {
        Self {
            document_styles: DocumentStyleSet::default(),
            generations: PageStyleGenerations::default(),
            style_cache: None,
            style_dirty: true,
            layout_dirty: true,
            last_restyle_trigger: None,
            pending_style_invalidation: Some(StyleInvalidationScope::Full),
            last_style_recalc: None,
            last_style_reuse: None,
        }
    }

    fn reset_for_navigation(&mut self) {
        self.document_styles.clear();
        self.generations = PageStyleGenerations::default();
        self.style_cache = None;
        self.style_dirty = true;
        self.layout_dirty = true;
        self.last_restyle_trigger = None;
        self.pending_style_invalidation = Some(StyleInvalidationScope::Full);
        self.last_style_recalc = None;
        self.last_style_reuse = None;
    }

    fn debug_snapshot(&self, has_dom: bool) -> RenderPipelineDebugSnapshot {
        let style_cache_state = match (&self.style_cache, self.style_dirty) {
            (None, _) => RenderArtifactState::Absent,
            (Some(_), true) => RenderArtifactState::RetainedStale,
            (Some(_), false) => RenderArtifactState::RetainedFresh,
        };

        let (styled_tree, layout_tree, paint_output) = if has_dom {
            (
                RenderArtifactState::BorrowBackedRebuiltOnDemand,
                RenderArtifactState::FrameLocalRebuiltPerFrame,
                RenderArtifactState::ImmediateFrameOutput,
            )
        } else {
            (
                RenderArtifactState::Absent,
                RenderArtifactState::Absent,
                RenderArtifactState::Absent,
            )
        };

        let style_invalidation = match self.pending_style_invalidation {
            Some(StyleInvalidationScope::Full) => StyleInvalidationState::Full,
            Some(StyleInvalidationScope::AttributeSuffix { .. }) => {
                StyleInvalidationState::AttributeSuffix
            }
            None => StyleInvalidationState::None,
        };

        RenderPipelineDebugSnapshot {
            has_dom,
            resolved_styles: style_cache_state,
            computed_styles: style_cache_state,
            styled_tree,
            layout_tree,
            paint_output,
            style_dirty: self.style_dirty,
            layout_dirty: self.layout_dirty,
            style_invalidation,
        }
    }
}

impl Default for RetainedRenderState {
    fn default() -> Self {
        Self::new()
    }
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

struct StyleRecomputeState<'a> {
    style_cache: &'a mut Option<PageStyleCache>,
    pending_style_invalidation: &'a mut Option<StyleInvalidationScope>,
    style_dirty: &'a mut bool,
    last_style_recalc: &'a mut Option<StyleRecalcKind>,
    last_style_reuse: &'a mut Option<ComputedStyleReuseStats>,
}

pub(crate) struct PageStylesheetReconcile {
    pub(crate) fetches: Vec<StylesheetFetch>,
    pub(crate) render_invalidation: Option<RenderInvalidationRequest>,
}

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
            RestyleTrigger::DocumentReplaced | RestyleTrigger::TreeMutated => {
                self.mark_style_inputs_changed(StyleInvalidationScope::Full)
            }
            RestyleTrigger::AttributesChanged => {
                let node_ids = hint.attribute_dirty_nodes;
                let scope = if node_ids.is_empty() {
                    StyleInvalidationScope::Full
                } else {
                    StyleInvalidationScope::AttributeSuffix { node_ids }
                };
                self.mark_style_inputs_changed(scope);
            }
        }

        render_invalidation_request(trigger.render_invalidation_entry_point())
    }

    fn mark_style_inputs_changed(&mut self, scope: StyleInvalidationScope) {
        self.rendering.generations.style_inputs = self
            .rendering
            .generations
            .style_inputs
            .checked_add(1)
            .expect("page style-input generation exhausted");
        self.invalidate_style(scope);
    }

    fn mark_stylesheets_changed(&mut self) {
        self.rendering.generations.stylesheets = self
            .rendering
            .generations
            .stylesheets
            .checked_add(1)
            .expect("page stylesheet generation exhausted");
        self.invalidate_style(StyleInvalidationScope::Full);
    }

    fn invalidate_style(&mut self, scope: StyleInvalidationScope) {
        let retained = &mut self.rendering;
        retained.style_dirty = true;
        retained.layout_dirty = true;

        let merged = match retained.pending_style_invalidation.take() {
            Some(existing) => existing.merge(scope),
            None => scope,
        };

        if matches!(merged, StyleInvalidationScope::Full) {
            retained.style_cache = None;
        }
        retained.pending_style_invalidation = Some(merged);
    }

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
        let render_invalidation = result.changed.then(|| {
            render_invalidation_request(RenderInvalidationEntryPoint::StylesheetSetChanged)
        });
        if result.changed {
            self.mark_stylesheets_changed();
        }
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
            self.mark_stylesheets_changed();
            Some(render_invalidation_request(
                RenderInvalidationEntryPoint::StylesheetSetChanged,
            ))
        } else {
            None
        }
    }

    pub(crate) fn mark_css_done(
        &mut self,
        slot_id: StylesheetSlotId,
    ) -> Option<RenderInvalidationRequest> {
        if self.rendering.document_styles.mark_external_done(slot_id) {
            self.mark_stylesheets_changed();
            Some(render_invalidation_request(
                RenderInvalidationEntryPoint::StylesheetSetChanged,
            ))
        } else {
            None
        }
    }

    pub(crate) fn mark_css_failed(
        &mut self,
        slot_id: StylesheetSlotId,
    ) -> Option<RenderInvalidationRequest> {
        if self.rendering.document_styles.mark_external_failed(slot_id) {
            self.mark_stylesheets_changed();
            Some(render_invalidation_request(
                RenderInvalidationEntryPoint::StylesheetSetChanged,
            ))
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
            self.mark_stylesheets_changed();
            Some(render_invalidation_request(
                RenderInvalidationEntryPoint::StylesheetSetChanged,
            ))
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

    /// Runtime style-phase boundary for page rendering.
    ///
    /// `PageState` owns retained resolved/computed style artifacts and the
    /// invalidation logic that decides whether they can be reused. This method
    /// either reuses or recomputes those retained artifacts, then rebuilds the
    /// borrow-backed `StyledNode` view wrapped in an explicit style-phase
    /// output contract for downstream layout and paint.
    pub(crate) fn build_style_phase_output(
        &mut self,
    ) -> Result<Option<StylePhaseOutput<'_>>, ComputedStyleResolutionError> {
        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };

        let retained = &mut self.rendering;
        let needs_recompute = retained.style_dirty
            || retained.style_cache.as_ref().is_none_or(|cache| {
                cache.style_input_generation != retained.generations.style_inputs
                    || cache.stylesheet_generation != retained.generations.stylesheets
            });

        if needs_recompute {
            Self::recompute_styles(
                dom,
                retained.document_styles.stylesheets(),
                retained.generations,
                StyleRecomputeState {
                    style_cache: &mut retained.style_cache,
                    pending_style_invalidation: &mut retained.pending_style_invalidation,
                    style_dirty: &mut retained.style_dirty,
                    last_style_recalc: &mut retained.last_style_recalc,
                    last_style_reuse: &mut retained.last_style_reuse,
                },
            )?;
        } else {
            retained.last_style_recalc = Some(StyleRecalcKind::ReusedCache);
            retained.last_style_reuse = Some(ComputedStyleReuseStats::default());
        }

        let cache = retained
            .style_cache
            .as_ref()
            .expect("style cache must exist after successful style computation");
        build_style_tree_from_computed_styles(dom, &cache.computed)
            .map(StylePhaseOutput::new)
            .map(Some)
    }

    /// Reports the retained/rebuilt policy for rendering artifacts owned or
    /// coordinated by the current page state.
    ///
    /// For frame-local layout and immediate paint output, this snapshot records
    /// the rebuild policy used when the viewport renders a frame. It does not
    /// imply that `PageState` currently retains a live layout tree or paint
    /// artifact between frames.
    pub fn render_pipeline_debug_snapshot(&self) -> RenderPipelineDebugSnapshot {
        self.rendering.debug_snapshot(self.dom.is_some())
    }

    fn recompute_styles(
        dom: &Node,
        sheets: &[StylesheetParse],
        generations: PageStyleGenerations,
        state: StyleRecomputeState<'_>,
    ) -> Result<(), ComputedStyleResolutionError> {
        let pending = state
            .pending_style_invalidation
            .take()
            .unwrap_or(StyleInvalidationScope::Full);

        if let StyleInvalidationScope::AttributeSuffix { node_ids } = &pending
            && let Some(cache) = state.style_cache.as_ref()
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
                *state.last_style_recalc = Some(StyleRecalcKind::IncrementalSuffix {
                    reused_prefix_len: incremental.reused_prefix_len,
                    recomputed_len: incremental.recomputed_len,
                });
                *state.last_style_reuse = Some(incremental.reuse_stats);
                *state.style_cache = Some(PageStyleCache {
                    style_input_generation: generations.style_inputs,
                    stylesheet_generation: generations.stylesheets,
                    resolved: incremental.resolved,
                    computed: incremental.computed,
                });
                *state.style_dirty = false;
                return Ok(());
            }
        }

        let resolved = resolve_document_styles(dom, sheets)
            .map_err(ComputedStyleResolutionError::StyleResolution)?;
        let computed =
            compute_document_styles_from_resolved_styles_with_reuse_stats(dom, &resolved)?;
        let elements = computed.computed.entries().len();
        *state.last_style_recalc = Some(StyleRecalcKind::Full { elements });
        *state.last_style_reuse = Some(computed.reuse_stats);
        *state.style_cache = Some(PageStyleCache {
            style_input_generation: generations.style_inputs,
            stylesheet_generation: generations.stylesheets,
            resolved,
            computed: computed.computed,
        });
        *state.style_dirty = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn style_generations(&self) -> PageStyleGenerations {
        self.rendering.generations
    }

    #[cfg(test)]
    pub(crate) fn style_dirty(&self) -> bool {
        self.rendering.style_dirty
    }

    #[cfg(test)]
    pub(crate) fn layout_dirty(&self) -> bool {
        self.rendering.layout_dirty
    }

    #[cfg(test)]
    pub(crate) fn clear_layout_dirty_for_tests(&mut self) {
        self.rendering.layout_dirty = false;
    }

    #[cfg(test)]
    pub(crate) fn mark_dom_changed_for_tests(&mut self, hint: RestyleHint) {
        let _ = self.mark_dom_changed(hint);
    }

    #[cfg(test)]
    pub(crate) fn last_restyle_trigger(&self) -> Option<RestyleTrigger> {
        self.rendering.last_restyle_trigger
    }

    #[cfg(test)]
    pub(crate) fn last_style_recalc(&self) -> Option<StyleRecalcKind> {
        self.rendering.last_style_recalc
    }

    #[cfg(test)]
    pub(crate) fn last_style_reuse(&self) -> Option<ComputedStyleReuseStats> {
        self.rendering.last_style_reuse
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

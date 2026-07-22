use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable, DocumentParseContext, EngineInvariantError, Token};
use crate::html5::tokenizer::{Html5Tokenizer, TextModeSpec, TextResolver, TokenizerControl};
use crate::html5::tree_builder::document::{DocumentState, PendingDoctype};
use crate::html5::tree_builder::formatting::ActiveFormattingList;
use crate::html5::tree_builder::invariants::DomInvariantState;
use crate::html5::tree_builder::known_tags::KnownTagIds;
use crate::html5::tree_builder::live_tree::LiveTree;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::patch_sink::PatchSink;
use crate::html5::tree_builder::stack::{OpenElementsStack, ScopeTagSet};
use crate::html5::tree_builder::table::PendingTableTextState;
#[cfg(any(test, feature = "html5-fuzzing", feature = "internal-api"))]
use crate::html5::tree_builder::template_state::TemplateInsertionMode;
use crate::html5::tree_builder::template_state::TemplateModeStack;
use std::num::NonZeroU32;

/// Centralized tree-builder hardening/resource bounds.
///
/// Recovery policy:
/// - a retained non-void start tag that would exceed
///   `max_open_elements_depth` is ignored;
/// - a void element may perform one bounded internal push/pop while retained
///   depth equals this limit; the observed high-water metric records that real
///   transient depth;
/// - creating an element/text/comment node after `max_nodes_created` is reached
///   is ignored;
/// - inserting another child under a full parent is ignored.
///
/// These limits intentionally preserve boundedness and internal consistency
/// first under adversarial input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeBuilderLimits {
    pub max_open_elements_depth: usize,
    /// Maximum number of non-document DOM nodes created in a single document.
    /// The synthetic document root is intentionally exempt.
    pub max_nodes_created: usize,
    pub max_children_per_node: usize,
}

impl Default for TreeBuilderLimits {
    fn default() -> Self {
        Self {
            max_open_elements_depth: 1024,
            max_nodes_created: 65_536,
            max_children_per_node: 16_384,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TreeBuilderConfig {
    /// Whether to coalesce adjacent text insertions under the same parent.
    ///
    /// Coalescing policy (Core-v0):
    /// - first insertion in a run emits `CreateText` + `AppendChild`,
    /// - adjacent insertions emit `AppendText` on the same key,
    /// - any structural mutation (insert/pop/comment/recovery-literal boundary) breaks the run.
    ///
    /// This policy keeps output deterministic for both buffered (`process`) and
    /// sink-based (`push_token`) paths, including across chunk boundaries.
    ///
    /// Streaming flush behavior:
    /// - Coalescing may span sink flush boundaries (`push_token` + sink push).
    /// - A later batch may emit `AppendText` for a text node created in an earlier batch.
    pub coalesce_text: bool,
    /// Explicit tree-builder hardening/resource bounds.
    pub limits: TreeBuilderLimits,
}

/// Tree builder step result.
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeBuilderStepResult {
    pub flow: TreeBuilderControlFlow,
    pub tokenizer_control: Option<TokenizerControl>,
}

impl TreeBuilderStepResult {
    pub(in crate::html5::tree_builder) fn continue_with(
        tokenizer_control: Option<TokenizerControl>,
    ) -> Self {
        Self {
            flow: TreeBuilderControlFlow::Continue,
            tokenizer_control,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeBuilderControlFlow {
    Continue,
    Suspend(SuspendReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuspendReason {
    Script,
    Other,
}

/// Tree building should not fail on malformed HTML; internal/resource failures
/// are the only error surface for now.
///
/// Policy note:
/// - Some invariants (like atom-table binding) are hard assertions and panic.
/// - Resource/internal failures continue to flow through this `Result` surface
///   (for example, key allocator exhaustion) to keep integration call sites
///   explicit while Core v0 evolves.
pub type TreeBuilderInternalError = EngineInvariantError;
pub type TreeBuilderError = TreeBuilderInternalError;

/// Stable parser-owned identity of the current form element.
///
/// This deliberately stores a parser `PatchKey`, never a borrowed DOM node or
/// browser/runtime identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct FormElementPointer(PatchKey);

impl FormElementPointer {
    pub(in crate::html5::tree_builder) fn new(key: PatchKey) -> Self {
        Self(key)
    }

    pub(in crate::html5::tree_builder) fn key(self) -> PatchKey {
        self.0
    }
}

/// Pending parser-only suppression of the first textarea line feed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct PendingTextareaInitialLf {
    textarea: PatchKey,
}

impl PendingTextareaInitialLf {
    pub(in crate::html5::tree_builder) fn new(textarea: PatchKey) -> Self {
        Self { textarea }
    }

    pub(in crate::html5::tree_builder) fn textarea(self) -> PatchKey {
        self.textarea
    }
}

/// HTML5 tree builder.
///
/// Invariants:
/// - Public methods are panic-free on malformed HTML content. Malformed input is
///   treated as recoverable and does not surface as an error.
/// - Public methods may panic on engine invariant violations/misuse (for
///   example, passing a foreign `AtomTable`).
/// - `TreeBuilderError` is reserved for engine invariant violations only
///   (e.g., invalid text spans or internal key allocator exhaustion).
/// - Emitted patch order is deterministic and source-ordered.
/// - `PatchKey` values are monotonically increasing, non-zero, and never reused
///   within a builder instance.
/// - Core-v0 currently emits `Arc<str>` names in patches from canonical atoms;
///   this will eventually move toward atom-first patch payloads.
/// - The builder is bound to the `AtomTable` from `new()`; passing any other
///   table to `process`/`push_token` is an engine invariant violation and panics.
pub struct Html5TreeBuilder {
    pub(in crate::html5::tree_builder) config: TreeBuilderConfig,
    pub(in crate::html5::tree_builder) atom_table_id: u64,
    pub(in crate::html5::tree_builder) insertion_mode: InsertionMode,
    pub(in crate::html5::tree_builder) original_insertion_mode: Option<InsertionMode>,
    pub(in crate::html5::tree_builder) known_tags: KnownTagIds,
    pub(in crate::html5::tree_builder) scope_tags: ScopeTagSet,
    pub(in crate::html5::tree_builder) live_tree: LiveTree,
    pub(in crate::html5::tree_builder) open_elements: OpenElementsStack,
    pub(in crate::html5::tree_builder) active_formatting: ActiveFormattingList,
    pub(in crate::html5::tree_builder) template_modes: TemplateModeStack,
    pub(in crate::html5::tree_builder) document_key: Option<PatchKey>,
    pub(in crate::html5::tree_builder) head_element_pointer: Option<PatchKey>,
    pub(in crate::html5::tree_builder) template_state_epoch: u64,
    pub(in crate::html5::tree_builder) accepted_template_count: u64,
    pub(in crate::html5::tree_builder) next_patch_key: NonZeroU32,
    pub(in crate::html5::tree_builder) pending_doctype: Option<PendingDoctype>,
    pub(in crate::html5::tree_builder) document_state: DocumentState,
    pub(in crate::html5::tree_builder) non_document_nodes_created: usize,
    // Do not push structural patches directly to `patches`.
    // Route structural edits through `push_structural_patch` so invariants stay checkable.
    pub(in crate::html5::tree_builder) patches: Vec<DomPatch>,
    pub(in crate::html5::tree_builder) last_text_patch:
        Option<crate::html5::tree_builder::coalescing::LastTextPatch>,
    pub(in crate::html5::tree_builder) structural_mutation_depth: u16,
    pub(in crate::html5::tree_builder) max_open_elements_depth: u32,
    pub(in crate::html5::tree_builder) max_active_formatting_depth: u32,
    pub(in crate::html5::tree_builder) perf_soe_push_ops: u64,
    pub(in crate::html5::tree_builder) perf_soe_pop_ops: u64,
    pub(in crate::html5::tree_builder) perf_soe_scope_scan_calls: u64,
    pub(in crate::html5::tree_builder) perf_soe_scope_scan_steps: u64,
    pub(in crate::html5::tree_builder) perf_soe_name_count_lookup_calls: u64,
    pub(in crate::html5::tree_builder) perf_soe_name_count_lookup_steps: u64,
    pub(in crate::html5::tree_builder) perf_soe_name_count_update_calls: u64,
    pub(in crate::html5::tree_builder) perf_soe_name_count_update_steps: u64,
    pub(in crate::html5::tree_builder) perf_soe_distinct_name_high_water: u32,
    pub(in crate::html5::tree_builder) perf_soe_end_tag_scan_calls: u64,
    pub(in crate::html5::tree_builder) perf_soe_end_tag_scan_steps: u64,
    pub(in crate::html5::tree_builder) perf_patches_emitted: u64,
    pub(in crate::html5::tree_builder) perf_text_nodes_created: u64,
    pub(in crate::html5::tree_builder) perf_text_appends: u64,
    pub(in crate::html5::tree_builder) perf_text_coalescing_invalidations: u64,
    pub(in crate::html5::tree_builder) perf_template_validation_fast_path_tokens: u64,
    pub(in crate::html5::tree_builder) perf_template_validation_transition_checks: u64,
    pub(in crate::html5::tree_builder) perf_max_same_token_cycle_states: u64,
    pub(in crate::html5::tree_builder) perf_template_close_ops: u64,
    pub(in crate::html5::tree_builder) perf_template_eof_unwind_iterations: u64,
    pub(in crate::html5::tree_builder) perf_reset_insertion_mode_scan_calls: u64,
    pub(in crate::html5::tree_builder) perf_reset_insertion_mode_scan_steps: u64,
    pub(in crate::html5::tree_builder) perf_template_recovery_owner_scan_calls: u64,
    pub(in crate::html5::tree_builder) perf_template_recovery_owner_scan_steps: u64,
    pub(in crate::html5::tree_builder) internal_post_adjustment_attribute_collisions: u64,
    #[cfg(any(
        test,
        feature = "html5-fuzzing",
        feature = "parser_invariants",
        feature = "debug-stats"
    ))]
    pub(in crate::html5::tree_builder) perf_template_full_audit_host_visits: u64,
    pub(in crate::html5::tree_builder) active_text_mode: Option<TextModeSpec>,
    pub(in crate::html5::tree_builder) form_element_pointer: Option<FormElementPointer>,
    pub(in crate::html5::tree_builder) pending_textarea_initial_lf:
        Option<PendingTextareaInitialLf>,
    pub(in crate::html5::tree_builder) foster_parenting_enabled: bool,
    pub(in crate::html5::tree_builder) pending_table_text: Option<PendingTableTextState>,
    pub(in crate::html5::tree_builder) pending_tokenizer_control: Option<TokenizerControl>,
    #[cfg(any(test, feature = "dom-snapshot", feature = "internal-api"))]
    pub(in crate::html5::tree_builder) parse_error_kinds: Vec<&'static str>,
}

#[cfg(any(test, feature = "debug-stats"))]
#[allow(
    dead_code,
    reason = "internal parser performance counters are consumed by test and debug-stat lanes"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeBuilderPerfStats {
    pub soe_push_ops: u64,
    /// Explicit SOE removals (`pop()` and successful `pop_until_including_in_scope()`).
    /// SOE resets via `clear()` are intentionally excluded.
    pub soe_pop_ops: u64,
    /// Scope scans across both probe-only checks (`has_in_scope`) and mutating
    /// close operations (`pop_until_including_in_scope`).
    pub soe_scope_scan_calls: u64,
    /// Total SOE entries inspected while performing scope scans.
    pub soe_scope_scan_steps: u64,
    pub soe_name_count_lookup_calls: u64,
    pub soe_name_count_lookup_steps: u64,
    pub soe_name_count_update_calls: u64,
    pub soe_name_count_update_steps: u64,
    pub soe_distinct_name_high_water: u32,
    /// Reverse SOE scans for the InBody "any other end tag" algorithm.
    /// These are deliberately separate from scope scans.
    pub soe_end_tag_scan_calls: u64,
    /// Total SOE entries inspected by generic end-tag scans.
    pub soe_end_tag_scan_steps: u64,
    pub patches_emitted: u64,
    pub text_nodes_created: u64,
    pub text_appends: u64,
    pub text_coalescing_invalidations: u64,
    pub template_validation_fast_path_tokens: u64,
    pub template_validation_transition_checks: u64,
    /// Peak exact semantic states retained while reprocessing one token.
    pub max_same_token_cycle_states: u64,
    pub template_close_ops: u64,
    pub template_eof_unwind_iterations: u64,
    pub reset_insertion_mode_scan_calls: u64,
    pub reset_insertion_mode_scan_steps: u64,
    pub template_recovery_owner_scan_calls: u64,
    pub template_recovery_owner_scan_steps: u64,
    pub template_full_audit_host_visits: u64,
}

#[cfg(any(test, feature = "html5-fuzzing"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreeBuilderProgressWitness {
    pub(crate) insertion_mode: InsertionMode,
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    pub(crate) table_text_original_insertion_mode: Option<InsertionMode>,
    pub(crate) active_text_mode: Option<TextModeSpec>,
    pub(crate) form_element_pointer: Option<PatchKey>,
    pub(crate) pending_textarea_initial_lf: Option<PatchKey>,
    pub(crate) head_element_pointer: Option<PatchKey>,
    pub(crate) template_modes: Vec<(PatchKey, TemplateInsertionMode)>,
    pub(crate) active_formatting_entries:
        Vec<crate::html5::tree_builder::formatting::AfeDiagnosticEntry>,
    pub(crate) open_element_keys: Vec<PatchKey>,
    pub(crate) current_table_key: Option<PatchKey>,
    pub(crate) pending_table_character_tokens: Vec<String>,
    pub(crate) pending_table_character_tokens_contains_non_space: bool,
    pub(crate) quirks_mode: crate::DocumentMode,
    pub(crate) frameset_ok: bool,
    pub(crate) foster_parenting_enabled: bool,
}

#[cfg(any(test, feature = "internal-api"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeBuilderStateSnapshot {
    pub(crate) insertion_mode: InsertionMode,
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    pub(crate) table_text_original_insertion_mode: Option<InsertionMode>,
    pub(crate) active_text_mode: Option<TextModeSpec>,
    pub(crate) form_element_pointer: Option<PatchKey>,
    pub(crate) pending_textarea_initial_lf: Option<PatchKey>,
    pub(crate) head_element_pointer: Option<PatchKey>,
    pub(crate) template_modes: Vec<(PatchKey, TemplateInsertionMode)>,
    pub(crate) active_formatting_entries:
        Vec<crate::html5::tree_builder::formatting::AfeDiagnosticEntry>,
    pub(crate) open_element_names: Vec<AtomId>,
    pub(crate) open_element_keys: Vec<PatchKey>,
    pub(crate) current_table_key: Option<PatchKey>,
    pub(crate) pending_table_character_tokens: Vec<String>,
    pub(crate) pending_table_character_tokens_contains_non_space: bool,
    pub(crate) quirks_mode: crate::DocumentMode,
    pub(crate) frameset_ok: bool,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn adjusted_current_node(
        &self,
    ) -> Option<crate::html5::tree_builder::foreign::AdjustedCurrentNode<'_>> {
        let current = self.open_elements.current()?;
        let (expanded_name, attributes) = self.live_tree.element_semantics(current.key())?;
        Some(
            crate::html5::tree_builder::foreign::AdjustedCurrentNode::from_stack_current(
                current.key(),
                expanded_name,
                attributes,
            ),
        )
    }

    pub(in crate::html5) fn adjusted_current_node_namespace(
        &self,
    ) -> Option<crate::names::ElementNamespace> {
        self.adjusted_current_node()
            .map(|node| node.expanded_name.namespace())
    }

    /// Synchronize the tokenizer's markup-declaration CDATA boundary before
    /// its next incremental pump.
    ///
    /// The tree builder remains the owner of adjusted-current-node semantics;
    /// the tokenizer receives only the namespace decision needed by the
    /// markup-declaration-open state. Pipeline drivers must call this once
    /// immediately before every tokenizer pump.
    pub fn prepare_tokenizer_pump(&self, tokenizer: &mut Html5Tokenizer) {
        tokenizer.set_adjusted_current_node_namespace(self.adjusted_current_node_namespace());
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn post_adjustment_attribute_collision_count(&self) -> u64 {
        self.internal_post_adjustment_attribute_collisions
    }

    pub fn new(
        config: TreeBuilderConfig,
        ctx: &mut DocumentParseContext,
    ) -> Result<Self, TreeBuilderError> {
        let known_tags = KnownTagIds::intern(&mut ctx.atoms).map_err(|_| EngineInvariantError)?;
        let scope_tags = known_tags.scope_tags();
        Ok(Self {
            config,
            atom_table_id: ctx.atoms.id(),
            insertion_mode: InsertionMode::Initial,
            original_insertion_mode: None,
            known_tags,
            scope_tags,
            live_tree: LiveTree::default(),
            open_elements: OpenElementsStack::new(ctx.atoms.id()),
            active_formatting: ActiveFormattingList::default(),
            template_modes: TemplateModeStack::default(),
            document_key: None,
            head_element_pointer: None,
            template_state_epoch: 0,
            accepted_template_count: 0,
            next_patch_key: NonZeroU32::MIN,
            pending_doctype: None,
            document_state: DocumentState::default(),
            non_document_nodes_created: 0,
            patches: Vec::new(),
            last_text_patch: None,
            structural_mutation_depth: 0,
            max_open_elements_depth: 0,
            max_active_formatting_depth: 0,
            perf_soe_push_ops: 0,
            perf_soe_pop_ops: 0,
            perf_soe_scope_scan_calls: 0,
            perf_soe_scope_scan_steps: 0,
            perf_soe_name_count_lookup_calls: 0,
            perf_soe_name_count_lookup_steps: 0,
            perf_soe_name_count_update_calls: 0,
            perf_soe_name_count_update_steps: 0,
            perf_soe_distinct_name_high_water: 0,
            perf_soe_end_tag_scan_calls: 0,
            perf_soe_end_tag_scan_steps: 0,
            perf_patches_emitted: 0,
            perf_text_nodes_created: 0,
            perf_text_appends: 0,
            perf_text_coalescing_invalidations: 0,
            perf_template_validation_fast_path_tokens: 0,
            perf_template_validation_transition_checks: 0,
            perf_max_same_token_cycle_states: 0,
            perf_template_close_ops: 0,
            perf_template_eof_unwind_iterations: 0,
            perf_reset_insertion_mode_scan_calls: 0,
            perf_reset_insertion_mode_scan_steps: 0,
            perf_template_recovery_owner_scan_calls: 0,
            perf_template_recovery_owner_scan_steps: 0,
            internal_post_adjustment_attribute_collisions: 0,
            #[cfg(any(
                test,
                feature = "html5-fuzzing",
                feature = "parser_invariants",
                feature = "debug-stats"
            ))]
            perf_template_full_audit_host_visits: 0,
            active_text_mode: None,
            form_element_pointer: None,
            pending_textarea_initial_lf: None,
            foster_parenting_enabled: false,
            pending_table_text: None,
            pending_tokenizer_control: None,
            #[cfg(any(test, feature = "dom-snapshot", feature = "internal-api"))]
            parse_error_kinds: Vec::new(),
        })
    }

    /// Process a token and buffer resulting patches internally.
    ///
    /// The caller may retrieve buffered patches with `drain_patches()`.
    /// This API is deterministic and equivalent to `push_token()` with a sink.
    ///
    /// Text-mode contract:
    /// - The returned [`TreeBuilderStepResult`] may contain tokenizer controls.
    /// - Those controls must be applied before the tokenizer consumes the next token.
    pub fn process(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        self.process_impl(token, atoms, text)
    }

    /// Push a token into the tree builder.
    ///
    /// Tokens are consumed in order; the builder may emit zero or more patches.
    /// The return value indicates whether parsing can continue or must suspend.
    ///
    /// This is the sink-based streaming adapter. For internal buffering, use
    /// `process()` + `drain_patches()`.
    ///
    /// Coalescing note:
    /// - With `coalesce_text=true`, coalescing state may continue across calls.
    /// - Therefore, `AppendText` emitted in a later call may target a node created
    ///   by an earlier flushed batch.
    ///
    /// Text-mode contract:
    /// - Apply `result.tokenizer_control` immediately after this call returns.
    /// - Do not let the tokenizer consume another token first.
    pub fn push_token(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
        sink: &mut dyn PatchSink,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        let result = self.process_impl(token, atoms, text)?;
        sink.push_many(&mut self.patches);
        Ok(result)
    }

    /// Drain patches produced by previous `process()` calls.
    ///
    /// Patch ordering is stable: the returned vector preserves source token order.
    #[must_use]
    pub fn drain_patches(&mut self) -> Vec<DomPatch> {
        std::mem::take(&mut self.patches)
    }

    /// Snapshot the tree builder's current structural DOM state for invariant checking.
    ///
    /// This is an advanced API intended for tests, fuzzers, and strict integration
    /// checks that need to validate emitted patch batches against the builder's
    /// live tree. Typed invariant failures are surfaced by
    /// `check_dom_invariants` / `check_patch_invariants`; the internal live-tree
    /// mirror itself remains assertion-based and treats violations as engine bugs.
    #[must_use]
    pub fn dom_invariant_state(&self) -> DomInvariantState {
        self.live_tree.invariant_state()
    }

    #[cfg(any(test, feature = "html5-fuzzing"))]
    pub(crate) fn progress_witness(&self) -> TreeBuilderProgressWitness {
        TreeBuilderProgressWitness {
            insertion_mode: self.insertion_mode,
            original_insertion_mode: self.original_insertion_mode,
            table_text_original_insertion_mode: self
                .pending_table_text
                .as_ref()
                .map(PendingTableTextState::original_insertion_mode),
            active_text_mode: self.active_text_mode,
            form_element_pointer: self.form_element_pointer.map(FormElementPointer::key),
            pending_textarea_initial_lf: self
                .pending_textarea_initial_lf
                .map(PendingTextareaInitialLf::textarea),
            head_element_pointer: self.head_element_pointer,
            template_modes: self.template_modes.snapshot(),
            active_formatting_entries: self.active_formatting.diagnostic_snapshot(),
            open_element_keys: (0..self.open_elements.len())
                .filter_map(|index| self.open_elements.get(index))
                .map(|entry| entry.key())
                .collect(),
            current_table_key: self.current_table_key(),
            pending_table_character_tokens: self
                .pending_table_text
                .as_ref()
                .map(|state| state.tokens().chunks().to_vec())
                .unwrap_or_default(),
            pending_table_character_tokens_contains_non_space: self
                .pending_table_text
                .as_ref()
                .is_some_and(|state| state.tokens().contains_non_space()),
            quirks_mode: self.document_state.quirks_mode,
            frameset_ok: self.document_state.frameset_ok,
            foster_parenting_enabled: self.foster_parenting_enabled,
        }
    }

    /// Internal metric: max open elements depth observed since session start.
    pub(crate) fn max_open_elements_depth(&self) -> u32 {
        self.max_open_elements_depth
    }

    /// Internal metric: max active formatting depth observed since session start.
    pub(crate) fn max_active_formatting_depth(&self) -> u32 {
        self.max_active_formatting_depth
    }

    pub(crate) fn perf_soe_push_ops(&self) -> u64 {
        self.perf_soe_push_ops
    }

    pub(crate) fn perf_soe_pop_ops(&self) -> u64 {
        self.perf_soe_pop_ops
    }

    pub(crate) fn perf_soe_scope_scan_calls(&self) -> u64 {
        self.perf_soe_scope_scan_calls
    }

    pub(crate) fn perf_soe_scope_scan_steps(&self) -> u64 {
        self.perf_soe_scope_scan_steps
    }

    pub(crate) fn perf_soe_name_count_lookup_calls(&self) -> u64 {
        self.perf_soe_name_count_lookup_calls
    }

    pub(crate) fn perf_soe_name_count_lookup_steps(&self) -> u64 {
        self.perf_soe_name_count_lookup_steps
    }

    pub(crate) fn perf_soe_name_count_update_calls(&self) -> u64 {
        self.perf_soe_name_count_update_calls
    }

    pub(crate) fn perf_soe_name_count_update_steps(&self) -> u64 {
        self.perf_soe_name_count_update_steps
    }

    pub(crate) fn perf_soe_distinct_name_high_water(&self) -> u32 {
        self.perf_soe_distinct_name_high_water
    }

    pub(crate) fn perf_patches_emitted(&self) -> u64 {
        self.perf_patches_emitted
    }

    pub(crate) fn perf_text_nodes_created(&self) -> u64 {
        self.perf_text_nodes_created
    }

    pub(crate) fn perf_text_appends(&self) -> u64 {
        self.perf_text_appends
    }

    pub(crate) fn perf_text_coalescing_invalidations(&self) -> u64 {
        self.perf_text_coalescing_invalidations
    }

    #[cfg(any(test, feature = "debug-stats"))]
    #[allow(
        dead_code,
        reason = "internal parser performance counters are consumed by test and debug-stat lanes"
    )]
    pub(crate) fn debug_perf_stats(&self) -> TreeBuilderPerfStats {
        TreeBuilderPerfStats {
            soe_push_ops: self.perf_soe_push_ops,
            soe_pop_ops: self.perf_soe_pop_ops,
            soe_scope_scan_calls: self.perf_soe_scope_scan_calls,
            soe_scope_scan_steps: self.perf_soe_scope_scan_steps,
            soe_name_count_lookup_calls: self.perf_soe_name_count_lookup_calls,
            soe_name_count_lookup_steps: self.perf_soe_name_count_lookup_steps,
            soe_name_count_update_calls: self.perf_soe_name_count_update_calls,
            soe_name_count_update_steps: self.perf_soe_name_count_update_steps,
            soe_distinct_name_high_water: self.perf_soe_distinct_name_high_water,
            soe_end_tag_scan_calls: self.perf_soe_end_tag_scan_calls,
            soe_end_tag_scan_steps: self.perf_soe_end_tag_scan_steps,
            patches_emitted: self.perf_patches_emitted,
            text_nodes_created: self.perf_text_nodes_created,
            text_appends: self.perf_text_appends,
            text_coalescing_invalidations: self.perf_text_coalescing_invalidations,
            template_validation_fast_path_tokens: self.perf_template_validation_fast_path_tokens,
            template_validation_transition_checks: self.perf_template_validation_transition_checks,
            max_same_token_cycle_states: self.perf_max_same_token_cycle_states,
            template_close_ops: self.perf_template_close_ops,
            template_eof_unwind_iterations: self.perf_template_eof_unwind_iterations,
            reset_insertion_mode_scan_calls: self.perf_reset_insertion_mode_scan_calls,
            reset_insertion_mode_scan_steps: self.perf_reset_insertion_mode_scan_steps,
            template_recovery_owner_scan_calls: self.perf_template_recovery_owner_scan_calls,
            template_recovery_owner_scan_steps: self.perf_template_recovery_owner_scan_steps,
            template_full_audit_host_visits: self.perf_template_full_audit_host_visits,
        }
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub fn state_snapshot(&self) -> TreeBuilderStateSnapshot {
        TreeBuilderStateSnapshot {
            insertion_mode: self.insertion_mode,
            original_insertion_mode: self.original_insertion_mode,
            table_text_original_insertion_mode: self
                .pending_table_text
                .as_ref()
                .map(PendingTableTextState::original_insertion_mode),
            active_text_mode: self.active_text_mode,
            form_element_pointer: self.form_element_pointer.map(FormElementPointer::key),
            pending_textarea_initial_lf: self
                .pending_textarea_initial_lf
                .map(PendingTextareaInitialLf::textarea),
            head_element_pointer: self.head_element_pointer,
            template_modes: self.template_modes.snapshot(),
            active_formatting_entries: self.active_formatting.diagnostic_snapshot(),
            open_element_names: self.open_elements.iter_names().collect(),
            open_element_keys: self.open_elements.iter_keys().collect(),
            current_table_key: self.current_table_key(),
            pending_table_character_tokens: self
                .pending_table_text
                .as_ref()
                .map(|state| state.tokens().chunks().to_vec())
                .unwrap_or_default(),
            pending_table_character_tokens_contains_non_space: self
                .pending_table_text
                .as_ref()
                .is_some_and(|state| state.tokens().contains_non_space()),
            quirks_mode: self.document_state.quirks_mode,
            frameset_ok: self.document_state.frameset_ok,
        }
    }

    #[cfg(any(test, feature = "dom-snapshot", feature = "internal-api"))]
    pub fn take_parse_error_kinds_for_test(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.parse_error_kinds)
    }

    #[cold]
    #[track_caller]
    pub(in crate::html5::tree_builder) fn assert_atom_table_binding(&self, atoms: &AtomTable) {
        let actual = atoms.id();
        let expected = self.atom_table_id;
        assert_eq!(
            actual, expected,
            "tree builder atom table mismatch (expected={expected}, actual={actual})"
        );
    }

    #[cfg(any(test, feature = "parser_invariants", feature = "html5-fuzzing"))]
    pub(in crate::html5::tree_builder) fn assert_open_element_name_invariants(
        &self,
        atoms: &AtomTable,
    ) {
        assert!(
            self.open_elements.name_cache_matches_stack(),
            "stack-of-open-elements expanded-name cache diverged from stack entries"
        );
        for entry in self.open_elements.iter_entries() {
            assert_eq!(
                u64::from(entry.name().interner_id()),
                self.atom_table_id,
                "open-element atom escaped its parser interner domain"
            );
            let atom_name = atoms
                .resolve(entry.name())
                .expect("open-element atom must resolve in the bound interner");
            let (expanded_name, _) = self
                .live_tree
                .element_semantics(entry.key())
                .expect("open-element identity must reference a live element");
            assert_eq!(expanded_name.namespace(), entry.namespace());
            assert_eq!(expanded_name.local_name().as_str(), atom_name);
        }
    }

    pub(in crate::html5::tree_builder) fn record_parse_error(
        &mut self,
        kind: &'static str,
        _tag: Option<AtomId>,
        _mode: Option<InsertionMode>,
    ) {
        #[cfg(any(test, feature = "dom-snapshot", feature = "internal-api"))]
        self.parse_error_kinds.push(kind);
        #[cfg(not(any(test, feature = "dom-snapshot", feature = "internal-api")))]
        {
            let _ = kind;
        }
    }
}

use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable, DocumentParseContext, EngineInvariantError, Token};
use crate::html5::tokenizer::{TextModeSpec, TextResolver, TokenizerControl};
use crate::html5::tree_builder::document::DocumentState;
use crate::html5::tree_builder::formatting::ActiveFormattingList;
use crate::html5::tree_builder::invariants::DomInvariantState;
use crate::html5::tree_builder::known_tags::KnownTagIds;
use crate::html5::tree_builder::live_tree::LiveTree;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::patch_sink::PatchSink;
use crate::html5::tree_builder::stack::{OpenElementsStack, ScopeTagSet};
use crate::html5::tree_builder::table::PendingTableCharacterTokens;
use std::num::NonZeroU32;

/// Centralized tree-builder hardening/resource bounds.
///
/// Recovery policy:
/// - a non-self-closing start tag that would exceed `max_open_elements_depth`
///   is ignored;
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
    pub(in crate::html5::tree_builder) document_key: Option<PatchKey>,
    pub(in crate::html5::tree_builder) next_patch_key: NonZeroU32,
    pub(in crate::html5::tree_builder) pending_doctype: Option<String>,
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
    pub(in crate::html5::tree_builder) perf_patches_emitted: u64,
    pub(in crate::html5::tree_builder) perf_text_nodes_created: u64,
    pub(in crate::html5::tree_builder) perf_text_appends: u64,
    pub(in crate::html5::tree_builder) perf_text_coalescing_invalidations: u64,
    pub(in crate::html5::tree_builder) active_text_mode: Option<TextModeSpec>,
    pub(in crate::html5::tree_builder) foster_parenting_enabled: bool,
    pub(in crate::html5::tree_builder) pending_table_character_tokens: PendingTableCharacterTokens,
    pub(in crate::html5::tree_builder) pending_tokenizer_control: Option<TokenizerControl>,
    #[cfg(any(test, feature = "internal-api"))]
    pub(in crate::html5::tree_builder) parse_error_kinds: Vec<&'static str>,
}

#[cfg(any(test, feature = "debug-stats"))]
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
    pub patches_emitted: u64,
    pub text_nodes_created: u64,
    pub text_appends: u64,
    pub text_coalescing_invalidations: u64,
}

#[cfg(any(test, feature = "html5-fuzzing"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreeBuilderProgressWitness {
    pub(crate) insertion_mode: InsertionMode,
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    pub(crate) active_text_mode: Option<TextModeSpec>,
    pub(crate) open_element_keys: Vec<PatchKey>,
    pub(crate) current_table_key: Option<PatchKey>,
    pub(crate) pending_table_character_tokens: Vec<String>,
    pub(crate) pending_table_character_tokens_contains_non_space: bool,
    pub(crate) quirks_mode: crate::html5::tree_builder::document::QuirksMode,
    pub(crate) frameset_ok: bool,
    pub(crate) foster_parenting_enabled: bool,
}

#[cfg(any(test, feature = "internal-api"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeBuilderStateSnapshot {
    pub(crate) insertion_mode: InsertionMode,
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    pub(crate) active_text_mode: Option<TextModeSpec>,
    pub(crate) open_element_names: Vec<AtomId>,
    pub(crate) open_element_keys: Vec<PatchKey>,
    pub(crate) current_table_key: Option<PatchKey>,
    pub(crate) pending_table_character_tokens: Vec<String>,
    pub(crate) pending_table_character_tokens_contains_non_space: bool,
    pub(crate) quirks_mode: crate::html5::tree_builder::document::QuirksMode,
    pub(crate) frameset_ok: bool,
}

impl Html5TreeBuilder {
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
            open_elements: OpenElementsStack::default(),
            active_formatting: ActiveFormattingList::default(),
            document_key: None,
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
            perf_patches_emitted: 0,
            perf_text_nodes_created: 0,
            perf_text_appends: 0,
            perf_text_coalescing_invalidations: 0,
            active_text_mode: None,
            foster_parenting_enabled: false,
            pending_table_character_tokens: PendingTableCharacterTokens::default(),
            pending_tokenizer_control: None,
            #[cfg(any(test, feature = "internal-api"))]
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
            active_text_mode: self.active_text_mode,
            open_element_keys: (0..self.open_elements.len())
                .filter_map(|index| self.open_elements.get(index))
                .map(|entry| entry.key())
                .collect(),
            current_table_key: self.current_table_key(),
            pending_table_character_tokens: self.pending_table_character_tokens.chunks().to_vec(),
            pending_table_character_tokens_contains_non_space: self
                .pending_table_character_tokens
                .contains_non_space(),
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
    pub(crate) fn debug_perf_stats(&self) -> TreeBuilderPerfStats {
        TreeBuilderPerfStats {
            soe_push_ops: self.perf_soe_push_ops,
            soe_pop_ops: self.perf_soe_pop_ops,
            soe_scope_scan_calls: self.perf_soe_scope_scan_calls,
            soe_scope_scan_steps: self.perf_soe_scope_scan_steps,
            patches_emitted: self.perf_patches_emitted,
            text_nodes_created: self.perf_text_nodes_created,
            text_appends: self.perf_text_appends,
            text_coalescing_invalidations: self.perf_text_coalescing_invalidations,
        }
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub fn state_snapshot(&self) -> TreeBuilderStateSnapshot {
        TreeBuilderStateSnapshot {
            insertion_mode: self.insertion_mode,
            original_insertion_mode: self.original_insertion_mode,
            active_text_mode: self.active_text_mode,
            open_element_names: self.open_elements.iter_names().collect(),
            open_element_keys: self.open_elements.iter_keys().collect(),
            current_table_key: self.current_table_key(),
            pending_table_character_tokens: self.pending_table_character_tokens.chunks().to_vec(),
            pending_table_character_tokens_contains_non_space: self
                .pending_table_character_tokens
                .contains_non_space(),
            quirks_mode: self.document_state.quirks_mode,
            frameset_ok: self.document_state.frameset_ok,
        }
    }

    #[cfg(any(test, feature = "internal-api"))]
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

    pub(in crate::html5::tree_builder) fn record_parse_error(
        &mut self,
        kind: &'static str,
        _tag: Option<AtomId>,
        _mode: Option<InsertionMode>,
    ) {
        #[cfg(any(test, feature = "internal-api"))]
        self.parse_error_kinds.push(kind);
        #[cfg(not(any(test, feature = "internal-api")))]
        {
            let _ = kind;
        }
    }
}

//! HTML5 tree builder public API.
//!
//! Consumes HTML5 tokens and emits DOM mutation patches. The builder owns all
//! tree-construction state (insertion modes, stack of open elements, active
//! formatting list, etc.) and is resumable across token boundaries.

use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{
    AtomError, AtomId, AtomTable, DocumentParseContext, EngineInvariantError, Token,
};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::emit::{emit_append_child, emit_create_element};
use crate::html5::tree_builder::formatting::ActiveFormattingList;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::{OpenElement, OpenElementsStack, ScopeKind, ScopeTagSet};
use std::num::NonZeroU32;

/// Deterministic DOM serializer for HTML5 tree-builder tests.
///
/// This produces the `html5-dom-v1` line format used by golden fixtures and
/// WPT DOM expected files:
/// - document: `#document` (optionally with doctype)
/// - element: `<name attr="value">`
/// - text: `"..."` with deterministic escaping
/// - comment: `<!-- ... -->` with deterministic escaping
///
/// Output ordering is stable and platform-independent:
/// - child traversal follows source tree order
/// - attributes are emitted in lexical name order (with deterministic
///   name-equal tie-breaking)
/// - escaping is explicit (`\n`, `\r`, `\t`, `\\`, `\"`, non-ASCII as `\u{HEX}`)
#[cfg(feature = "dom-snapshot")]
pub fn serialize_dom_for_test(root: &crate::Node) -> Vec<String> {
    serialize_dom_for_test_with_options(root, crate::dom_snapshot::DomSnapshotOptions::default())
}

/// Deterministic DOM serializer for HTML5 tree-builder tests with explicit
/// snapshot options.
#[cfg(feature = "dom-snapshot")]
pub fn serialize_dom_for_test_with_options(
    root: &crate::Node,
    options: crate::dom_snapshot::DomSnapshotOptions,
) -> Vec<String> {
    crate::dom_snapshot::DomSnapshot::new(root, options)
        .as_lines()
        .to_vec()
}

#[derive(Clone, Debug, Default)]
pub struct TreeBuilderConfig {
    /// Whether to coalesce adjacent text nodes within a batch.
    /// Coalescing must be deterministic and purely local (no buffering thresholds).
    pub coalesce_text: bool,
}

/// Tree builder step result.
#[must_use]
#[derive(Clone, Debug)]
pub enum TreeBuilderStepResult {
    Continue,
    Suspend(SuspendReason),
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuirksMode {
    NoQuirks,
    Quirks,
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentState {
    quirks_mode: QuirksMode,
    frameset_ok: bool,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            quirks_mode: QuirksMode::NoQuirks,
            frameset_ok: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LastTextPatch {
    parent: PatchKey,
    text_key: PatchKey,
    create_patch_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DispatchOutcome {
    Done,
    Reprocess(InsertionMode),
}

/// Patch sink for streaming emission.
pub trait PatchSink {
    fn push(&mut self, patch: DomPatch);

    fn extend_owned(&mut self, patches: Vec<DomPatch>) {
        for patch in patches {
            self.push(patch);
        }
    }

    /// Drains `patches` to enable caller-owned buffer reuse without reallocating.
    fn push_many(&mut self, patches: &mut Vec<DomPatch>) {
        for patch in patches.drain(..) {
            self.push(patch);
        }
    }
}

/// Patch sink that buffers into a Vec.
pub struct VecPatchSink<'a>(pub &'a mut Vec<DomPatch>);

impl<'a> PatchSink for VecPatchSink<'a> {
    fn push(&mut self, patch: DomPatch) {
        self.0.push(patch);
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
    config: TreeBuilderConfig,
    atom_table_id: u64,
    insertion_mode: InsertionMode,
    original_insertion_mode: Option<InsertionMode>,
    known_tags: KnownTagIds,
    scope_tags: ScopeTagSet,
    open_elements: OpenElementsStack,
    active_formatting: ActiveFormattingList,
    document_key: Option<PatchKey>,
    next_patch_key: NonZeroU32,
    pending_doctype: Option<String>,
    document_state: DocumentState,
    patches: Vec<DomPatch>,
    last_text_patch: Option<LastTextPatch>,
    max_open_elements_depth: u32,
    max_active_formatting_depth: u32,
}

#[cfg(any(test, feature = "internal-api"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeBuilderStateSnapshot {
    pub(crate) insertion_mode: InsertionMode,
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    pub(crate) open_element_names: Vec<AtomId>,
    pub(crate) open_element_keys: Vec<PatchKey>,
    pub(crate) quirks_mode: QuirksMode,
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
            open_elements: OpenElementsStack::default(),
            active_formatting: ActiveFormattingList::default(),
            document_key: None,
            next_patch_key: NonZeroU32::MIN,
            pending_doctype: None,
            document_state: DocumentState::default(),
            patches: Vec::new(),
            last_text_patch: None,
            max_open_elements_depth: 0,
            max_active_formatting_depth: 0,
        })
    }

    /// Process a token and buffer resulting patches internally.
    ///
    /// The caller may retrieve buffered patches with `drain_patches()`.
    /// This API is deterministic and equivalent to `push_token()` with a sink.
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
    pub fn push_token(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
        sink: &mut dyn PatchSink,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        let result = self.process_impl(token, atoms, text)?;
        sink.push_many(&mut self.patches);
        self.last_text_patch = None;
        Ok(result)
    }

    /// Drain patches produced by previous `process()` calls.
    ///
    /// Patch ordering is stable: the returned vector preserves source token order.
    #[must_use]
    pub fn drain_patches(&mut self) -> Vec<DomPatch> {
        self.last_text_patch = None;
        std::mem::take(&mut self.patches)
    }

    fn process_impl(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        self.assert_atom_table_binding(atoms);
        let mut mode = self.insertion_mode;
        let mut handled = false;
        let mut last_successful_mode = self.insertion_mode;
        for _ in 0..12 {
            self.insertion_mode = mode;
            let outcome = match mode {
                InsertionMode::Initial => self.handle_initial(token, atoms, text)?,
                InsertionMode::BeforeHtml => self.handle_before_html(token, atoms, text)?,
                InsertionMode::BeforeHead => self.handle_before_head(token, atoms, text)?,
                InsertionMode::InHead => self.handle_in_head(token, atoms, text)?,
                InsertionMode::AfterHead => self.handle_after_head(token, atoms, text)?,
                InsertionMode::InBody => self.handle_in_body(token, atoms, text)?,
                InsertionMode::Text => self.handle_text_mode(token, atoms, text)?,
            };
            match outcome {
                DispatchOutcome::Done => {
                    handled = true;
                    last_successful_mode = self.insertion_mode;
                    break;
                }
                DispatchOutcome::Reprocess(next_mode) => {
                    mode = next_mode;
                }
            }
        }
        if !handled {
            self.record_parse_error("mode-reprocess-budget-exhausted", None, Some(mode));
            self.insertion_mode = last_successful_mode;
        }
        self.max_open_elements_depth = self
            .max_open_elements_depth
            .max(self.open_elements.max_depth());
        self.max_active_formatting_depth = self
            .max_active_formatting_depth
            .max(self.active_formatting.max_depth());
        // Core-v0 routing is fully recoverable and never suspends yet.
        // Suspend paths remain reserved for script/loading integration.
        Ok(TreeBuilderStepResult::Continue)
    }

    /// Internal metric: max open elements depth observed since session start.
    pub(crate) fn max_open_elements_depth(&self) -> u32 {
        self.max_open_elements_depth
    }

    /// Internal metric: max active formatting depth observed since session start.
    pub(crate) fn max_active_formatting_depth(&self) -> u32 {
        self.max_active_formatting_depth
    }

    fn alloc_patch_key(&mut self) -> Result<PatchKey, TreeBuilderError> {
        let key = PatchKey(self.next_patch_key.get());
        let next = self
            .next_patch_key
            .get()
            .checked_add(1)
            .ok_or(EngineInvariantError)?;
        self.next_patch_key = NonZeroU32::new(next).ok_or(EngineInvariantError)?;
        Ok(key)
    }

    #[cold]
    #[track_caller]
    fn assert_atom_table_binding(&self, atoms: &AtomTable) {
        let actual = atoms.id();
        let expected = self.atom_table_id;
        assert_eq!(
            actual, expected,
            "tree builder atom table mismatch (expected={expected}, actual={actual})"
        );
    }

    fn ensure_document_created(&mut self) -> Result<PatchKey, TreeBuilderError> {
        if let Some(key) = self.document_key {
            return Ok(key);
        }
        self.invalidate_text_coalescing();
        let key = self.alloc_patch_key()?;
        self.patches.push(DomPatch::CreateDocument {
            key,
            doctype: self.pending_doctype.take(),
        });
        self.document_key = Some(key);
        self.insertion_mode = InsertionMode::BeforeHtml;
        self.open_elements.clear();
        self.active_formatting.clear();
        self.original_insertion_mode = None;
        self.document_state.frameset_ok = true;
        Ok(key)
    }

    fn handle_initial(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype {
                name, force_quirks, ..
            } => {
                self.handle_doctype(name, *force_quirks, atoms)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
                Ok(DispatchOutcome::Done)
            }
            _ => {
                self.record_parse_error("initial-unexpected-token", None, None);
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHtml))
            }
        }
    }

    fn handle_before_html(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error("before-html-doctype", None, None);
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.insert_element(self.known_tags.html, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHead))
            }
            _ => {
                let _ = self.insert_element(self.known_tags.html, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHead))
            }
        }
    }

    fn handle_before_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.head => {
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.insert_element(self.known_tags.head, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InHead))
            }
            _ => {
                let _ = self.insert_element(self.known_tags.head, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InHead))
            }
        }
    }

    fn handle_in_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_parse_error("in-head-doctype", None, None);
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.script
                || *name == self.known_tags.style
                || *name == self.known_tags.title
                || *name == self.known_tags.textarea =>
            {
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                self.original_insertion_mode = Some(self.insertion_mode);
                self.insertion_mode = InsertionMode::Text;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.head => {
                let _ = self.close_element_in_scope(*name, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            _ => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
        }
    }

    fn handle_after_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.body => {
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
            _ => {
                let _ = self.insert_element(self.known_tags.body, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
        }
    }

    fn handle_in_body(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error("in-body-doctype", None, None);
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                self.update_mode_for_start_tag(*name);
            }
            Token::EndTag { name } => {
                let scope = self.scope_kind_for_in_body_end_tag(*name);
                let _ = self.close_element_in_scope(*name, scope);
                self.update_mode_for_end_tag(*name);
            }
            Token::Text { text: token_text } => {
                self.insert_text(token_text, text)?;
                self.insertion_mode = InsertionMode::InBody;
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
            }
        }
        Ok(DispatchOutcome::Done)
    }

    fn handle_text_mode(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::EndTag { name }
                if *name == self.known_tags.script
                    || *name == self.known_tags.style
                    || *name == self.known_tags.title
                    || *name == self.known_tags.textarea =>
            {
                let _ = self.close_element_in_scope(*name, ScopeKind::InScope);
                self.insertion_mode = self
                    .original_insertion_mode
                    .take()
                    .unwrap_or(InsertionMode::InBody);
            }
            Token::Text { text: token_text } => {
                self.insert_text(token_text, text)?;
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
            }
            Token::Eof => {
                self.record_parse_error("eof-in-text-mode", None, None);
                let _ = self.ensure_document_created()?;
                self.insertion_mode = self
                    .original_insertion_mode
                    .take()
                    .unwrap_or(InsertionMode::InBody);
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                // Core-v0 safety rule: do not mutate tree structure from Text mode
                // on unexpected start tags; keep stack shape deterministic and recoverable.
                self.record_parse_error("start-tag-in-text-mode", Some(*name), None);
                let tag_name = resolve_atom(atoms, *name)?;
                if attrs.iter().any(|attr| attr.value.is_some()) {
                    self.record_parse_error(
                        "text-mode-literalized-start-tag-attribute-values-dropped",
                        Some(*name),
                        None,
                    );
                }
                let mut attr_names = Vec::with_capacity(attrs.len());
                for attr in attrs {
                    attr_names.push(resolve_atom(atoms, attr.name)?.to_string());
                }
                attr_names.sort();
                // Recovery formatting is intentionally non-spec serialization;
                // deduping names keeps malformed-token diffs stable and less noisy.
                let len_before_dedup = attr_names.len();
                attr_names.dedup();
                if attr_names.len() != len_before_dedup {
                    self.record_parse_error(
                        "text-mode-literalized-start-tag-duplicate-attributes-deduped",
                        Some(*name),
                        None,
                    );
                }
                let mut literal = String::new();
                literal.push('<');
                literal.push_str(tag_name);
                for attr_name in attr_names {
                    literal.push(' ');
                    literal.push_str(&attr_name);
                }
                if *self_closing {
                    literal.push_str("/>");
                } else {
                    literal.push('>');
                }
                self.insert_recovery_literal_text(&literal)?;
            }
            Token::Doctype { .. } => {
                self.record_parse_error("doctype-in-text-mode", None, None);
            }
            Token::EndTag { name } => {
                self.record_parse_error("unexpected-end-tag-in-text-mode", Some(*name), None);
                let tag_name = resolve_atom(atoms, *name)?;
                let literal = format!("</{tag_name}>");
                self.insert_recovery_literal_text(&literal)?;
            }
        }
        Ok(DispatchOutcome::Done)
    }

    fn handle_doctype(
        &mut self,
        name: &Option<AtomId>,
        force_quirks: bool,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        self.invalidate_text_coalescing();
        if self.document_key.is_none() && self.pending_doctype.is_none() {
            self.pending_doctype = match name {
                Some(id) => Some(resolve_atom(atoms, *id)?.to_string()),
                None => None,
            };
        }
        if force_quirks {
            self.document_state.quirks_mode = QuirksMode::Quirks;
        }
        Ok(())
    }

    fn insert_element(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<PatchKey, TreeBuilderError> {
        // All SOE push/pop mutations must flow through central helpers that
        // invalidate coalescing state before changing tree structure.
        self.invalidate_text_coalescing();
        let document_key = self.ensure_document_created()?;
        let element_name = resolve_atom_arc(atoms, name)?;
        let parent = self
            .open_elements
            .current()
            .map(OpenElement::key)
            .unwrap_or(document_key);
        let key = self.alloc_patch_key()?;
        let mut attributes = Vec::with_capacity(attrs.len());
        for attr in attrs {
            let attr_name = resolve_atom_arc(atoms, attr.name)?;
            let attr_value = resolve_attribute_value(attr, text)?;
            attributes.push((attr_name, attr_value));
        }
        emit_create_element(&mut self.patches, key, element_name, attributes);
        emit_append_child(&mut self.patches, parent, key);
        if !self_closing {
            self.open_elements.push(OpenElement::new(key, name));
        }
        Ok(key)
    }

    fn insert_text(
        &mut self,
        token_text: &crate::html5::shared::TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let resolved = resolve_text_value(token_text, text)?;
        self.insert_resolved_text(&resolved)
    }

    fn insert_literal_text(&mut self, literal: &str) -> Result<(), TreeBuilderError> {
        self.insert_resolved_text(literal)
    }

    fn insert_recovery_literal_text(&mut self, literal: &str) -> Result<(), TreeBuilderError> {
        // Keep synthetic recovery artifacts distinct from adjacent content.
        self.invalidate_text_coalescing();
        self.insert_literal_text(literal)?;
        self.invalidate_text_coalescing();
        Ok(())
    }

    fn insert_resolved_text(&mut self, resolved: &str) -> Result<(), TreeBuilderError> {
        if resolved.is_empty() {
            return Ok(());
        }
        let document_key = self.ensure_document_created()?;
        let parent = self
            .open_elements
            .current()
            .map(OpenElement::key)
            .unwrap_or(document_key);
        if self.config.coalesce_text
            && let Some(last) = self.last_text_patch
            && last.parent == parent
            && let Some(DomPatch::CreateText {
                key,
                text: existing_text,
            }) = self.patches.get_mut(last.create_patch_index)
            && *key == last.text_key
        {
            existing_text.push_str(resolved);
            return Ok(());
        }
        let key = self.alloc_patch_key()?;
        let create_patch_index = self.patches.len();
        self.patches.push(DomPatch::CreateText {
            key,
            text: resolved.to_string(),
        });
        emit_append_child(&mut self.patches, parent, key);
        self.last_text_patch = if self.config.coalesce_text {
            Some(LastTextPatch {
                parent,
                text_key: key,
                create_patch_index,
            })
        } else {
            None
        };
        Ok(())
    }

    fn insert_comment(
        &mut self,
        token_text: &crate::html5::shared::TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.invalidate_text_coalescing();
        let resolved = resolve_text_value(token_text, text)?;
        let document_key = self.ensure_document_created()?;
        let parent = self
            .open_elements
            .current()
            .map(OpenElement::key)
            .unwrap_or(document_key);
        let key = self.alloc_patch_key()?;
        self.patches.push(DomPatch::CreateComment {
            key,
            text: resolved,
        });
        emit_append_child(&mut self.patches, parent, key);
        Ok(())
    }

    fn close_element_in_scope(&mut self, name: AtomId, scope: ScopeKind) -> bool {
        // Keep this as the single end-tag stack mutation path in Core-v0 so
        // coalescing invalidation stays aligned with parent/adjacency changes.
        let popped = self
            .open_elements
            .pop_until_including_in_scope(name, scope, &self.scope_tags);
        if popped.is_none() {
            self.record_parse_error("end-tag-not-in-scope", Some(name), None);
            return false;
        }
        self.invalidate_text_coalescing();
        true
    }

    fn invalidate_text_coalescing(&mut self) {
        self.last_text_patch = None;
    }

    fn record_parse_error(
        &mut self,
        _kind: &'static str,
        _tag: Option<AtomId>,
        _mode: Option<InsertionMode>,
    ) {
        // Core-v0 intentionally keeps parse errors recoverable and non-fatal.
    }

    fn update_mode_for_start_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.html {
            InsertionMode::BeforeHead
        } else if name == self.known_tags.head {
            InsertionMode::InHead
        } else if name == self.known_tags.body {
            InsertionMode::InBody
        } else if name == self.known_tags.script
            || name == self.known_tags.style
            || name == self.known_tags.title
            || name == self.known_tags.textarea
        {
            self.original_insertion_mode = Some(self.insertion_mode);
            InsertionMode::Text
        } else {
            InsertionMode::InBody
        };
    }

    fn update_mode_for_end_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.head {
            InsertionMode::AfterHead
        } else if name == self.known_tags.script
            || name == self.known_tags.style
            || name == self.known_tags.title
            || name == self.known_tags.textarea
        {
            self.original_insertion_mode
                .take()
                .unwrap_or(InsertionMode::InBody)
        } else if name == self.known_tags.body {
            InsertionMode::InBody
        } else {
            self.insertion_mode
        };
    }

    // Core-v0 coupling: this scope decision is specific to the current InBody
    // end-tag path and is not a universal "tag -> scope" rule.
    fn scope_kind_for_in_body_end_tag(&self, name: AtomId) -> ScopeKind {
        if name == self.known_tags.button {
            ScopeKind::Button
        } else if name == self.known_tags.li {
            ScopeKind::ListItem
        } else if name == self.known_tags.table {
            ScopeKind::Table
        } else {
            ScopeKind::InScope
        }
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub fn state_snapshot(&self) -> TreeBuilderStateSnapshot {
        TreeBuilderStateSnapshot {
            insertion_mode: self.insertion_mode,
            original_insertion_mode: self.original_insertion_mode,
            open_element_names: self.open_elements.iter_names().collect(),
            open_element_keys: self.open_elements.iter_keys().collect(),
            quirks_mode: self.document_state.quirks_mode,
            frameset_ok: self.document_state.frameset_ok,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct KnownTagIds {
    html: AtomId,
    head: AtomId,
    body: AtomId,
    script: AtomId,
    style: AtomId,
    title: AtomId,
    textarea: AtomId,
    table: AtomId,
    template: AtomId,
    td: AtomId,
    th: AtomId,
    caption: AtomId,
    marquee: AtomId,
    object: AtomId,
    applet: AtomId,
    button: AtomId,
    ol: AtomId,
    ul: AtomId,
    li: AtomId,
}

impl KnownTagIds {
    fn intern(atoms: &mut AtomTable) -> Result<Self, AtomError> {
        Ok(Self {
            html: atoms.intern_ascii_folded("html")?,
            head: atoms.intern_ascii_folded("head")?,
            body: atoms.intern_ascii_folded("body")?,
            script: atoms.intern_ascii_folded("script")?,
            style: atoms.intern_ascii_folded("style")?,
            title: atoms.intern_ascii_folded("title")?,
            textarea: atoms.intern_ascii_folded("textarea")?,
            table: atoms.intern_ascii_folded("table")?,
            template: atoms.intern_ascii_folded("template")?,
            td: atoms.intern_ascii_folded("td")?,
            th: atoms.intern_ascii_folded("th")?,
            caption: atoms.intern_ascii_folded("caption")?,
            marquee: atoms.intern_ascii_folded("marquee")?,
            object: atoms.intern_ascii_folded("object")?,
            applet: atoms.intern_ascii_folded("applet")?,
            button: atoms.intern_ascii_folded("button")?,
            ol: atoms.intern_ascii_folded("ol")?,
            ul: atoms.intern_ascii_folded("ul")?,
            li: atoms.intern_ascii_folded("li")?,
        })
    }

    #[inline]
    fn scope_tags(&self) -> ScopeTagSet {
        ScopeTagSet {
            html: self.html,
            table: self.table,
            template: self.template,
            td: self.td,
            th: self.th,
            caption: self.caption,
            marquee: self.marquee,
            object: self.object,
            applet: self.applet,
            button: self.button,
            ol: self.ol,
            ul: self.ul,
        }
    }
}

fn resolve_atom(atoms: &AtomTable, id: AtomId) -> Result<&str, TreeBuilderError> {
    atoms.resolve(id).ok_or(EngineInvariantError)
}

fn resolve_atom_arc(
    atoms: &AtomTable,
    id: AtomId,
) -> Result<std::sync::Arc<str>, TreeBuilderError> {
    atoms.resolve_arc(id).ok_or(EngineInvariantError)
}

fn resolve_attribute_value(
    attribute: &crate::html5::shared::Attribute,
    text: &dyn TextResolver,
) -> Result<Option<String>, TreeBuilderError> {
    match &attribute.value {
        None => Ok(None),
        Some(crate::html5::shared::AttributeValue::Owned(value)) => Ok(Some(value.clone())),
        Some(crate::html5::shared::AttributeValue::Span(span)) => text
            .resolve_span(*span)
            .map(|value| Some(value.to_string()))
            .map_err(|_| EngineInvariantError),
    }
}

fn resolve_text_value(
    value: &crate::html5::shared::TextValue,
    text: &dyn TextResolver,
) -> Result<String, TreeBuilderError> {
    match value {
        crate::html5::shared::TextValue::Owned(value) => Ok(value.clone()),
        crate::html5::shared::TextValue::Span(span) => text
            .resolve_span(*span)
            .map(|value| value.to_string())
            .map_err(|_| EngineInvariantError),
    }
}

mod emit;
mod formatting;
mod modes;
mod stack;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod internal_tests {
    use super::{DocumentParseContext, KnownTagIds};

    #[test]
    fn known_tag_scope_tag_view_shares_ids() {
        let mut ctx = DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx.atoms).expect("known tags");
        let scope = known.scope_tags();

        assert_eq!(scope.html, known.html);
        assert_eq!(scope.table, known.table);
        assert_eq!(scope.template, known.template);
        assert_eq!(scope.td, known.td);
        assert_eq!(scope.th, known.th);
        assert_eq!(scope.caption, known.caption);
        assert_eq!(scope.marquee, known.marquee);
        assert_eq!(scope.object, known.object);
        assert_eq!(scope.applet, known.applet);
        assert_eq!(scope.button, known.button);
        assert_eq!(scope.ol, known.ol);
        assert_eq!(scope.ul, known.ul);
    }
}

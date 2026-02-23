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
        Ok(result)
    }

    /// Drain patches produced by previous `process()` calls.
    ///
    /// Patch ordering is stable: the returned vector preserves source token order.
    #[must_use]
    pub fn drain_patches(&mut self) -> Vec<DomPatch> {
        std::mem::take(&mut self.patches)
    }

    fn process_impl(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        self.assert_atom_table_binding(atoms);
        // Core-v0 scaffold: config is wired into state; coalescing behavior lands
        // in a follow-up and should not be dropped from the public config.
        debug_assert!(
            !self.config.coalesce_text,
            "coalesce_text is configured but not implemented (Core-v0 scaffold)"
        );
        // TODO(html5/tree_builder): implement deterministic text coalescing
        // behavior when `self.config.coalesce_text` is enabled.
        match token {
            Token::Doctype {
                name, force_quirks, ..
            } => {
                if self.document_key.is_none() && self.pending_doctype.is_none() {
                    self.pending_doctype = match name {
                        Some(id) => Some(resolve_atom(atoms, *id)?.to_string()),
                        None => None,
                    };
                }
                if *force_quirks {
                    self.document_state.quirks_mode = QuirksMode::Quirks;
                }
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let document_key = self.ensure_document_created()?;
                let element_name = resolve_atom_arc(atoms, *name)?;
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
                if !*self_closing {
                    self.open_elements.push(OpenElement::new(key, *name));
                }
                self.update_mode_for_start_tag(*name);
            }
            Token::EndTag { name } => {
                // Core-v0: only pop-until-matching when the target is in baseline
                // HTML scope; complete scope + implied-end-tag logic lands later.
                let scope = self.scope_kind_for_in_body_end_tag(*name);
                // Core-v0 intentionally ignores the matched element details.
                let _ =
                    self.open_elements
                        .pop_until_including_in_scope(*name, scope, &self.scope_tags);
                self.update_mode_for_end_tag(*name);
            }
            Token::Text { text: token_text } => {
                let resolved = resolve_text_value(token_text, text)?;
                if !resolved.is_empty() {
                    let document_key = self.ensure_document_created()?;
                    let parent = self
                        .open_elements
                        .current()
                        .map(OpenElement::key)
                        .unwrap_or(document_key);
                    let key = self.alloc_patch_key()?;
                    self.patches.push(DomPatch::CreateText {
                        key,
                        text: resolved,
                    });
                    emit_append_child(&mut self.patches, parent, key);
                }
                self.insertion_mode = InsertionMode::InBody;
            }
            Token::Comment { text: token_text } => {
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
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
            }
        }
        self.max_open_elements_depth = self
            .max_open_elements_depth
            .max(self.open_elements.max_depth());
        self.max_active_formatting_depth = self
            .max_active_formatting_depth
            .max(self.active_formatting.max_depth());
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

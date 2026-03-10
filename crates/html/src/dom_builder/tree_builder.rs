use crate::dom_patch::DomPatch;
use crate::types::{AtomTable, Node, NodeKey, Token, TokenStream, debug_assert_lowercase_atom};
use std::sync::Arc;

use super::arena::{ArenaNode, NodeArena};
use super::patches::patch_key;
use super::text::PendingText;
use super::{TokenTextResolver, TreeBuilderConfig, TreeBuilderError, TreeBuilderResult};

#[derive(Clone, Copy, Debug)]
pub(super) enum InsertionMode {
    // Placeholder: will diverge once basic html/body handling and other modes land.
    Initial,
}

/// Incremental DOM construction state machine.
///
/// Invariants:
/// - `root_index` always points at the document node in `arena`.
/// - `open_elements` stores arena indices for open element nodes (never the document node).
/// - `open_elements.last()` is the current insertion parent when non-empty.
/// - `pending_text` (when enabled) is tied to the parent index that was current when buffering.
/// - Node keys are monotonically assigned and never reused within a document lifetime.
/// - The document root is assigned the first key for the parse.
/// - Keys are stable for the lifetime of the built DOM; they are never re-assigned.
/// - Patch appliers must treat unknown keys as protocol violations; keys are introduced
///   only when nodes are created and remain valid for the document lifetime.
/// - When text coalescing is enabled, a single text node is created per insertion
///   point and subsequent text tokens are buffered; `SetText` is emitted only at
///   flush boundaries (tag/comment/doctype/end/finish).
/// - `CreateDocument` is emitted before the first non-doctype mutation patch; doctype
///   is captured before that point, and doctype-after-emission is rejected.
pub struct TreeBuilder {
    pub(super) arena: NodeArena,
    pub(super) root_index: usize,
    pub(super) root_key: NodeKey,
    pub(super) open_elements: Vec<usize>,
    pub(super) pending_text: Option<PendingText>,
    #[allow(dead_code, reason = "placeholder for upcoming insertion mode handling")]
    pub(super) insertion_mode: InsertionMode,
    pub(super) patches: Vec<DomPatch>,
    pub(super) document_emitted: bool,
    pub(super) coalesce_text: bool,
    pub(super) finished: bool,
}

impl TreeBuilder {
    pub fn with_capacity(node_capacity: usize) -> Self {
        Self::with_capacity_and_config(node_capacity, TreeBuilderConfig::default())
    }

    pub fn with_capacity_and_config(node_capacity: usize, config: TreeBuilderConfig) -> Self {
        // Node keys are unique and stable for this document's lifetime. Cross-parse
        // stability requires a persistent allocator and is a future milestone.
        // Tokenizer uses text spans to avoid allocation; DOM materialization still
        // owns text buffers (Node::Text uses String).
        let mut arena = NodeArena::with_capacity(node_capacity);
        let root_key = arena.alloc_key();
        let root_index = arena.push(ArenaNode::Document {
            key: root_key,
            children: Vec::new(),
            doctype: None,
        });

        let open_capacity = node_capacity.saturating_sub(1).min(1024);
        Self {
            arena,
            root_index,
            root_key,
            open_elements: Vec::with_capacity(open_capacity),
            pending_text: None,
            insertion_mode: InsertionMode::Initial,
            patches: Vec::new(),
            document_emitted: false,
            coalesce_text: config.coalesce_text,
            finished: false,
        }
    }

    #[allow(
        dead_code,
        reason = "used by tests; runtime toggling is planned for streaming parse"
    )]
    pub fn set_coalesce_text(&mut self, enabled: bool) -> TreeBuilderResult<()> {
        if self.coalesce_text && !enabled {
            self.finalize_pending_text()?;
        }
        self.coalesce_text = enabled;
        Ok(())
    }

    pub fn push_token<R: TokenTextResolver + ?Sized>(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text_resolver: &R,
    ) -> TreeBuilderResult<()> {
        #[cfg(feature = "parse-guards")]
        crate::parse_guards::record_token_processed();
        if self.finished {
            return Err(TreeBuilderError::Finished);
        }

        match token {
            Token::Doctype(s) => self.handle_doctype(s.as_str(text_resolver.source()))?,
            Token::Comment(c) => self.handle_comment(c.as_str(text_resolver.source()))?,
            Token::TextSpan { .. } | Token::TextOwned { .. } => {
                self.handle_text(token, text_resolver)?
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
                ..
            } => self.handle_start_tag(*name, attributes, *self_closing, atoms, text_resolver)?,
            Token::EndTag(name) => self.handle_end_tag(*name, atoms)?,
        }

        #[cfg(debug_assertions)]
        self.debug_assert_invariants();

        Ok(())
    }

    pub fn finish(&mut self) -> TreeBuilderResult<()> {
        if self.finished {
            return Err(TreeBuilderError::Finished);
        }
        self.finalize_pending_text()?;
        debug_assert!(
            self.pending_text.is_none(),
            "pending text must be flushed on finish"
        );
        // Allow implicit closes at EOF for still-open elements.
        self.open_elements.clear();
        if !self.document_emitted {
            self.ensure_document_emitted()?;
        }
        self.finished = true;
        Ok(())
    }

    /// Materialize the internal arena into a fully owned `Node` tree.
    ///
    /// This is an explicitly expensive operation intended for debug/export paths,
    /// not incremental preview ticks. Prefer patches and arena indices for hot paths.
    pub fn materialize(self) -> TreeBuilderResult<Node> {
        if !self.finished {
            return Err(TreeBuilderError::InvariantViolation(
                "TreeBuilder::finish() must be called before materialize()",
            ));
        }
        #[cfg(feature = "parse-guards")]
        crate::parse_guards::record_dom_materialize();
        Ok(self.arena.materialize(self.root_index))
    }

    /// Drain any patches emitted so far.
    ///
    /// Call this before consuming the builder with `materialize()`.
    pub fn take_patches(&mut self) -> Vec<DomPatch> {
        std::mem::take(&mut self.patches)
    }

    /// Finish parsing and return both the owned DOM and all emitted patches.
    ///
    /// This performs a full materialization; avoid calling it on hot paths.
    pub fn finish_into_owned_dom_and_patches(mut self) -> TreeBuilderResult<(Node, Vec<DomPatch>)> {
        self.finish()?;
        let patches = self.take_patches();
        let dom = self.materialize()?;
        Ok((dom, patches))
    }

    pub fn push_stream_token(
        &mut self,
        token: &Token,
        stream: &TokenStream,
    ) -> TreeBuilderResult<()> {
        self.push_token(token, stream.atoms(), stream)
    }

    pub fn push_stream(&mut self, stream: &TokenStream) -> TreeBuilderResult<()> {
        let atoms = stream.atoms();
        for token in stream.tokens() {
            self.push_token(token, atoms, stream)?;
        }
        Ok(())
    }

    #[inline]
    pub(super) fn current_parent(&self) -> usize {
        self.open_elements
            .last()
            .copied()
            .unwrap_or(self.root_index)
    }

    fn handle_doctype(&mut self, doctype: &str) -> TreeBuilderResult<()> {
        self.finalize_pending_text()?;
        if self.document_emitted {
            return Err(TreeBuilderError::Protocol(
                "doctype after document emission",
            ));
        }
        if self.arena.doctype(self.root_index).is_some() {
            return Err(TreeBuilderError::Protocol("duplicate doctype"));
        }
        self.arena.set_doctype(self.root_index, doctype.to_string());
        Ok(())
    }

    fn handle_comment(&mut self, comment: &str) -> TreeBuilderResult<()> {
        self.ensure_document_emitted()?;
        self.finalize_pending_text()?;
        let parent_index = self.current_parent();
        let key = self.arena.alloc_key();
        let text = comment.to_string();
        self.arena.add_child(
            parent_index,
            ArenaNode::Comment {
                key,
                text: text.clone(),
            },
        );
        self.emit_patch(DomPatch::CreateComment {
            key: patch_key(key),
            text: text.clone(),
        })?;
        self.emit_patch(DomPatch::AppendChild {
            parent: patch_key(self.arena.node_key(parent_index)),
            child: patch_key(key),
        })?;
        Ok(())
    }

    fn handle_text<R: TokenTextResolver + ?Sized>(
        &mut self,
        token: &Token,
        text_resolver: &R,
    ) -> TreeBuilderResult<()> {
        self.ensure_document_emitted()?;
        if let Some(txt) = text_resolver.text(token) {
            self.push_text(txt)?;
        }
        Ok(())
    }

    fn handle_start_tag<R: TokenTextResolver + ?Sized>(
        &mut self,
        name: crate::types::AtomId,
        attributes: &[(crate::types::AtomId, Option<crate::types::AttributeValue>)],
        self_closing: bool,
        atoms: &AtomTable,
        text_resolver: &R,
    ) -> TreeBuilderResult<()> {
        self.ensure_document_emitted()?;
        self.finalize_pending_text()?;
        let parent_index = self.current_parent();
        // Materialize attribute values into owned DOM strings; revisit once
        // attribute storage is arena-backed to reduce cloning.
        let mut resolved_attributes = Vec::with_capacity(attributes.len());
        let mut patch_attributes = Vec::with_capacity(attributes.len());
        for (k, v) in attributes {
            let attr_name = atoms.resolve_arc(*k);
            let resolved_value = v
                .as_ref()
                .map(|value| value.as_str(text_resolver.source()).to_string());
            patch_attributes.push((Arc::clone(&attr_name), resolved_value.clone()));
            resolved_attributes.push((attr_name, resolved_value));
        }
        let resolved_name = atoms.resolve_arc(name);
        debug_assert_lowercase_atom(resolved_name.as_ref(), "dom builder tag atom");
        #[cfg(debug_assertions)]
        for (k, _) in &resolved_attributes {
            debug_assert_lowercase_atom(k.as_ref(), "dom builder attribute atom");
        }
        let key = self.arena.alloc_key();
        let patch_name = Arc::clone(&resolved_name);
        let new_index = self.arena.add_child(
            parent_index,
            ArenaNode::Element {
                key,
                name: resolved_name,
                attributes: resolved_attributes,
                children: Vec::new(),
                style: Vec::new(),
            },
        );
        self.emit_patch(DomPatch::CreateElement {
            key: patch_key(key),
            name: patch_name,
            attributes: patch_attributes,
        })?;
        self.emit_patch(DomPatch::AppendChild {
            parent: patch_key(self.arena.node_key(parent_index)),
            child: patch_key(key),
        })?;

        if !self_closing {
            self.open_elements.push(new_index);
        }
        Ok(())
    }

    fn handle_end_tag(
        &mut self,
        name: crate::types::AtomId,
        atoms: &AtomTable,
    ) -> TreeBuilderResult<()> {
        self.ensure_document_emitted()?;
        self.finalize_pending_text()?;
        // End tags only affect tree builder state; they do not emit patches.
        let target = atoms.resolve(name);
        debug_assert_lowercase_atom(target, "dom builder end-tag atom");
        while let Some(open_index) = self.open_elements.pop() {
            debug_assert!(open_index != self.root_index);
            if self.arena.is_element_named(open_index, target) {
                break;
            }
        }
        Ok(())
    }
}

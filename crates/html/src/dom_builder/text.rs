use crate::dom_patch::DomPatch;
use crate::types::NodeKey;

use super::arena::ArenaNode;
use super::patches::patch_key;
use super::{TreeBuilder, TreeBuilderResult};

#[derive(Debug)]
pub(super) struct PendingText {
    pub(super) parent_index: usize,
    pub(super) node_index: usize,
    pub(super) key: NodeKey,
    pub(super) text: String,
    pub(super) dirty: bool,
}

impl TreeBuilder {
    pub(super) fn push_text(&mut self, text: &str) -> TreeBuilderResult<()> {
        if text.is_empty() {
            return Ok(());
        }

        if !self.coalesce_text {
            let parent_index = self.current_parent();
            let _ = self.add_text_node(parent_index, text.to_string())?;
            return Ok(());
        }

        let parent_index = self.current_parent();
        if let Some(pending) = &mut self.pending_text {
            if pending.parent_index == parent_index {
                pending.text.push_str(text);
                // dirty means the final text differs from the initial CreateText payload.
                pending.dirty = true;
                return Ok(());
            }
            self.finalize_pending_text()?;
        }

        if let Some((node_index, key, text_owned)) =
            self.add_text_node_and_return(parent_index, text.to_string())?
        {
            self.pending_text = Some(PendingText {
                parent_index,
                node_index,
                key,
                text: text_owned,
                dirty: false,
            });
        }
        Ok(())
    }

    pub(super) fn finalize_pending_text(&mut self) -> TreeBuilderResult<()> {
        let Some(pending) = self.pending_text.take() else {
            return Ok(());
        };
        if pending.dirty {
            self.emit_patch(DomPatch::SetText {
                key: patch_key(pending.key),
                text: pending.text.clone(),
            })?;
        }
        self.arena.set_text(pending.node_index, pending.text);
        Ok(())
    }

    fn add_text_node(
        &mut self,
        parent_index: usize,
        text: String,
    ) -> TreeBuilderResult<Option<(usize, NodeKey)>> {
        self.add_text_node_internal(parent_index, text)
            .map(|opt| opt.map(|(idx, key, _)| (idx, key)))
    }

    fn add_text_node_and_return(
        &mut self,
        parent_index: usize,
        text: String,
    ) -> TreeBuilderResult<Option<(usize, NodeKey, String)>> {
        self.add_text_node_internal(parent_index, text)
    }

    fn add_text_node_internal(
        &mut self,
        parent_index: usize,
        text: String,
    ) -> TreeBuilderResult<Option<(usize, NodeKey, String)>> {
        if text.is_empty() {
            return Ok(None);
        }
        // Arena text starts with the initial payload and is updated on flush boundaries.
        let text_out = text.clone();
        let key = self.arena.alloc_key();
        let node_index = self
            .arena
            .add_child(parent_index, ArenaNode::Text { key, text });
        self.emit_patch(DomPatch::CreateText {
            key: patch_key(key),
            text: text_out.clone(),
        })?;
        self.emit_patch(DomPatch::AppendChild {
            parent: patch_key(self.arena.node_key(parent_index)),
            child: patch_key(key),
        })?;
        Ok(Some((node_index, key, text_out)))
    }
}

use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, EngineInvariantError};
use crate::html5::tokenizer::is_html_space;
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::OpenElement;

/// Pending character-token run for the HTML5 `In table text` buffering
/// algorithm.
///
/// The buffer stores owned chunks so chunked tokenizer spans can be merged into
/// one logical table-text run without borrowing tokenizer storage past the
/// current token boundary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct PendingTableCharacterTokens {
    chunks: Vec<String>,
    contains_non_space: bool,
}

impl PendingTableCharacterTokens {
    #[inline]
    pub(in crate::html5::tree_builder) fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn chunks(&self) -> &[String] {
        &self.chunks
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn contains_non_space(&self) -> bool {
        self.contains_non_space
    }

    pub(in crate::html5::tree_builder) fn push_str(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.contains_non_space |= chunk.chars().any(|ch| !is_html_space(ch));
        self.chunks.push(chunk.to_string());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct PendingTableTextState {
    original_insertion_mode: InsertionMode,
    tokens: PendingTableCharacterTokens,
}

impl PendingTableTextState {
    fn new(original_insertion_mode: InsertionMode) -> Self {
        Self {
            original_insertion_mode,
            tokens: PendingTableCharacterTokens::default(),
        }
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn original_insertion_mode(&self) -> InsertionMode {
        self.original_insertion_mode
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn tokens(&self) -> &PendingTableCharacterTokens {
        &self.tokens
    }
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn current_table_key(&self) -> Option<PatchKey> {
        self.open_elements
            .find_last_by_name(self.known_tags.table)
            .map(OpenElement::key)
    }

    pub(in crate::html5::tree_builder) fn buffer_pending_table_character_tokens(
        &mut self,
        resolved: &str,
    ) -> Result<(), TreeBuilderError> {
        let Some(state) = self.pending_table_text.as_mut() else {
            return Err(EngineInvariantError);
        };
        state.tokens.push_str(resolved);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn clear_pending_table_text_state(&mut self) {
        self.pending_table_text = None;
    }

    pub(in crate::html5::tree_builder) fn enter_in_table_text_mode(
        &mut self,
        original_mode: InsertionMode,
    ) -> Result<(), TreeBuilderError> {
        if self.pending_table_text.is_some() {
            return Err(EngineInvariantError);
        }
        if !matches!(
            original_mode,
            InsertionMode::InTable | InsertionMode::InTableBody | InsertionMode::InRow
        ) {
            return Err(EngineInvariantError);
        }
        self.pending_table_text = Some(PendingTableTextState::new(original_mode));
        self.insertion_mode = InsertionMode::InTableText;
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn take_pending_table_text_state(
        &mut self,
    ) -> Result<PendingTableTextState, TreeBuilderError> {
        if self.insertion_mode != InsertionMode::InTableText {
            return Err(EngineInvariantError);
        }
        let Some(state) = self.pending_table_text.take() else {
            return Err(EngineInvariantError);
        };
        Ok(state)
    }

    pub(super) fn current_node_name(&self) -> Option<AtomId> {
        self.open_elements.current().map(OpenElement::name)
    }

    pub(super) fn current_node_uses_in_table_text_mode(&self) -> bool {
        let Some(current) = self.open_elements.current() else {
            return false;
        };
        let name = current.name();
        name == self.known_tags.table
            || name == self.known_tags.tbody
            || name == self.known_tags.tfoot
            || name == self.known_tags.thead
            || name == self.known_tags.tr
    }
}

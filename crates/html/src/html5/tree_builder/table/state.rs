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
            .find_last_html_by_name(self.known_tags.table)
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

    pub(in crate::html5::tree_builder) fn is_table_foster_target_name(&self, name: AtomId) -> bool {
        name == self.known_tags.table
            || name == self.known_tags.tbody
            || name == self.known_tags.tfoot
            || name == self.known_tags.thead
            || name == self.known_tags.tr
    }

    pub(in crate::html5::tree_builder) fn current_node_is_table_foster_target(&self) -> bool {
        self.open_elements.current().is_some_and(|current| {
            current.namespace() == crate::ElementNamespace::Html
                && self.is_table_foster_target_name(current.name())
        })
    }

    pub(super) fn current_node_uses_in_table_text_mode(&self) -> bool {
        self.current_node_is_table_foster_target()
    }

    /// Restores the insertion mode represented by the supported full-document
    /// SOE subset after closing a nested table. This is stack-derived state,
    /// not a remembered return mode.
    pub(in crate::html5::tree_builder) fn reset_supported_insertion_mode_from_soe(
        &mut self,
    ) -> Result<InsertionMode, TreeBuilderError> {
        self.perf_reset_insertion_mode_scan_calls =
            self.perf_reset_insertion_mode_scan_calls.saturating_add(1);
        for index in (0..self.open_elements.len()).rev() {
            self.perf_reset_insertion_mode_scan_steps =
                self.perf_reset_insertion_mode_scan_steps.saturating_add(1);
            let entry = self.open_elements.get(index).ok_or(EngineInvariantError)?;
            if entry.namespace() != crate::ElementNamespace::Html {
                continue;
            }
            let name = entry.name();
            let mode = if name == self.known_tags.td || name == self.known_tags.th {
                Some(InsertionMode::InCell)
            } else if name == self.known_tags.tr {
                Some(InsertionMode::InRow)
            } else if name == self.known_tags.tbody
                || name == self.known_tags.thead
                || name == self.known_tags.tfoot
            {
                Some(InsertionMode::InTableBody)
            } else if name == self.known_tags.caption {
                Some(InsertionMode::InCaption)
            } else if name == self.known_tags.colgroup {
                Some(InsertionMode::InColumnGroup)
            } else if name == self.known_tags.table {
                Some(InsertionMode::InTable)
            } else if name == self.known_tags.template {
                let current = self.template_modes.current().ok_or(EngineInvariantError)?;
                if current.owner() != entry.key() {
                    return Err(EngineInvariantError);
                }
                Some(current.mode().as_insertion_mode())
            } else if name == self.known_tags.head {
                Some(InsertionMode::InHead)
            } else if name == self.known_tags.body {
                Some(InsertionMode::InBody)
            } else if name == self.known_tags.html {
                Some(if self.head_element_pointer.is_some() {
                    InsertionMode::AfterHead
                } else {
                    InsertionMode::BeforeHead
                })
            } else {
                None
            };
            if let Some(mode) = mode {
                self.insertion_mode = mode;
                return Ok(mode);
            }
        }
        Err(EngineInvariantError)
    }
}

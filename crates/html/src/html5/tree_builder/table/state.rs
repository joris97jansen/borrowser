use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;
use crate::html5::tokenizer::is_html_space;
use crate::html5::tree_builder::Html5TreeBuilder;
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

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn current_table_key(&self) -> Option<PatchKey> {
        self.open_elements
            .find_last_by_name(self.known_tags.table)
            .map(OpenElement::key)
    }

    pub(in crate::html5::tree_builder) fn buffer_pending_table_character_tokens(
        &mut self,
        resolved: &str,
    ) {
        self.pending_table_character_tokens.push_str(resolved);
    }

    pub(in crate::html5::tree_builder) fn clear_pending_table_character_tokens(&mut self) {
        self.pending_table_character_tokens = PendingTableCharacterTokens::default();
    }

    pub(in crate::html5::tree_builder) fn take_pending_table_character_tokens(
        &mut self,
    ) -> PendingTableCharacterTokens {
        std::mem::take(&mut self.pending_table_character_tokens)
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

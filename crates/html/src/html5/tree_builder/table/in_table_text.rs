use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::resolve_text_value;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_table_text(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Text { text: token_text } => {
                let resolved = resolve_text_value(token_text, text)?;
                self.buffer_pending_table_character_tokens(&resolved);
                Ok(DispatchOutcome::Done)
            }
            _ => {
                self.flush_pending_table_character_tokens(atoms, text)?;
                self.insertion_mode = InsertionMode::InTable;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
        }
    }
}

use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError, TreeBuilderStepResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum DispatchOutcome {
    Done,
    Reprocess(InsertionMode),
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn process_impl(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        self.assert_atom_table_binding(atoms);
        debug_assert!(self.pending_tokenizer_control.is_none());
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
                InsertionMode::InTable => self.handle_in_table(token, atoms, text)?,
                InsertionMode::InTableText => self.handle_in_table_text(token, atoms, text)?,
                InsertionMode::InCaption => self.handle_in_caption(token, atoms, text)?,
                InsertionMode::InColumnGroup => self.handle_in_column_group(token, atoms, text)?,
                InsertionMode::InTableBody => self.handle_in_table_body(token, atoms, text)?,
                InsertionMode::InRow => self.handle_in_row(token, atoms, text)?,
                InsertionMode::InCell => self.handle_in_cell(token, atoms, text)?,
                InsertionMode::Text => self.handle_text_mode(token, atoms, text)?,
            };
            match outcome {
                DispatchOutcome::Done => {
                    handled = true;
                    last_successful_mode = self.insertion_mode;
                    break;
                }
                DispatchOutcome::Reprocess(next_mode) => mode = next_mode,
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
        self.perf_soe_push_ops = self.open_elements.push_ops();
        self.perf_soe_pop_ops = self.open_elements.pop_ops();
        self.perf_soe_scope_scan_calls = self.open_elements.scope_scan_calls();
        self.perf_soe_scope_scan_steps = self.open_elements.scope_scan_steps();
        Ok(TreeBuilderStepResult::continue_with(
            self.pending_tokenizer_control.take(),
        ))
    }
}

#![allow(
    dead_code,
    reason = "table-mode state plumbing lands incrementally across Milestone I"
)]

use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, TextValue, Token};
use crate::html5::tokenizer::{TextResolver, is_html_space};
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{is_html_whitespace_str, resolve_text_value};
use crate::html5::tree_builder::stack::{OpenElement, ScopeKind};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

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
    fn is_table_body_section(&self, name: AtomId) -> bool {
        name == self.known_tags.tbody
            || name == self.known_tags.thead
            || name == self.known_tags.tfoot
    }

    fn current_table_body_section_name(&self) -> Option<AtomId> {
        let current = self.current_node_name()?;
        self.is_table_body_section(current).then_some(current)
    }

    fn has_any_table_body_section_in_table_scope(&mut self) -> bool {
        self.has_in_table_scope(self.known_tags.tbody)
            || self.has_in_table_scope(self.known_tags.thead)
            || self.has_in_table_scope(self.known_tags.tfoot)
    }

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

    pub(in crate::html5::tree_builder) fn has_in_table_scope(&mut self, name: AtomId) -> bool {
        self.open_elements
            .has_in_scope(name, ScopeKind::Table, &self.scope_tags)
    }

    fn current_table_cell_in_scope(&mut self) -> Option<OpenElement> {
        self.open_elements.find_last_table_cell_in_scope(
            self.known_tags.td,
            self.known_tags.th,
            &self.scope_tags,
        )
    }

    pub(in crate::html5::tree_builder) fn clear_stack_to_table_context(&mut self) -> usize {
        let removed = self.open_elements.clear_to_table_context(&self.scope_tags);
        if removed > 0 {
            self.invalidate_text_coalescing();
        }
        removed
    }

    fn clear_stack_to_table_body_context(&mut self) -> usize {
        let removed = self.open_elements.clear_to_table_body_context(
            self.known_tags.tbody,
            self.known_tags.thead,
            self.known_tags.tfoot,
            &self.scope_tags,
        );
        if removed > 0 {
            self.invalidate_text_coalescing();
        }
        removed
    }

    fn clear_stack_to_table_row_context(&mut self) -> usize {
        let removed = self
            .open_elements
            .clear_to_table_row_context(self.known_tags.tr, &self.scope_tags);
        if removed > 0 {
            self.invalidate_text_coalescing();
        }
        removed
    }

    pub(in crate::html5::tree_builder) fn close_cell(&mut self) -> bool {
        let Some(cell) = self.current_table_cell_in_scope() else {
            self.record_parse_error(
                "close-cell-no-cell-in-table-scope",
                None,
                Some(self.insertion_mode),
            );
            return false;
        };
        let closed = self.close_element_in_scope(cell.name(), ScopeKind::Table);
        if !closed {
            return false;
        }
        let _ = self.active_formatting.clear_to_last_marker();
        self.insertion_mode = InsertionMode::InRow;
        true
    }

    fn close_current_table_body_section(&mut self) -> bool {
        self.clear_stack_to_table_body_context();
        let Some(section) = self.current_table_body_section_name() else {
            self.record_parse_error(
                "close-table-body-no-open-section",
                None,
                Some(InsertionMode::InTableBody),
            );
            return false;
        };
        let closed = self.close_element_in_scope(section, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    fn close_table_body_section_named(&mut self, name: AtomId) -> bool {
        if !self.has_in_table_scope(name) {
            self.record_parse_error(
                "table-body-end-tag-not-in-table-scope",
                Some(name),
                Some(InsertionMode::InTableBody),
            );
            return false;
        }
        self.clear_stack_to_table_body_context();
        if self.current_node_name() != Some(name) {
            self.record_parse_error(
                "table-body-close-current-node-mismatch",
                Some(name),
                Some(InsertionMode::InTableBody),
            );
        }
        let closed = self.close_element_in_scope(name, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    fn close_row(&mut self) -> bool {
        if !self.has_in_table_scope(self.known_tags.tr) {
            self.record_parse_error(
                "tr-end-tag-not-in-table-scope",
                Some(self.known_tags.tr),
                Some(InsertionMode::InRow),
            );
            return false;
        }
        self.clear_stack_to_table_row_context();
        if self.current_node_name() != Some(self.known_tags.tr) {
            self.record_parse_error(
                "tr-close-current-node-mismatch",
                Some(self.known_tags.tr),
                Some(InsertionMode::InRow),
            );
        }
        let closed = self.close_element_in_scope(self.known_tags.tr, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTableBody;
        true
    }

    fn handle_unimplemented_table_mode(&mut self, mode: InsertionMode) -> DispatchOutcome {
        // Milestone I state plumbing lands before real table-mode algorithms.
        // Keep the fallback explicit, parse-error marked, and easy to delete so
        // placeholder dispatch cannot be mistaken for supported table parsing.
        self.record_parse_error("table-mode-not-yet-implemented", None, Some(mode));
        self.insertion_mode = InsertionMode::InBody;
        DispatchOutcome::Reprocess(InsertionMode::InBody)
    }

    fn current_node_name(&self) -> Option<AtomId> {
        self.open_elements.current().map(OpenElement::name)
    }

    fn current_node_uses_in_table_text_mode(&self) -> bool {
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

    fn process_using_in_body_rules(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
        foster_parenting_enabled: bool,
    ) -> Result<(), TreeBuilderError> {
        let saved_mode = self.insertion_mode;
        let saved_foster_parenting = self.foster_parenting_enabled;
        self.foster_parenting_enabled = foster_parenting_enabled;
        let result = self.handle_in_body(token, atoms, text);
        self.foster_parenting_enabled = saved_foster_parenting;
        if !matches!(self.insertion_mode, InsertionMode::Text) {
            self.insertion_mode = saved_mode;
        }
        result.map(|_| ())
    }

    fn close_caption(&mut self) -> bool {
        if !self.has_in_table_scope(self.known_tags.caption) {
            self.record_parse_error(
                "caption-end-tag-not-in-table-scope",
                Some(self.known_tags.caption),
                Some(InsertionMode::InCaption),
            );
            return false;
        }
        if self.current_node_name() != Some(self.known_tags.caption) {
            self.record_parse_error(
                "caption-close-current-node-mismatch",
                Some(self.known_tags.caption),
                Some(InsertionMode::InCaption),
            );
        }
        let closed = self.close_element_in_scope(self.known_tags.caption, ScopeKind::Table);
        if !closed {
            return false;
        }
        let _ = self.active_formatting.clear_to_last_marker();
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    fn close_column_group(&mut self) -> bool {
        if self.current_node_name() != Some(self.known_tags.colgroup) {
            return false;
        }
        let closed = self.close_element_in_scope(self.known_tags.colgroup, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    fn flush_pending_table_character_tokens(
        &mut self,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let pending = self.take_pending_table_character_tokens();
        if pending.is_empty() {
            return Ok(());
        }
        let mut merged = String::new();
        for chunk in pending.chunks() {
            merged.push_str(chunk);
        }
        if pending.contains_non_space() {
            self.record_parse_error(
                "in-table-text-non-space-foster-parented",
                None,
                Some(InsertionMode::InTableText),
            );
            self.process_using_in_body_rules(
                &Token::Text {
                    text: TextValue::Owned(merged),
                },
                atoms,
                text,
                true,
            )?;
        } else {
            self.insert_resolved_text(&merged)?;
        }
        Ok(())
    }

    fn handle_in_table_anything_else(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        self.record_parse_error(
            "in-table-anything-else-reprocess-in-body",
            None,
            Some(InsertionMode::InTable),
        );
        self.process_using_in_body_rules(token, atoms, text, true)?;
        Ok(DispatchOutcome::Done)
    }

    pub(in crate::html5::tree_builder) fn handle_in_table(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error("in-table-doctype", None, Some(InsertionMode::InTable));
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { .. } if self.current_node_uses_in_table_text_mode() => {
                self.clear_pending_table_character_tokens();
                self.insertion_mode = InsertionMode::InTableText;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableText))
            }
            Token::Text { .. } => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.caption => {
                self.clear_stack_to_table_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.active_formatting.push_marker();
                self.insertion_mode = InsertionMode::InCaption;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.colgroup => {
                self.clear_stack_to_table_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InColumnGroup;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.col => {
                self.clear_stack_to_table_context();
                let _ = self.insert_element(self.known_tags.colgroup, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InColumnGroup;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InColumnGroup))
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.tbody
                || *name == self.known_tags.thead
                || *name == self.known_tags.tfoot =>
            {
                self.clear_stack_to_table_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.tr => {
                self.clear_stack_to_table_context();
                let _ = self.insert_element(self.known_tags.tbody, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.record_parse_error(
                    "in-table-cell-start-tag-implies-row-group",
                    Some(*name),
                    Some(InsertionMode::InTable),
                );
                self.clear_stack_to_table_context();
                let _ = self.insert_element(self.known_tags.tbody, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. } if *name == self.known_tags.table => {
                self.record_parse_error(
                    "in-table-nested-table-start-tag",
                    Some(*name),
                    Some(InsertionMode::InTable),
                );
                if !self.has_in_table_scope(self.known_tags.table) {
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_element_in_scope(self.known_tags.table, ScopeKind::Table);
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.has_in_table_scope(*name) {
                    self.record_parse_error(
                        "in-table-table-end-tag-not-in-scope",
                        Some(*name),
                        Some(InsertionMode::InTable),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_element_in_scope(*name, ScopeKind::Table);
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.td
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.th
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                self.record_parse_error(
                    "in-table-unexpected-table-family-end-tag",
                    Some(*name),
                    Some(InsertionMode::InTable),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table_anything_else(token, atoms, text),
        }
    }

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

    pub(in crate::html5::tree_builder) fn handle_in_caption(
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
                self.record_parse_error("in-caption-doctype", None, Some(InsertionMode::InCaption));
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.caption => {
                let _ = self.close_caption();
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.td
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.th
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if !self.close_caption() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.close_caption() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.td
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.th
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                self.record_parse_error(
                    "in-caption-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InCaption),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => {
                // Caption contents intentionally use the normal InBody token
                // path until a caption/table-structure escape token closes the
                // caption and reprocesses in the outer table mode.
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_in_column_group(
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
                self.record_parse_error(
                    "in-column-group-doctype",
                    None,
                    Some(InsertionMode::InColumnGroup),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } => {
                let resolved = resolve_text_value(token_text, text)?;
                if is_html_whitespace_str(&resolved) {
                    self.insert_resolved_text(&resolved)?;
                    Ok(DispatchOutcome::Done)
                } else {
                    self.record_parse_error(
                        "in-column-group-non-space-text-closes-colgroup",
                        None,
                        Some(InsertionMode::InColumnGroup),
                    );
                    if !self.close_column_group() {
                        return Ok(DispatchOutcome::Done);
                    }
                    Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
                }
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.col => {
                let _ = self.insert_element(*name, attrs, true, atoms, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.colgroup => {
                if !self.close_column_group() {
                    self.record_parse_error(
                        "in-column-group-colgroup-end-tag-ignored",
                        Some(*name),
                        Some(InsertionMode::InColumnGroup),
                    );
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.col => {
                self.record_parse_error(
                    "in-column-group-col-end-tag-ignored",
                    Some(*name),
                    Some(InsertionMode::InColumnGroup),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                if self.current_node_name() != Some(self.known_tags.colgroup) {
                    let _ = self.ensure_document_created()?;
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_column_group();
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            _ => {
                self.record_parse_error(
                    "in-column-group-anything-else-closes-colgroup",
                    None,
                    Some(InsertionMode::InColumnGroup),
                );
                if !self.close_column_group() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_in_table_body(
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
                self.record_parse_error(
                    "in-table-body-doctype",
                    None,
                    Some(InsertionMode::InTableBody),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.tr => {
                self.clear_stack_to_table_body_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InRow;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.record_parse_error(
                    "in-table-body-cell-start-tag-implies-tr",
                    Some(*name),
                    Some(InsertionMode::InTableBody),
                );
                self.clear_stack_to_table_body_context();
                let _ = self.insert_element(self.known_tags.tr, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InRow;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                if !self.has_any_table_body_section_in_table_scope() {
                    self.record_parse_error(
                        "in-table-body-section-transition-without-open-section",
                        Some(*name),
                        Some(InsertionMode::InTableBody),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_current_table_body_section() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                let _ = self.close_table_body_section_named(*name);
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.has_any_table_body_section_in_table_scope() {
                    self.record_parse_error(
                        "in-table-body-table-end-tag-without-open-section",
                        Some(*name),
                        Some(InsertionMode::InTableBody),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_current_table_body_section() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.td
                    || *name == self.known_tags.th
                    || *name == self.known_tags.tr =>
            {
                self.record_parse_error(
                    "in-table-body-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InTableBody),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table(token, atoms, text),
        }
    }

    pub(in crate::html5::tree_builder) fn handle_in_row(
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
                self.record_parse_error("in-row-doctype", None, Some(InsertionMode::InRow));
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.td || *name == self.known_tags.th => {
                self.clear_stack_to_table_row_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.active_formatting.push_marker();
                self.insertion_mode = InsertionMode::InCell;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if !self.close_row() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::EndTag { name } if *name == self.known_tags.tr => {
                let _ = self.close_row();
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.table
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                if !self.has_in_table_scope(*name) {
                    self.record_parse_error(
                        "in-row-end-tag-not-in-table-scope",
                        Some(*name),
                        Some(InsertionMode::InRow),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_row() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.td
                    || *name == self.known_tags.th =>
            {
                self.record_parse_error(
                    "in-row-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InRow),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table(token, atoms, text),
        }
    }

    pub(in crate::html5::tree_builder) fn handle_in_cell(
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
                self.record_parse_error("in-cell-doctype", None, Some(InsertionMode::InCell));
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                let Some(open_cell) = self.current_table_cell_in_scope() else {
                    self.record_parse_error(
                        "in-cell-cell-end-tag-not-in-table-scope",
                        Some(*name),
                        Some(InsertionMode::InCell),
                    );
                    return Ok(DispatchOutcome::Done);
                };
                if open_cell.name() != *name {
                    self.record_parse_error(
                        "in-cell-cell-end-tag-open-cell-mismatch",
                        Some(*name),
                        Some(InsertionMode::InCell),
                    );
                }
                if self.current_node_name() != Some(*name) {
                    self.record_parse_error(
                        "in-cell-cell-end-tag-current-node-mismatch",
                        Some(*name),
                        Some(InsertionMode::InCell),
                    );
                }
                let _ = self.close_cell();
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.td
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.th
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if self.current_table_cell_in_scope().is_none() {
                    self.record_parse_error(
                        "in-cell-table-structure-start-tag-without-open-cell",
                        Some(*name),
                        Some(InsertionMode::InCell),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_cell() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::EndTag { name }
                if *name == self.known_tags.table
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if !self.has_in_table_scope(*name) {
                    self.record_parse_error(
                        "in-cell-table-structure-end-tag-not-in-table-scope",
                        Some(*name),
                        Some(InsertionMode::InCell),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_cell() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html =>
            {
                self.record_parse_error(
                    "in-cell-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InCell),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
        }
    }
}

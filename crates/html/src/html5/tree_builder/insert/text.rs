use crate::dom_patch::DomPatch;
use crate::html5::shared::TextValue;
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::coalescing::LastTextPatch;
use crate::html5::tree_builder::resolve::resolve_text_value;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn insert_text(
        &mut self,
        token_text: &TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let resolved = resolve_text_value(token_text, text)?;
        self.insert_resolved_text(&resolved)
    }

    pub(in crate::html5::tree_builder) fn insert_literal_text(
        &mut self,
        literal: &str,
    ) -> Result<(), TreeBuilderError> {
        self.insert_resolved_text(literal)
    }

    pub(in crate::html5::tree_builder) fn insert_recovery_literal_text(
        &mut self,
        literal: &str,
    ) -> Result<(), TreeBuilderError> {
        self.invalidate_text_coalescing();
        self.insert_literal_text(literal)?;
        self.invalidate_text_coalescing();
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn insert_resolved_text(
        &mut self,
        resolved: &str,
    ) -> Result<(), TreeBuilderError> {
        debug_assert_eq!(self.structural_mutation_depth, 0);
        if resolved.is_empty() {
            return Ok(());
        }
        let location = self.element_or_text_insertion_location()?;
        if self.config.coalesce_text
            && let Some(last) = self.last_text_patch.as_ref()
            && last.parent == location.parent
            && last.before == location.before
        {
            let key = self
                .last_text_patch
                .as_ref()
                .expect("coalescing state must remain present within branch")
                .text_key;
            self.push_patch(DomPatch::AppendText {
                key,
                text: resolved.to_string(),
            });
            self.perf_text_appends = self.perf_text_appends.saturating_add(1);
            return Ok(());
        }
        let key = self.with_structural_mutation(|this| {
            let key = this.alloc_patch_key()?;
            this.push_structural_patch(DomPatch::CreateText {
                key,
                text: resolved.to_string(),
            });
            this.insert_existing_child_at(location, key);
            this.perf_text_nodes_created = this.perf_text_nodes_created.saturating_add(1);
            Ok(key)
        })?;
        self.last_text_patch = if self.config.coalesce_text {
            Some(LastTextPatch {
                parent: location.parent,
                before: location.before,
                text_key: key,
            })
        } else {
            None
        };
        Ok(())
    }
}

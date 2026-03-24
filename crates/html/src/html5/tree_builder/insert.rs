use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable, Attribute, TextValue};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::coalescing::LastTextPatch;
use crate::html5::tree_builder::formatting::{AfeAttributeSnapshot, AfeElementEntry};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{
    resolve_atom_arc, resolve_attribute_value, resolve_text_value,
};
use crate::html5::tree_builder::stack::{OpenElement, ScopeKind};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct InsertionLocation {
    parent: PatchKey,
    before: Option<PatchKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FosterParentAnchor {
    Template(PatchKey),
    TableIndex(usize),
}

impl Html5TreeBuilder {
    fn find_last_table_and_template_indices(&self) -> (Option<usize>, Option<usize>) {
        let mut table_index = None;
        let mut template_index = None;
        for index in (0..self.open_elements.len()).rev() {
            let element = self
                .open_elements
                .get(index)
                .expect("open-elements index must remain in bounds during foster scan");
            if table_index.is_none() && element.name() == self.known_tags.table {
                table_index = Some(index);
            } else if template_index.is_none() && element.name() == self.known_tags.template {
                template_index = Some(index);
            }
            if table_index.is_some() && template_index.is_some() {
                break;
            }
        }
        (table_index, template_index)
    }

    fn find_last_open_element_index(&self, target: AtomId) -> Option<usize> {
        for index in (0..self.open_elements.len()).rev() {
            let element = self
                .open_elements
                .get(index)
                .expect("open-elements index must remain in bounds during foster scan");
            if element.name() == target {
                return Some(index);
            }
        }
        None
    }

    fn current_insertion_parent(&self) -> Result<PatchKey, TreeBuilderError> {
        let document_key = self
            .document_key
            .ok_or(crate::html5::shared::EngineInvariantError)?;
        Ok(self
            .open_elements
            .current()
            .map(OpenElement::key)
            .unwrap_or(document_key))
    }

    fn current_insertion_location(&self) -> Result<InsertionLocation, TreeBuilderError> {
        Ok(InsertionLocation {
            parent: self.current_insertion_parent()?,
            before: None,
        })
    }

    fn foster_parenting_anchor_from_soe(&self) -> Option<FosterParentAnchor> {
        let (last_table_index, last_template_index) = self.find_last_table_and_template_indices();
        if let (Some(table_index), Some(template_index)) = (last_table_index, last_template_index)
            && template_index > table_index
        {
            let template = self
                .open_elements
                .get(template_index)
                .expect("template index must remain valid while computing foster location");
            return Some(FosterParentAnchor::Template(template.key()));
        }
        last_table_index.map(FosterParentAnchor::TableIndex)
    }

    fn foster_parenting_location_for_table_anchor(&self, table_index: usize) -> InsertionLocation {
        let table = self
            .open_elements
            .get(table_index)
            .expect("table index must remain valid while computing foster location");
        if let Some(parent) = self.live_tree.parent(table.key()) {
            return InsertionLocation {
                parent,
                before: Some(table.key()),
            };
        }

        // Foster parenting only uses this detached-table branch when the table is
        // still represented on the SOE under a prior context element. A detached
        // foster table therefore must have a previous SOE entry available as the
        // append target.
        let foster_parent = table_index
            .checked_sub(1)
            .and_then(|index| self.open_elements.get(index))
            .expect("detached foster table must still have a previous SOE entry");
        InsertionLocation {
            parent: foster_parent.key(),
            before: None,
        }
    }

    fn foster_parenting_location_without_table(
        &self,
    ) -> Result<InsertionLocation, TreeBuilderError> {
        let Some(html_index) = self.find_last_open_element_index(self.known_tags.html) else {
            return self.current_insertion_location();
        };
        let html = self
            .open_elements
            .get(html_index)
            .expect("html index must remain valid while computing foster location");
        Ok(InsertionLocation {
            parent: html.key(),
            before: None,
        })
    }

    pub(in crate::html5::tree_builder) fn foster_parenting_insertion_location(
        &self,
    ) -> Result<InsertionLocation, TreeBuilderError> {
        match self.foster_parenting_anchor_from_soe() {
            Some(FosterParentAnchor::Template(parent)) => Ok(InsertionLocation {
                parent,
                before: None,
            }),
            Some(FosterParentAnchor::TableIndex(table_index)) => {
                Ok(self.foster_parenting_location_for_table_anchor(table_index))
            }
            None => self.foster_parenting_location_without_table(),
        }
    }

    fn element_or_text_insertion_location(&self) -> Result<InsertionLocation, TreeBuilderError> {
        if self.foster_parenting_enabled {
            self.foster_parenting_insertion_location()
        } else {
            self.current_insertion_location()
        }
    }

    fn insert_existing_child_at(&mut self, location: InsertionLocation, child: PatchKey) {
        match location.before {
            Some(before) => self.insert_existing_child_before(location.parent, child, before),
            None => self.append_existing_child(location.parent, child),
        }
    }

    pub(in crate::html5::tree_builder) fn insert_existing_child_using_foster_parenting_location(
        &mut self,
        child: PatchKey,
    ) -> Result<(), TreeBuilderError> {
        let location = self.foster_parenting_insertion_location()?;
        self.insert_existing_child_at(location, child);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn append_existing_child(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
    ) {
        self.push_structural_patch(DomPatch::AppendChild { parent, child });
    }

    #[allow(
        dead_code,
        reason = "kept for upcoming AAA parser integration and foster-parent insertion work"
    )]
    pub(in crate::html5::tree_builder) fn insert_existing_child_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) {
        self.push_structural_patch(DomPatch::InsertBefore {
            parent,
            child,
            before,
        });
    }

    pub(in crate::html5::tree_builder) fn create_detached_element(
        &mut self,
        name: AtomId,
        attrs: &[(std::sync::Arc<str>, Option<String>)],
        atoms: &AtomTable,
    ) -> Result<PatchKey, TreeBuilderError> {
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateElement {
            key,
            name: resolve_atom_arc(atoms, name)?,
            attributes: attrs.to_vec(),
        });
        Ok(key)
    }

    fn create_detached_element_from_token_attrs(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<PatchKey, TreeBuilderError> {
        let mut attributes = Vec::with_capacity(attrs.len());
        for attr in attrs {
            let attr_name = resolve_atom_arc(atoms, attr.name)?;
            let attr_value = resolve_attribute_value(attr, text)?;
            attributes.push((attr_name, attr_value));
        }
        self.create_detached_element(name, &attributes, atoms)
    }

    pub(in crate::html5::tree_builder) fn create_detached_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        atoms: &AtomTable,
    ) -> Result<PatchKey, TreeBuilderError> {
        let mut attributes = Vec::with_capacity(entry.attrs.len());
        for attr in &entry.attrs {
            let attr_name = resolve_atom_arc(atoms, attr.name)?;
            attributes.push((attr_name, attr.value.clone()));
        }
        self.create_detached_element(entry.name, &attributes, atoms)
    }

    pub(in crate::html5::tree_builder) fn insert_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<PatchKey, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let _ = this.ensure_document_created()?;
            let location = this.element_or_text_insertion_location()?;
            let key = this.create_detached_element_from_token_attrs(name, attrs, atoms, text)?;
            this.insert_existing_child_at(location, key);
            if !self_closing {
                this.open_elements.push(OpenElement::new(key, name));
            }
            Ok(key)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        atoms: &AtomTable,
    ) -> Result<PatchKey, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let _ = this.ensure_document_created()?;
            let location = this.element_or_text_insertion_location()?;
            let key = this.create_detached_element_from_afe_entry(entry, atoms)?;
            this.insert_existing_child_at(location, key);
            this.open_elements.push(OpenElement::new(key, entry.name));
            Ok(key)
        })
    }

    pub(in crate::html5::tree_builder) fn snapshot_afe_attributes(
        &self,
        attrs: &[Attribute],
        text: &dyn TextResolver,
    ) -> Result<Vec<AfeAttributeSnapshot>, TreeBuilderError> {
        let mut snapshot = Vec::with_capacity(attrs.len());
        for attr in attrs {
            snapshot.push(AfeAttributeSnapshot::new(
                attr.name,
                resolve_attribute_value(attr, text)?,
            ));
        }
        Ok(snapshot)
    }

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

    pub(in crate::html5::tree_builder) fn insert_comment(
        &mut self,
        token_text: &TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created()?;
            let parent = this
                .open_elements
                .current()
                .map(OpenElement::key)
                .unwrap_or(document_key);
            let key = this.alloc_patch_key()?;
            this.push_structural_patch(DomPatch::CreateComment {
                key,
                text: resolved,
            });
            this.push_structural_patch(DomPatch::AppendChild { parent, child: key });
            Ok(())
        })
    }

    pub(in crate::html5::tree_builder) fn close_element_in_scope(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
    ) -> bool {
        self.pop_element_in_scope_with_reporting(name, scope, true)
            .is_some()
    }

    #[inline]
    #[allow(
        dead_code,
        reason = "kept as a convenience wrapper while insertion-mode AFE/AAA integration expands"
    )]
    pub(in crate::html5::tree_builder) fn close_element_in_scope_with_reporting(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
        report_not_in_scope_error: bool,
    ) -> bool {
        self.pop_element_in_scope_with_reporting(name, scope, report_not_in_scope_error)
            .is_some()
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn pop_element_in_scope_with_reporting(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
        report_not_in_scope_error: bool,
    ) -> Option<OpenElement> {
        let popped = self
            .open_elements
            .pop_until_including_in_scope(name, scope, &self.scope_tags);
        if popped.is_none() {
            if report_not_in_scope_error {
                self.record_parse_error("end-tag-not-in-scope", Some(name), None);
            }
            return None;
        }
        self.invalidate_text_coalescing();
        popped
    }

    pub(in crate::html5::tree_builder) fn update_mode_for_start_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.html {
            InsertionMode::BeforeHead
        } else if name == self.known_tags.head {
            InsertionMode::InHead
        } else {
            InsertionMode::InBody
        };
    }

    pub(in crate::html5::tree_builder) fn update_mode_for_end_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.head {
            InsertionMode::AfterHead
        } else if name == self.known_tags.body {
            InsertionMode::InBody
        } else {
            self.insertion_mode
        };
    }

    pub(in crate::html5::tree_builder) fn scope_kind_for_in_body_end_tag(
        &self,
        name: AtomId,
    ) -> ScopeKind {
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
}

#[cfg(test)]
mod tests {
    use super::{Html5TreeBuilder, InsertionLocation};
    use crate::dom_patch::{DomPatch, PatchKey};
    use crate::html5::shared::DocumentParseContext;
    use crate::html5::tokenizer::{TextResolveError, TextResolver};
    use crate::html5::tree_builder::stack::OpenElement;

    struct EmptyResolver;

    impl TextResolver for EmptyResolver {
        fn resolve_span(
            &self,
            span: crate::html5::shared::TextSpan,
        ) -> Result<&str, TextResolveError> {
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    fn bootstrap_html_body(
        builder: &mut Html5TreeBuilder,
        ctx: &DocumentParseContext,
    ) -> (PatchKey, PatchKey) {
        builder
            .with_structural_mutation(|this| {
                let document = this.ensure_document_created()?;
                let html = this.create_detached_element(this.known_tags.html, &[], &ctx.atoms)?;
                this.append_existing_child(document, html);
                this.open_elements
                    .push(OpenElement::new(html, this.known_tags.html));

                let body = this.create_detached_element(this.known_tags.body, &[], &ctx.atoms)?;
                this.append_existing_child(html, body);
                this.open_elements
                    .push(OpenElement::new(body, this.known_tags.body));
                Ok((html, body))
            })
            .expect("bootstrap should remain recoverable")
    }

    fn attach_live_table(
        builder: &mut Html5TreeBuilder,
        ctx: &DocumentParseContext,
        body: PatchKey,
    ) -> PatchKey {
        builder
            .with_structural_mutation(|this| {
                let table = this.create_detached_element(this.known_tags.table, &[], &ctx.atoms)?;
                this.append_existing_child(body, table);
                this.open_elements
                    .push(OpenElement::new(table, this.known_tags.table));
                Ok(table)
            })
            .expect("live table attach should remain recoverable")
    }

    #[test]
    fn foster_parenting_location_uses_live_table_parent_and_before_key() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        let table = attach_live_table(&mut builder, &ctx, body);

        assert_eq!(
            builder
                .foster_parenting_insertion_location()
                .expect("foster location"),
            InsertionLocation {
                parent: body,
                before: Some(table),
            }
        );
    }

    #[test]
    fn foster_parenting_location_uses_previous_soe_entry_for_detached_table() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        builder
            .with_structural_mutation(|this| {
                let table = this.create_detached_element(this.known_tags.table, &[], &ctx.atoms)?;
                this.open_elements
                    .push(OpenElement::new(table, this.known_tags.table));
                assert_eq!(
                    this.foster_parenting_insertion_location()?,
                    InsertionLocation {
                        parent: body,
                        before: None,
                    }
                );
                Ok(())
            })
            .expect("detached foster-parent computation should remain recoverable");
    }

    #[test]
    fn foster_parenting_location_prefers_template_above_table() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        let _table = attach_live_table(&mut builder, &ctx, body);
        builder
            .with_structural_mutation(|this| {
                let template =
                    this.create_detached_element(this.known_tags.template, &[], &ctx.atoms)?;
                this.open_elements
                    .push(OpenElement::new(template, this.known_tags.template));
                assert_eq!(
                    this.foster_parenting_insertion_location()?,
                    InsertionLocation {
                        parent: template,
                        before: None,
                    }
                );
                Ok(())
            })
            .expect("template-preferred foster-parent computation should remain recoverable");
    }

    #[test]
    fn foster_parenting_text_insertion_uses_insert_before_for_live_table() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        let table = attach_live_table(&mut builder, &ctx, body);
        let _ = builder.drain_patches();
        builder.foster_parenting_enabled = true;

        builder
            .insert_literal_text("x")
            .expect("foster-parent text insertion should remain recoverable");
        let patches = builder.drain_patches();

        assert_eq!(
            patches,
            vec![
                DomPatch::CreateText {
                    key: PatchKey(5),
                    text: "x".to_string(),
                },
                DomPatch::InsertBefore {
                    parent: body,
                    child: PatchKey(5),
                    before: table,
                },
            ]
        );
    }

    #[test]
    fn foster_parenting_element_insertion_uses_insert_before_for_live_table() {
        let resolver = EmptyResolver;
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        let table = attach_live_table(&mut builder, &ctx, body);
        let _ = builder.drain_patches();
        builder.foster_parenting_enabled = true;
        let div = ctx.atoms.intern_ascii_folded("div").expect("atom");

        let inserted = builder
            .insert_element(div, &[], false, &ctx.atoms, &resolver)
            .expect("foster-parent element insertion should remain recoverable");
        let patches = builder.drain_patches();

        assert_eq!(inserted, PatchKey(5));
        assert_eq!(
            patches,
            vec![
                DomPatch::CreateElement {
                    key: PatchKey(5),
                    name: std::sync::Arc::from("div"),
                    attributes: Vec::new(),
                },
                DomPatch::InsertBefore {
                    parent: body,
                    child: PatchKey(5),
                    before: table,
                },
            ]
        );
    }

    #[test]
    fn foster_parenting_reparents_existing_nodes_with_insert_before_only() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        let table = attach_live_table(&mut builder, &ctx, body);
        let (container, child) = builder
            .with_structural_mutation(|this| {
                let div = this.create_detached_element(
                    ctx.atoms.intern_ascii_folded("div").expect("atom"),
                    &[],
                    &ctx.atoms,
                )?;
                this.append_existing_child(body, div);
                let span = this.create_detached_element(
                    ctx.atoms.intern_ascii_folded("span").expect("atom"),
                    &[],
                    &ctx.atoms,
                )?;
                this.append_existing_child(div, span);
                Ok((div, span))
            })
            .expect("existing child setup should remain recoverable");
        let _ = builder.drain_patches();

        builder
            .with_structural_mutation(|this| {
                this.insert_existing_child_using_foster_parenting_location(child)
            })
            .expect("existing child foster-parent move should remain recoverable");
        let patches = builder.drain_patches();

        assert_eq!(
            patches,
            vec![DomPatch::InsertBefore {
                parent: body,
                child,
                before: table,
            }]
        );
        assert!(
            !patches
                .iter()
                .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
            "foster-parent reparenting must use canonical InsertBefore move encoding"
        );
        assert_eq!(builder.live_tree.parent(child), Some(body));
        assert_eq!(
            builder.live_tree.children_snapshot(container),
            Vec::<PatchKey>::new()
        );
        assert_eq!(
            builder.live_tree.children_snapshot(body),
            vec![child, table, container]
        );
    }
}

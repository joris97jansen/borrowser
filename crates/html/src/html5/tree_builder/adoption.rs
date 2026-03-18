//! HTML5 adoption agency algorithm support.
//!
//! The H5 landing keeps the AAA engine isolated from `InBody` dispatch so the
//! algorithm can be tested directly before the parser switches supported
//! formatting end tags over to it in the follow-up integration issue.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "AAA dispatch integration lands after the core engine"
    )
)]

use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable};
use crate::html5::tree_builder::formatting::AfeElementEntry;
use crate::html5::tree_builder::resolve::resolve_atom;
use crate::html5::tree_builder::stack::{OpenElement, ScopeKeyMatch};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

const ADOPTION_AGENCY_OUTER_LOOP_LIMIT: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum AdoptionAgencyOutcome {
    FallbackToGenericEndTag,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct AdoptionAgencyRunReport {
    pub(in crate::html5::tree_builder) outcome: AdoptionAgencyOutcome,
    pub(in crate::html5::tree_builder) outer_iterations: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct FormattingElementCandidate {
    pub(in crate::html5::tree_builder) afe_index: usize,
    pub(in crate::html5::tree_builder) key: PatchKey,
    pub(in crate::html5::tree_builder) name: AtomId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum FormattingElementValidation {
    MissingFromSoe,
    NotInScope,
    Eligible {
        soe_index: usize,
        is_current_node: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct FurthestBlockCandidate {
    pub(in crate::html5::tree_builder) soe_index: usize,
    pub(in crate::html5::tree_builder) element: OpenElement,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn adoption_agency_lookup_formatting_element(
        &self,
        subject: AtomId,
    ) -> Option<FormattingElementCandidate> {
        let afe_index = self
            .active_formatting
            .find_last_index_by_name_after_last_marker(subject)?;
        let entry = self.active_formatting.element_at(afe_index)?;
        Some(FormattingElementCandidate {
            afe_index,
            key: entry.key,
            name: entry.name,
        })
    }

    pub(in crate::html5::tree_builder) fn adoption_agency_validate_formatting_element(
        &mut self,
        candidate: FormattingElementCandidate,
    ) -> FormattingElementValidation {
        match self.open_elements.classify_key_in_scope(
            candidate.key,
            crate::html5::tree_builder::stack::ScopeKind::InScope,
            &self.scope_tags,
        ) {
            ScopeKeyMatch::Missing => FormattingElementValidation::MissingFromSoe,
            ScopeKeyMatch::OutOfScope => FormattingElementValidation::NotInScope,
            ScopeKeyMatch::InScope(soe_index) => FormattingElementValidation::Eligible {
                soe_index,
                is_current_node: self
                    .open_elements
                    .current()
                    .is_some_and(|current| current.key() == candidate.key),
            },
        }
    }

    pub(in crate::html5::tree_builder) fn adoption_agency_find_furthest_block(
        &self,
        formatting_soe_index: usize,
        atoms: &AtomTable,
    ) -> Result<Option<FurthestBlockCandidate>, TreeBuilderError> {
        if formatting_soe_index + 1 >= self.open_elements.len() {
            return Ok(None);
        }
        for soe_index in ((formatting_soe_index + 1)..self.open_elements.len()).rev() {
            let element = self
                .open_elements
                .get(soe_index)
                .expect("furthest-block scan index must remain in bounds");
            if is_special_html_tag(element.name(), atoms)? {
                return Ok(Some(FurthestBlockCandidate { soe_index, element }));
            }
        }
        Ok(None)
    }

    pub(in crate::html5::tree_builder) fn adoption_agency_common_ancestor(
        &self,
        formatting_soe_index: usize,
    ) -> Option<OpenElement> {
        formatting_soe_index
            .checked_sub(1)
            .and_then(|index| self.open_elements.get(index))
    }

    pub(in crate::html5::tree_builder) fn run_adoption_agency_algorithm(
        &mut self,
        subject: AtomId,
        atoms: &AtomTable,
    ) -> Result<AdoptionAgencyRunReport, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let mut outer_iterations = 0u8;
            loop {
                if outer_iterations >= ADOPTION_AGENCY_OUTER_LOOP_LIMIT {
                    return Ok(AdoptionAgencyRunReport {
                        outcome: AdoptionAgencyOutcome::Completed,
                        outer_iterations,
                    });
                }
                outer_iterations += 1;

                let no_active_formatting_match = this
                    .adoption_agency_lookup_formatting_element(subject)
                    .is_none();
                if no_active_formatting_match
                    && this
                        .open_elements
                        .current()
                        .is_some_and(|current| current.name() == subject)
                {
                    let popped = this
                        .open_elements
                        .pop()
                        .expect("current-node AAA shortcut requires a current node");
                    debug_assert_eq!(popped.name(), subject);
                    return Ok(AdoptionAgencyRunReport {
                        outcome: AdoptionAgencyOutcome::Completed,
                        outer_iterations,
                    });
                }

                let Some(candidate) = this.adoption_agency_lookup_formatting_element(subject)
                else {
                    return Ok(AdoptionAgencyRunReport {
                        outcome: AdoptionAgencyOutcome::FallbackToGenericEndTag,
                        outer_iterations,
                    });
                };
                let formatting_entry = this
                    .active_formatting
                    .element_at(candidate.afe_index)
                    .expect("AAA formatting element lookup must target an AFE element")
                    .clone();

                match this.adoption_agency_validate_formatting_element(candidate) {
                    FormattingElementValidation::MissingFromSoe => {
                        this.record_parse_error(
                            "adoption-agency-formatting-element-missing-from-soe",
                            Some(subject),
                            Some(this.insertion_mode),
                        );
                        let _ = this
                            .active_formatting
                            .remove_element_at(candidate.afe_index);
                        return Ok(AdoptionAgencyRunReport {
                            outcome: AdoptionAgencyOutcome::Completed,
                            outer_iterations,
                        });
                    }
                    FormattingElementValidation::NotInScope => {
                        this.record_parse_error(
                            "adoption-agency-formatting-element-not-in-scope",
                            Some(subject),
                            Some(this.insertion_mode),
                        );
                        return Ok(AdoptionAgencyRunReport {
                            outcome: AdoptionAgencyOutcome::Completed,
                            outer_iterations,
                        });
                    }
                    FormattingElementValidation::Eligible {
                        soe_index,
                        is_current_node,
                    } => {
                        if !is_current_node {
                            this.record_parse_error(
                                "adoption-agency-formatting-element-not-current-node",
                                Some(subject),
                                Some(this.insertion_mode),
                            );
                        }

                        let Some(furthest_block) =
                            this.adoption_agency_find_furthest_block(soe_index, atoms)?
                        else {
                            while this
                                .open_elements
                                .current()
                                .is_some_and(|current| current.key() != formatting_entry.key)
                            {
                                let _ = this.open_elements.pop();
                            }
                            let popped = this.open_elements.pop().expect(
                                "AAA no-furthest-block path must pop the formatting element",
                            );
                            debug_assert_eq!(popped.key(), formatting_entry.key);
                            let _ = this
                                .active_formatting
                                .remove_element_at(candidate.afe_index);
                            return Ok(AdoptionAgencyRunReport {
                                outcome: AdoptionAgencyOutcome::Completed,
                                outer_iterations,
                            });
                        };

                        let common_ancestor = this
                            .adoption_agency_common_ancestor(soe_index)
                            .expect("AAA furthest-block path requires a common ancestor");
                        let furthest_block_key = furthest_block.element.key();
                        let mut bookmark = candidate.afe_index;
                        let mut node_index = furthest_block.soe_index;
                        let mut last_node = furthest_block_key;
                        let mut inner_iterations = 0usize;

                        while node_index > 0 {
                            inner_iterations += 1;
                            node_index -= 1;
                            let node = this
                                .open_elements
                                .get(node_index)
                                .expect("AAA inner-loop scan index must remain in bounds");
                            if node.key() == formatting_entry.key {
                                break;
                            }

                            let mut node_afe_index =
                                this.active_formatting.find_index_by_key(node.key());
                            if inner_iterations > 3
                                && let Some(index) = node_afe_index
                            {
                                let _ = this.active_formatting.remove_element_at(index);
                                adjust_bookmark_for_removed_index(&mut bookmark, index);
                                node_afe_index = None;
                            }

                            let Some(node_afe_index) = node_afe_index else {
                                let _ = this.open_elements.remove_at(node_index);
                                continue;
                            };

                            let node_entry = this
                                .active_formatting
                                .element_at(node_afe_index)
                                .expect("AAA inner-loop AFE lookup must target an element")
                                .clone();
                            let replacement_key =
                                this.create_detached_element_from_afe_entry(&node_entry, atoms)?;
                            let replacement_entry = AfeElementEntry::new(
                                replacement_key,
                                node_entry.name,
                                node_entry.attrs.clone(),
                            );
                            let _ = this
                                .active_formatting
                                .replace_element_at(node_afe_index, replacement_entry);
                            let _ = this.open_elements.replace_at(
                                node_index,
                                OpenElement::new(replacement_key, node_entry.name),
                            );

                            if last_node == furthest_block_key {
                                bookmark = node_afe_index + 1;
                            }
                            this.append_existing_child(replacement_key, last_node);
                            last_node = replacement_key;
                        }

                        this.adoption_agency_insert_last_node(common_ancestor, last_node)?;

                        let replacement_key =
                            this.create_detached_element_from_afe_entry(&formatting_entry, atoms)?;
                        let furthest_block_children =
                            this.live_tree.children_snapshot(furthest_block_key);
                        for child in furthest_block_children {
                            this.append_existing_child(replacement_key, child);
                        }
                        this.append_existing_child(furthest_block_key, replacement_key);

                        let _ = this
                            .active_formatting
                            .remove_element_at(candidate.afe_index);
                        adjust_bookmark_for_removed_index(&mut bookmark, candidate.afe_index);
                        let replacement_entry = AfeElementEntry::new(
                            replacement_key,
                            formatting_entry.name,
                            formatting_entry.attrs.clone(),
                        );
                        let bookmark = bookmark.min(this.active_formatting.len());
                        this.active_formatting
                            .insert_element_at(bookmark, replacement_entry);

                        let _ = this.open_elements.remove_at(soe_index);
                        let furthest_block_index = this
                            .open_elements
                            .find_index_by_key(furthest_block_key)
                            .expect("AAA furthest block must remain on SOE");
                        this.open_elements.insert_at(
                            furthest_block_index + 1,
                            OpenElement::new(replacement_key, formatting_entry.name),
                        );
                    }
                }
            }
        })
    }
}

fn adjust_bookmark_for_removed_index(bookmark: &mut usize, removed_index: usize) {
    if removed_index < *bookmark {
        *bookmark -= 1;
    }
}

fn requires_foster_parenting(builder: &Html5TreeBuilder, name: AtomId) -> bool {
    name == builder.known_tags.table
        || name == builder.known_tags.tbody
        || name == builder.known_tags.tfoot
        || name == builder.known_tags.thead
        || name == builder.known_tags.tr
}

impl Html5TreeBuilder {
    fn adoption_agency_insert_last_node(
        &mut self,
        common_ancestor: OpenElement,
        last_node: PatchKey,
    ) -> Result<(), TreeBuilderError> {
        if !requires_foster_parenting(self, common_ancestor.name()) {
            self.append_existing_child(common_ancestor.key(), last_node);
            return Ok(());
        }

        let last_table_index = find_last_open_element_index(self, self.known_tags.table);
        let last_template_index = find_last_open_element_index(self, self.known_tags.template);
        if let (Some(table_index), Some(template_index)) = (last_table_index, last_template_index)
            && template_index > table_index
        {
            let template = self
                .open_elements
                .get(template_index)
                .expect("template index must remain valid while computing foster location");
            self.append_existing_child(template.key(), last_node);
            return Ok(());
        }

        if let Some(table_index) = last_table_index {
            let table = self
                .open_elements
                .get(table_index)
                .expect("table index must remain valid while computing foster location");
            if let Some(parent) = self.live_tree.parent(table.key()) {
                self.insert_existing_child_before(parent, last_node, table.key());
                return Ok(());
            }
            let foster_parent = table_index
                .checked_sub(1)
                .and_then(|index| self.open_elements.get(index))
                .expect("detached foster table must still have a previous SOE entry");
            self.append_existing_child(foster_parent.key(), last_node);
            return Ok(());
        }

        let html_index = find_last_open_element_index(self, self.known_tags.html)
            .expect("HTML parsing stack must contain <html> for foster parenting");
        let html = self
            .open_elements
            .get(html_index)
            .expect("html index must remain valid while computing foster location");
        self.append_existing_child(html.key(), last_node);
        Ok(())
    }
}

fn find_last_open_element_index(builder: &Html5TreeBuilder, target: AtomId) -> Option<usize> {
    for index in (0..builder.open_elements.len()).rev() {
        let element = builder
            .open_elements
            .get(index)
            .expect("open-elements index must remain in bounds during foster scan");
        if element.name() == target {
            return Some(index);
        }
    }
    None
}

fn is_special_html_tag(name: AtomId, atoms: &AtomTable) -> Result<bool, TreeBuilderError> {
    Ok(matches!(
        resolve_atom(atoms, name)?,
        "address"
            | "applet"
            | "area"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "bgsound"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "iframe"
            | "img"
            | "input"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "marquee"
            | "menu"
            | "meta"
            | "nav"
            | "noembed"
            | "noframes"
            | "ol"
            | "p"
            | "param"
            | "plaintext"
            | "pre"
            | "script"
            | "search"
            | "section"
            | "select"
            | "source"
            | "style"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
            | "wbr"
            | "xmp"
    ))
}

#[cfg(test)]
mod tests {
    use super::Html5TreeBuilder;
    use crate::dom_patch::PatchKey;
    use crate::html5::shared::DocumentParseContext;
    use crate::html5::tree_builder::stack::OpenElement;

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

    #[test]
    fn adoption_agency_insert_last_node_uses_previous_soe_entry_when_table_is_detached() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, body) = bootstrap_html_body(&mut builder, &ctx);
        let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
        builder
            .with_structural_mutation(|this| {
                let table = this.create_detached_element(this.known_tags.table, &[], &ctx.atoms)?;
                let last_node = this.create_detached_element(div, &[], &ctx.atoms)?;
                this.open_elements
                    .push(OpenElement::new(table, this.known_tags.table));
                this.adoption_agency_insert_last_node(
                    OpenElement::new(PatchKey(999), this.known_tags.tbody),
                    last_node,
                )?;
                assert_eq!(this.live_tree.parent(last_node), Some(body));
                Ok(())
            })
            .expect("detached-table foster parenting should remain recoverable");
    }

    #[test]
    fn adoption_agency_insert_last_node_prefers_template_above_table() {
        let mut ctx = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");

        let (_html, _body) = bootstrap_html_body(&mut builder, &ctx);
        let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
        builder
            .with_structural_mutation(|this| {
                let table = this.create_detached_element(this.known_tags.table, &[], &ctx.atoms)?;
                let template =
                    this.create_detached_element(this.known_tags.template, &[], &ctx.atoms)?;
                let last_node = this.create_detached_element(div, &[], &ctx.atoms)?;
                this.open_elements
                    .push(OpenElement::new(table, this.known_tags.table));
                this.open_elements
                    .push(OpenElement::new(template, this.known_tags.template));
                this.adoption_agency_insert_last_node(
                    OpenElement::new(PatchKey(999), this.known_tags.thead),
                    last_node,
                )?;
                assert_eq!(this.live_tree.parent(last_node), Some(template));
                Ok(())
            })
            .expect("template-preferred foster parenting should remain recoverable");
    }
}

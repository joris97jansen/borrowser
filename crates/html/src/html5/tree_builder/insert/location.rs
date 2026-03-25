use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::tree_builder::stack::OpenElement;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct InsertionLocation {
    pub(in crate::html5::tree_builder) parent: PatchKey,
    pub(in crate::html5::tree_builder) before: Option<PatchKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FosterParentAnchor {
    Template(PatchKey),
    TableIndex(usize),
}

impl Html5TreeBuilder {
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

    fn foster_parenting_anchor_from_soe(&mut self) -> Option<FosterParentAnchor> {
        let indices = self.open_elements.foster_parenting_anchor_indices(
            self.known_tags.html,
            self.known_tags.table,
            self.known_tags.template,
        );
        let last_table_index = indices.table_index;
        let last_template_index = indices.template_index;
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
        &mut self,
    ) -> Result<InsertionLocation, TreeBuilderError> {
        let Some(html_index) = self
            .open_elements
            .foster_parenting_anchor_indices(
                self.known_tags.html,
                self.known_tags.table,
                self.known_tags.template,
            )
            .html_index
        else {
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
        &mut self,
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

    pub(super) fn element_or_text_insertion_location(
        &mut self,
    ) -> Result<InsertionLocation, TreeBuilderError> {
        if self.foster_parenting_enabled {
            self.foster_parenting_insertion_location()
        } else {
            self.current_insertion_location()
        }
    }

    pub(super) fn insert_existing_child_at(
        &mut self,
        location: InsertionLocation,
        child: PatchKey,
    ) {
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
}

use super::stack::OpenElementsStack;
use super::types::FosterParentingAnchorIndices;
use crate::html5::shared::AtomId;

impl OpenElementsStack {
    pub(crate) fn foster_parenting_anchor_indices(
        &mut self,
        html: AtomId,
        table: AtomId,
        template: AtomId,
    ) -> FosterParentingAnchorIndices {
        if let Some(cached) = self
            .foster_parenting_cache
            .get_if_valid(html, table, template)
        {
            return cached;
        }

        self.foster_parenting_cache.scan_calls =
            self.foster_parenting_cache.scan_calls.saturating_add(1);
        let mut indices = FosterParentingAnchorIndices::default();
        for index in (0..self.items.len()).rev() {
            self.foster_parenting_cache.scan_steps =
                self.foster_parenting_cache.scan_steps.saturating_add(1);
            let name = self.items[index].name();
            if indices.table_index.is_none() && name == table {
                indices.table_index = Some(index);
            } else if indices.template_index.is_none() && name == template {
                indices.template_index = Some(index);
            } else if indices.html_index.is_none() && name == html {
                indices.html_index = Some(index);
            }
            if indices.table_index.is_some()
                && indices.template_index.is_some()
                && indices.html_index.is_some()
            {
                break;
            }
        }
        self.foster_parenting_cache
            .store(html, table, template, indices);
        indices
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FosterParentingIndexCache {
    html: Option<AtomId>,
    table: Option<AtomId>,
    template: Option<AtomId>,
    indices: FosterParentingAnchorIndices,
    valid: bool,
    pub(super) scan_calls: u64,
    pub(super) scan_steps: u64,
}

impl FosterParentingIndexCache {
    pub(super) fn invalidate(&mut self) {
        self.valid = false;
    }

    pub(super) fn get_if_valid(
        &self,
        html: AtomId,
        table: AtomId,
        template: AtomId,
    ) -> Option<FosterParentingAnchorIndices> {
        if self.valid
            && self.html == Some(html)
            && self.table == Some(table)
            && self.template == Some(template)
        {
            Some(self.indices)
        } else {
            None
        }
    }

    pub(super) fn store(
        &mut self,
        html: AtomId,
        table: AtomId,
        template: AtomId,
        indices: FosterParentingAnchorIndices,
    ) {
        self.html = Some(html);
        self.table = Some(table);
        self.template = Some(template);
        self.indices = indices;
        self.valid = true;
    }

    pub(super) fn note_push(&mut self, index: usize, name: AtomId) {
        if !self.valid {
            return;
        }
        if Some(name) == self.html {
            self.indices.html_index = Some(index);
        } else if Some(name) == self.table {
            self.indices.table_index = Some(index);
        } else if Some(name) == self.template {
            self.indices.template_index = Some(index);
        }
    }

    pub(super) fn note_pop(&mut self, removed_index: usize, name: AtomId) {
        if !self.valid {
            return;
        }
        if (Some(name) == self.html && self.indices.html_index == Some(removed_index))
            || (Some(name) == self.table && self.indices.table_index == Some(removed_index))
            || (Some(name) == self.template && self.indices.template_index == Some(removed_index))
        {
            self.invalidate();
        }
    }

    pub(super) fn note_suffix_removal(&mut self, start_index: usize, old_len: usize) {
        if !self.valid {
            return;
        }
        debug_assert!(start_index < old_len);
        let removed = start_index..old_len;
        if self
            .indices
            .html_index
            .is_some_and(|index| removed.contains(&index))
            || self
                .indices
                .table_index
                .is_some_and(|index| removed.contains(&index))
            || self
                .indices
                .template_index
                .is_some_and(|index| removed.contains(&index))
        {
            self.invalidate();
        }
    }
}

//! Active formatting elements list.
//!
//! This module implements the parser-side active formatting elements (AFE)
//! storage used by HTML5 tree construction. The structure is intentionally
//! `Vec`-backed: ordering is semantically significant, scans are short and
//! deterministic, and Noah's Ark duplicate enforcement must not depend on hash
//! iteration order.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "AFE integration lands incrementally across Milestone H"
    )
)]

use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Attribute};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::stack::OpenElementsStack;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AfeAttributeSnapshot {
    pub(crate) name: AtomId,
    pub(crate) value: Option<String>,
}

impl AfeAttributeSnapshot {
    #[inline]
    pub(crate) fn new(name: AtomId, value: Option<String>) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AfeElementEntry {
    pub(crate) key: PatchKey,
    pub(crate) name: AtomId,
    pub(crate) attrs: Vec<AfeAttributeSnapshot>,
}

impl AfeElementEntry {
    #[inline]
    pub(crate) fn new(key: PatchKey, name: AtomId, attrs: Vec<AfeAttributeSnapshot>) -> Self {
        Self { key, name, attrs }
    }

    #[inline]
    fn same_name_and_attrs(&self, other: &Self) -> bool {
        self.name == other.name && self.attrs == other.attrs
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AfeEntry {
    Marker,
    Element(AfeElementEntry),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveFormattingList {
    items: Vec<AfeEntry>,
    max_depth: u32,
}

impl ActiveFormattingList {
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn clear(&mut self) {
        // Note: does not reset max_depth (high-water mark metric).
        self.items.clear();
    }

    pub(crate) fn push_marker(&mut self) {
        self.items.push(AfeEntry::Marker);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    /// Pushes a formatting element entry while enforcing the HTML5 Noah's Ark
    /// duplicate bound within the suffix after the last marker.
    ///
    /// Returns the evicted entry, if duplicate trimming removed one.
    pub(crate) fn push_formatting_element(
        &mut self,
        entry: AfeElementEntry,
    ) -> Option<AfeElementEntry> {
        // A duplicate PatchKey inside AFE is a tree-builder logic bug, not an
        // input-driven parse error. Keep this as a debug invariant while AFE
        // integration is staged across Milestone H, rather than introducing a
        // recoverable runtime error path for internal corruption.
        debug_assert!(
            self.find_by_key(entry.key).is_none(),
            "AFE invariant violated: duplicate PatchKey inserted into active formatting list"
        );

        let evicted = self.trim_oldest_duplicate_if_needed(&entry);
        self.items.push(AfeEntry::Element(entry));
        self.max_depth = self.max_depth.max(self.items.len() as u32);
        evicted
    }

    /// Removes AFE entries after the most recent marker and then removes that
    /// marker. If no marker exists, clears the entire list.
    ///
    /// Returns the number of removed entries.
    pub(crate) fn clear_to_last_marker(&mut self) -> usize {
        match self
            .items
            .iter()
            .rposition(|entry| *entry == AfeEntry::Marker)
        {
            Some(marker_index) => {
                let removed = self.items.len() - marker_index;
                self.items.truncate(marker_index);
                removed
            }
            None => {
                let removed = self.items.len();
                self.items.clear();
                removed
            }
        }
    }

    pub(crate) fn remove(&mut self, key: PatchKey) -> Option<AfeElementEntry> {
        let index = self.find_index_by_key(key)?;
        match self.items.remove(index) {
            AfeEntry::Marker => unreachable!("find_index_by_key() must return an element entry"),
            AfeEntry::Element(entry) => Some(entry),
        }
    }

    #[inline]
    pub(crate) fn find_by_key(&self, key: PatchKey) -> Option<&AfeElementEntry> {
        self.items.iter().find_map(|entry| match entry {
            AfeEntry::Marker => None,
            AfeEntry::Element(element) if element.key == key => Some(element),
            AfeEntry::Element(_) => None,
        })
    }

    #[inline]
    pub(crate) fn find_last_by_name_after_last_marker(
        &self,
        name: AtomId,
    ) -> Option<&AfeElementEntry> {
        self.items
            .iter()
            .rev()
            .take_while(|entry| !matches!(entry, AfeEntry::Marker))
            .find_map(|entry| match entry {
                AfeEntry::Marker => None,
                AfeEntry::Element(element) if element.name == name => Some(element),
                AfeEntry::Element(_) => None,
            })
    }

    #[inline]
    pub(crate) fn find_last_index_by_name_after_last_marker(&self, name: AtomId) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_, entry)| !matches!(entry, AfeEntry::Marker))
            .find_map(|(index, entry)| match entry {
                AfeEntry::Marker => None,
                AfeEntry::Element(element) if element.name == name => Some(index),
                AfeEntry::Element(_) => None,
            })
    }

    #[inline]
    pub(crate) fn max_depth(&self) -> u32 {
        self.max_depth
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub(crate) fn entries(&self) -> &[AfeEntry] {
        &self.items
    }

    #[inline]
    pub(crate) fn element_at(&self, index: usize) -> Option<&AfeElementEntry> {
        match self.items.get(index)? {
            AfeEntry::Marker => None,
            AfeEntry::Element(element) => Some(element),
        }
    }

    pub(crate) fn remove_element_at(&mut self, index: usize) -> AfeElementEntry {
        match self.items.remove(index) {
            AfeEntry::Marker => unreachable!("AAA must not remove a marker as an element"),
            AfeEntry::Element(entry) => entry,
        }
    }

    pub(crate) fn insert_element_at(&mut self, index: usize, entry: AfeElementEntry) {
        debug_assert!(
            self.find_by_key(entry.key).is_none(),
            "AFE invariant violated: duplicate PatchKey inserted into active formatting list"
        );
        self.items.insert(index, AfeEntry::Element(entry));
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    pub(crate) fn replace_element_at(
        &mut self,
        index: usize,
        entry: AfeElementEntry,
    ) -> AfeElementEntry {
        debug_assert!(
            self.find_by_key(entry.key).is_none()
                || self.find_index_by_key(entry.key) == Some(index),
            "AFE invariant violated: replacement PatchKey already present elsewhere in active formatting list"
        );
        match std::mem::replace(&mut self.items[index], AfeEntry::Element(entry)) {
            AfeEntry::Marker => unreachable!("AAA must not replace a marker as an element"),
            AfeEntry::Element(previous) => previous,
        }
    }

    fn reconstruction_start_index(&self, open_elements: &OpenElementsStack) -> Option<usize> {
        let mut index = self.items.len().checked_sub(1)?;
        match &self.items[index] {
            AfeEntry::Marker => return None,
            AfeEntry::Element(entry) if open_elements.contains_key(entry.key) => return None,
            AfeEntry::Element(_) => {}
        }
        while index > 0 {
            match &self.items[index - 1] {
                AfeEntry::Marker => break,
                AfeEntry::Element(entry) if open_elements.contains_key(entry.key) => break,
                AfeEntry::Element(_) => index -= 1,
            }
        }
        Some(index)
    }

    fn replace_key_at(&mut self, index: usize, key: PatchKey) {
        match &mut self.items[index] {
            AfeEntry::Marker => unreachable!("reconstruction key replacement targets elements"),
            AfeEntry::Element(entry) => entry.key = key,
        }
    }

    fn trim_oldest_duplicate_if_needed(
        &mut self,
        candidate: &AfeElementEntry,
    ) -> Option<AfeElementEntry> {
        let mut matching_entries = 0usize;
        let mut oldest_match_index = None;
        for index in (0..self.items.len()).rev() {
            match &self.items[index] {
                AfeEntry::Marker => break,
                AfeEntry::Element(existing) if existing.same_name_and_attrs(candidate) => {
                    matching_entries += 1;
                    oldest_match_index = Some(index);
                }
                AfeEntry::Element(_) => {}
            }
        }
        if matching_entries < 3 {
            return None;
        }
        let index = oldest_match_index.expect("matching_entries >= 3 implies an oldest match");
        match self.items.remove(index) {
            AfeEntry::Marker => unreachable!("duplicate trimming must never target a marker"),
            AfeEntry::Element(entry) => Some(entry),
        }
    }

    pub(crate) fn find_index_by_key(&self, key: PatchKey) -> Option<usize> {
        self.items.iter().position(|entry| match entry {
            AfeEntry::Marker => false,
            AfeEntry::Element(element) => element.key == key,
        })
    }
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn push_active_formatting_element(
        &mut self,
        key: PatchKey,
        name: AtomId,
        attrs: &[Attribute],
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let snapshot = self.snapshot_afe_attributes(attrs, text)?;
        let _ = self
            .active_formatting
            .push_formatting_element(AfeElementEntry::new(key, name, snapshot));
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn reconstruct_active_formatting_elements(
        &mut self,
        atoms: &AtomTable,
    ) -> Result<usize, TreeBuilderError> {
        let Some(start_index) = self
            .active_formatting
            .reconstruction_start_index(&self.open_elements)
        else {
            return Ok(0);
        };

        let mut reconstructed = 0usize;
        for index in start_index..self.active_formatting.items.len() {
            let entry = match &self.active_formatting.items[index] {
                AfeEntry::Marker => {
                    unreachable!("reconstruction suffix must not contain markers")
                }
                AfeEntry::Element(entry) => entry.clone(),
            };
            let new_key = self.insert_element_from_afe_entry(&entry, atoms)?;
            self.active_formatting.replace_key_at(index, new_key);
            reconstructed += 1;
        }
        Ok(reconstructed)
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveFormattingList, AfeAttributeSnapshot, AfeElementEntry, AfeEntry};
    use crate::dom_patch::PatchKey;
    use crate::html5::shared::AtomId;

    fn tag(id: u32) -> AtomId {
        AtomId(id)
    }

    fn key(id: u32) -> PatchKey {
        PatchKey(id)
    }

    fn attr(name: u32, value: Option<&str>) -> AfeAttributeSnapshot {
        AfeAttributeSnapshot::new(tag(name), value.map(str::to_string))
    }

    fn element(id: u32, name: u32, attrs: Vec<AfeAttributeSnapshot>) -> AfeElementEntry {
        AfeElementEntry::new(key(id), tag(name), attrs)
    }

    fn keys(list: &ActiveFormattingList) -> Vec<Option<PatchKey>> {
        list.entries()
            .iter()
            .map(|entry| match entry {
                AfeEntry::Marker => None,
                AfeEntry::Element(element) => Some(element.key),
            })
            .collect()
    }

    #[test]
    fn push_marker_adds_marker_entry() {
        let mut list = ActiveFormattingList::default();

        list.push_marker();

        assert_eq!(list.len(), 1);
        assert_eq!(list.entries(), &[AfeEntry::Marker]);
        assert_eq!(list.max_depth(), 1);
    }

    #[test]
    fn push_find_and_remove_formatting_element_round_trip() {
        let mut list = ActiveFormattingList::default();
        let entry = element(10, 1, vec![attr(100, Some("x"))]);

        let evicted = list.push_formatting_element(entry.clone());
        assert!(evicted.is_none());
        assert_eq!(list.find_by_key(key(10)), Some(&entry));

        let removed = list.remove(key(10));
        assert_eq!(removed, Some(entry));
        assert!(list.is_empty());
    }

    #[test]
    fn remove_missing_key_returns_none_without_mutation() {
        let mut list = ActiveFormattingList::default();
        let first = element(10, 1, vec![]);
        let second = element(11, 2, vec![]);
        list.push_formatting_element(first.clone());
        list.push_marker();
        list.push_formatting_element(second.clone());
        let before = list.entries().to_vec();

        let removed = list.remove(key(999));

        assert!(removed.is_none());
        assert_eq!(list.entries(), before.as_slice());
    }

    #[test]
    fn remove_preserves_marker_and_relative_order_of_remaining_entries() {
        let mut list = ActiveFormattingList::default();
        list.push_formatting_element(element(1, 1, vec![]));
        list.push_marker();
        let second = element(2, 2, vec![]);
        let third = element(3, 3, vec![]);
        list.push_formatting_element(second.clone());
        list.push_formatting_element(third);

        let removed = list.remove(second.key);

        assert_eq!(removed, Some(second));
        assert_eq!(keys(&list), vec![Some(key(1)), None, Some(key(3))]);
    }

    #[test]
    fn clear_to_last_marker_removes_suffix_and_marker_only() {
        let mut list = ActiveFormattingList::default();
        list.push_formatting_element(element(1, 1, vec![]));
        list.push_marker();
        list.push_formatting_element(element(2, 2, vec![]));
        list.push_formatting_element(element(3, 3, vec![]));

        let removed = list.clear_to_last_marker();

        assert_eq!(removed, 3);
        assert_eq!(keys(&list), vec![Some(key(1))]);
    }

    #[test]
    fn clear_to_last_marker_without_marker_clears_entire_list() {
        let mut list = ActiveFormattingList::default();
        list.push_formatting_element(element(1, 1, vec![]));
        list.push_formatting_element(element(2, 2, vec![]));

        let removed = list.clear_to_last_marker();

        assert_eq!(removed, 2);
        assert!(list.is_empty());
    }

    #[test]
    fn find_last_by_name_after_last_marker_respects_marker_boundary() {
        let mut list = ActiveFormattingList::default();
        let before_marker = element(1, 7, vec![]);
        let after_marker = element(2, 7, vec![]);
        list.push_formatting_element(before_marker);
        list.push_marker();
        list.push_formatting_element(after_marker.clone());

        let found = list.find_last_by_name_after_last_marker(tag(7));

        assert_eq!(found, Some(&after_marker));
    }

    #[test]
    fn noahs_ark_trims_oldest_matching_entry_deterministically() {
        let mut list = ActiveFormattingList::default();
        let attrs = vec![attr(100, Some("one")), attr(101, None)];
        let first = element(1, 7, attrs.clone());
        let second = element(2, 7, attrs.clone());
        let third = element(3, 7, attrs.clone());
        let fourth = element(4, 7, attrs);
        list.push_formatting_element(first.clone());
        list.push_formatting_element(second.clone());
        list.push_formatting_element(third.clone());

        let evicted = list.push_formatting_element(fourth.clone());

        assert_eq!(evicted, Some(first));
        assert_eq!(keys(&list), vec![Some(key(2)), Some(key(3)), Some(key(4))]);
    }

    #[test]
    fn noahs_ark_does_not_cross_marker_boundaries() {
        let mut list = ActiveFormattingList::default();
        let attrs = vec![attr(100, Some("shared"))];
        let before_marker = element(1, 7, attrs.clone());
        let after_marker_a = element(2, 7, attrs.clone());
        let after_marker_b = element(3, 7, attrs.clone());
        let after_marker_c = element(4, 7, attrs.clone());
        list.push_formatting_element(before_marker);
        list.push_marker();
        list.push_formatting_element(after_marker_a.clone());
        list.push_formatting_element(after_marker_b.clone());
        list.push_formatting_element(after_marker_c.clone());

        let evicted = list.push_formatting_element(element(5, 7, attrs));

        assert_eq!(evicted, Some(after_marker_a));
        assert_eq!(
            keys(&list),
            vec![Some(key(1)), None, Some(key(3)), Some(key(4)), Some(key(5))]
        );
    }

    #[test]
    fn noahs_ark_compares_full_attribute_sequence() {
        let mut list = ActiveFormattingList::default();
        let attrs_a = vec![attr(100, Some("x")), attr(101, Some("y"))];
        let attrs_b = vec![attr(101, Some("y")), attr(100, Some("x"))];
        let attrs_c = vec![attr(100, Some("x")), attr(101, Some(""))];
        list.push_formatting_element(element(1, 7, attrs_a.clone()));
        list.push_formatting_element(element(2, 7, attrs_b.clone()));
        list.push_formatting_element(element(3, 7, attrs_c.clone()));

        let evicted = list.push_formatting_element(element(4, 7, attrs_a.clone()));

        assert!(evicted.is_none());
        assert_eq!(
            keys(&list),
            vec![Some(key(1)), Some(key(2)), Some(key(3)), Some(key(4))]
        );
    }

    #[test]
    fn max_depth_tracks_high_water_mark_across_clear_operations() {
        let mut list = ActiveFormattingList::default();
        list.push_formatting_element(element(1, 1, vec![]));
        list.push_marker();
        list.push_formatting_element(element(2, 2, vec![]));
        assert_eq!(list.max_depth(), 3);

        list.clear_to_last_marker();
        list.clear();

        assert_eq!(list.max_depth(), 3);
        assert!(list.is_empty());
    }
}

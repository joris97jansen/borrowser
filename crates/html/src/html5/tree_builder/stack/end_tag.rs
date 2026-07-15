use super::open_elements::OpenElementsStack;
use super::types::{InBodyEndTagScan, OpenElement, OpenElementMatch};
use crate::html5::shared::{AtomId, AtomTable, EngineInvariantError};
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::html_semantics::is_special_html_element;

impl OpenElementsStack {
    /// Performs the single probe-only reverse scan required by the InBody
    /// "any other end tag" algorithm.
    pub(crate) fn scan_in_body_any_other_end_tag(
        &mut self,
        target: AtomId,
        atoms: &AtomTable,
    ) -> Result<InBodyEndTagScan, TreeBuilderError> {
        self.end_tag_scan_calls = self.end_tag_scan_calls.saturating_add(1);
        for index in (0..self.items.len()).rev() {
            self.end_tag_scan_steps = self.end_tag_scan_steps.saturating_add(1);
            let element = self.items[index];
            if element.name() == target {
                return Ok(InBodyEndTagScan::Matched(OpenElementMatch {
                    index,
                    element,
                }));
            }
            if is_special_html_element(element.name(), atoms)? {
                return Ok(InBodyEndTagScan::BlockedBySpecial { index, element });
            }
        }

        // Full-document InBody processing must retain the special HTML root.
        // Exhaustion therefore identifies broken parser state, not malformed
        // input recovery.
        Err(EngineInvariantError)
    }

    /// Removes the stack suffix through an identity/index captured by a prior
    /// probe, without rescanning for the target.
    pub(crate) fn pop_suffix_from_match(
        &mut self,
        matched: OpenElementMatch,
    ) -> Result<OpenElement, TreeBuilderError> {
        if self.items.get(matched.index).copied() != Some(matched.element) {
            return Err(EngineInvariantError);
        }
        let old_len = self.items.len();
        self.foster_parenting_cache
            .note_suffix_removal(matched.index, old_len);
        while self.items.len() > matched.index + 1 {
            let popped = self.items.pop().ok_or(EngineInvariantError)?;
            self.note_name_pop(popped.name());
        }
        let target = self.items.pop().ok_or(EngineInvariantError)?;
        if target != matched.element {
            return Err(EngineInvariantError);
        }
        self.note_name_pop(target.name());
        self.pop_ops = self
            .pop_ops
            .saturating_add((old_len - matched.index) as u64);
        Ok(target)
    }
}

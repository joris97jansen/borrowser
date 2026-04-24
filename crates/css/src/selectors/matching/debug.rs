use super::dom_index::write_selector_dom_snapshot_body;
use super::result::write_selector_match_outcome_snapshot_body;
use super::{SelectorDomIndex, SelectorMatchingContext, SelectorMatchingLimits};
use crate::selectors::{SelectorListParseResult, write_selector_parse_result_snapshot_body};
use std::fmt::Write;

impl SelectorDomIndex<'_> {
    /// Returns a deterministic selector-matching debug snapshot for one
    /// selector parse result evaluated against this normalized selector DOM.
    ///
    /// This surface is intentionally tied to the owned-tree DOM adapter used by
    /// regression tests. It combines:
    /// - the selector parse result snapshot body
    /// - the normalized selector DOM snapshot body
    /// - one selector-match outcome per indexed element in document order
    pub fn to_matching_debug_snapshot(&self, selectors: &SelectorListParseResult) -> String {
        self.to_matching_debug_snapshot_with_limits(selectors, SelectorMatchingLimits::default())
    }

    /// Returns a deterministic selector-matching debug snapshot for one
    /// selector parse result evaluated against this normalized selector DOM
    /// using explicit selector-matching limits.
    pub fn to_matching_debug_snapshot_with_limits(
        &self,
        selectors: &SelectorListParseResult,
        limits: SelectorMatchingLimits,
    ) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "selector-matching").expect("write snapshot");

        writeln!(&mut out, "selectors:").expect("write snapshot");
        write_selector_parse_result_snapshot_body(&mut out, selectors, 2);

        writeln!(&mut out, "dom:").expect("write snapshot");
        write_selector_dom_snapshot_body(&mut out, self, 2);

        writeln!(&mut out, "matches:").expect("write snapshot");
        let context = SelectorMatchingContext::with_limits(self, limits);
        for (target_index, element_id) in self.elements().enumerate() {
            writeln!(
                &mut out,
                "  target[{target_index}]: element={} name=\"{}\"",
                element_id.get(),
                context.element_name(element_id)
            )
            .expect("write snapshot");
            match context.match_selector_list(element_id, selectors) {
                Ok(outcome) => write_selector_match_outcome_snapshot_body(&mut out, &outcome, 4),
                Err(error) => {
                    writeln!(&mut out, "    limit-error: {error}").expect("write snapshot")
                }
            }
        }

        out
    }
}

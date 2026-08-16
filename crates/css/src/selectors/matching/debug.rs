use super::dom_index::write_selector_dom_snapshot_body;
use super::result::write_selector_match_outcome_snapshot_body;
use super::{
    SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment, SelectorMatchingLimits,
};
use crate::selectors::{SelectorListParseResult, write_selector_parse_result_snapshot_body};
use std::fmt::Write;

impl SelectorDomIndex<'_> {
    /// Returns a deterministic selector-matching debug snapshot for one
    /// selector parse result evaluated against this validated selector DOM.
    ///
    /// This surface is intentionally tied to a successfully built selector DOM
    /// projection. It combines:
    /// - the selector parse result snapshot body
    /// - the validated selector DOM snapshot body
    /// - one selector-match outcome per indexed element in document order
    pub fn to_matching_debug_snapshot(
        &self,
        matching_environment: SelectorMatchingEnvironment,
        selectors: &SelectorListParseResult,
    ) -> String {
        self.to_matching_debug_snapshot_with_limits(
            matching_environment,
            selectors,
            SelectorMatchingLimits::default(),
        )
    }

    /// Returns a deterministic selector-matching debug snapshot for one
    /// selector parse result evaluated against this validated selector DOM
    /// using explicit selector-matching limits.
    pub fn to_matching_debug_snapshot_with_limits(
        &self,
        matching_environment: SelectorMatchingEnvironment,
        selectors: &SelectorListParseResult,
        limits: SelectorMatchingLimits,
    ) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 3").expect("write snapshot");
        writeln!(&mut out, "selector-matching").expect("write snapshot");
        writeln!(
            &mut out,
            "matching-environment: document-mode={}",
            matching_environment.document_mode()
        )
        .expect("write snapshot");

        writeln!(&mut out, "selectors:").expect("write snapshot");
        write_selector_parse_result_snapshot_body(&mut out, selectors, 2);

        writeln!(&mut out, "dom:").expect("write snapshot");
        write_selector_dom_snapshot_body(&mut out, self, 2);

        writeln!(&mut out, "matches:").expect("write snapshot");
        let context = SelectorMatchingContext::with_limits(self, matching_environment, limits);
        for (target_index, element_id) in self.elements().enumerate() {
            writeln!(
                &mut out,
                "  target[{target_index}]: element={} name=\"{}\"",
                element_id.get(),
                context.element_local_name(element_id)
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

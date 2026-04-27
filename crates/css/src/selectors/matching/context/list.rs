use super::super::SelectorListMatchOutcome;
use super::SelectorMatchingContext;
use super::dom::SelectorMatchDom;
use super::limits::SelectorMatchingLimitError;
use crate::selectors::{SelectorList, SelectorListParseResult};

impl<D: SelectorMatchDom> SelectorMatchingContext<'_, D> {
    /// Matches one selector list against one target element using the current
    /// supported selector IR.
    ///
    /// Parsed selector lists are evaluated deterministically from the selector
    /// IR. Unsupported and invalid parse results remain explicit non-matchable
    /// outcomes, while selector-matching resource-limit failures remain
    /// explicit errors on this authoritative path.
    pub fn match_selector_list(
        &self,
        element: D::ElementId,
        selectors: &SelectorListParseResult,
    ) -> Result<SelectorListMatchOutcome, SelectorMatchingLimitError> {
        match selectors {
            SelectorListParseResult::Parsed(list) => {
                self.match_parsed_selector_list_checked(element, list)
            }
            SelectorListParseResult::Unsupported(_) => Ok(SelectorListMatchOutcome::unsupported()),
            SelectorListParseResult::Invalid(_) => Ok(SelectorListMatchOutcome::invalid()),
        }
    }

    /// Compatibility helper for callers that need a conservative fallback
    /// outcome instead of an explicit selector-matching limit error.
    ///
    /// Limit exhaustion is downgraded to an invalid non-matchable outcome here.
    /// Authoritative engine paths should prefer [`Self::match_selector_list`].
    pub fn match_selector_list_conservative(
        &self,
        element: D::ElementId,
        selectors: &SelectorListParseResult,
    ) -> SelectorListMatchOutcome {
        self.match_selector_list(element, selectors)
            .unwrap_or_else(|_| SelectorListMatchOutcome::invalid())
    }

    /// Compatibility alias for existing call sites that already opt into an
    /// explicitly named checked path.
    pub fn match_selector_list_checked(
        &self,
        element: D::ElementId,
        selectors: &SelectorListParseResult,
    ) -> Result<SelectorListMatchOutcome, SelectorMatchingLimitError> {
        self.match_selector_list(element, selectors)
    }

    fn match_parsed_selector_list_checked(
        &self,
        element: D::ElementId,
        selectors: &SelectorList,
    ) -> Result<SelectorListMatchOutcome, SelectorMatchingLimitError> {
        let mut builder = SelectorListMatchOutcome::builder();

        for (selector_index, selector) in selectors.iter().enumerate() {
            if self.matches_complex_selector_checked(element, selector)? {
                builder.record_match(selector_index, selector.specificity());
            }
        }

        Ok(builder.build())
    }
}

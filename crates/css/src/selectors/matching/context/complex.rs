use super::SelectorMatchingContext;
use super::budget::SelectorMatchBudget;
use super::dom::SelectorMatchDom;
use super::limits::SelectorMatchingLimitError;
use crate::selectors::{Combinator, ComplexSelector, CompoundSelector};

impl<D: SelectorMatchDom> SelectorMatchingContext<'_, D> {
    /// Matches one full complex selector against one target element.
    ///
    /// Evaluation proceeds right-to-left over the selector IR. Ancestor and
    /// previous-sibling search explore candidates nearest-first to keep
    /// traversal deterministic across equivalent DOM projections.
    ///
    /// Recursive backtracking is bounded by `SelectorMatchingLimits` so hostile
    /// selector/DOM combinations cannot traverse unbounded ancestor or sibling
    /// axes during one match attempt. Resource-limit failures remain explicit
    /// errors on this authoritative path.
    pub fn matches_complex_selector(
        &self,
        element: D::ElementId,
        selector: &ComplexSelector,
    ) -> Result<bool, SelectorMatchingLimitError> {
        let mut budget = SelectorMatchBudget::new(self.limits.max_axis_steps_per_match);
        self.matches_complex_selector_from_checked(
            element,
            selector,
            selector.tail().len(),
            &mut budget,
        )
    }

    /// Compatibility helper for callers that need a conservative `false`
    /// fallback instead of an explicit selector-matching limit error.
    ///
    /// Limit exhaustion is downgraded to `false` here. Authoritative engine
    /// paths should prefer [`Self::matches_complex_selector`].
    pub fn matches_complex_selector_conservative(
        &self,
        element: D::ElementId,
        selector: &ComplexSelector,
    ) -> bool {
        self.matches_complex_selector(element, selector)
            .unwrap_or(false)
    }

    /// Compatibility alias for existing call sites that already opt into an
    /// explicitly named checked path.
    pub fn matches_complex_selector_checked(
        &self,
        element: D::ElementId,
        selector: &ComplexSelector,
    ) -> Result<bool, SelectorMatchingLimitError> {
        self.matches_complex_selector(element, selector)
    }

    fn matches_complex_selector_from_checked(
        &self,
        element: D::ElementId,
        selector: &ComplexSelector,
        compound_index: usize,
        budget: &mut SelectorMatchBudget,
    ) -> Result<bool, SelectorMatchingLimitError> {
        let compound = complex_selector_compound(selector, compound_index);

        if !self.matches_compound_selector(element, compound) {
            return Ok(false);
        }

        if compound_index == 0 {
            return Ok(true);
        }

        let combined = &selector.tail()[compound_index - 1];

        match combined.combinator() {
            // Structural backtracking remains explicit here: we continue
            // exploring candidates until the remaining left-hand selector chain
            // succeeds or candidates are exhausted.
            Combinator::Descendant => {
                for candidate in self.ancestor_elements(element) {
                    budget.consume_axis_step()?;

                    if self.matches_complex_selector_from_checked(
                        candidate,
                        selector,
                        compound_index - 1,
                        budget,
                    )? {
                        return Ok(true);
                    }
                }

                Ok(false)
            }
            Combinator::Child => match self.parent_element(element) {
                Some(candidate) => {
                    budget.consume_axis_step()?;

                    self.matches_complex_selector_from_checked(
                        candidate,
                        selector,
                        compound_index - 1,
                        budget,
                    )
                }
                None => Ok(false),
            },
            Combinator::NextSibling => match self.previous_sibling_element(element) {
                Some(candidate) => {
                    budget.consume_axis_step()?;

                    self.matches_complex_selector_from_checked(
                        candidate,
                        selector,
                        compound_index - 1,
                        budget,
                    )
                }
                None => Ok(false),
            },
            Combinator::SubsequentSibling => {
                for candidate in self.previous_sibling_elements(element) {
                    budget.consume_axis_step()?;

                    if self.matches_complex_selector_from_checked(
                        candidate,
                        selector,
                        compound_index - 1,
                        budget,
                    )? {
                        return Ok(true);
                    }
                }

                Ok(false)
            }
        }
    }
}

fn complex_selector_compound(
    selector: &ComplexSelector,
    compound_index: usize,
) -> &CompoundSelector {
    if compound_index == 0 {
        selector.head()
    } else {
        selector.tail()[compound_index - 1].selector()
    }
}

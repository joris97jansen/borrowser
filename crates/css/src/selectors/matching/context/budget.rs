use super::limits::SelectorMatchingLimitError;

pub(super) struct SelectorMatchBudget {
    remaining_axis_steps: usize,
    configured_axis_steps: usize,
}

impl SelectorMatchBudget {
    pub(super) fn new(max_axis_steps: usize) -> Self {
        Self {
            remaining_axis_steps: max_axis_steps,
            configured_axis_steps: max_axis_steps,
        }
    }

    pub(super) fn consume_axis_step(&mut self) -> Result<(), SelectorMatchingLimitError> {
        if self.remaining_axis_steps == 0 {
            return Err(SelectorMatchingLimitError::AxisStepLimitExceeded {
                limit: self.configured_axis_steps,
            });
        }

        self.remaining_axis_steps -= 1;
        Ok(())
    }
}

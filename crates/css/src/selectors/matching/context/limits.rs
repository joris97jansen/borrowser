#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectorMatchingLimits {
    pub max_axis_steps_per_match: usize,
}

impl Default for SelectorMatchingLimits {
    fn default() -> Self {
        Self {
            max_axis_steps_per_match: 65_536,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorMatchingLimitError {
    AxisStepLimitExceeded { limit: usize },
}

impl std::fmt::Display for SelectorMatchingLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AxisStepLimitExceeded { limit } => {
                write!(f, "selector matching exceeded axis step limit {limit}")
            }
        }
    }
}

impl std::error::Error for SelectorMatchingLimitError {}

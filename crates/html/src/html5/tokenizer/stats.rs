use crate::html5::tokenizer::Html5Tokenizer;

/// Minimal tokenizer instrumentation.
///
/// Note: counters are populated in test/debug builds and when the
/// `debug-stats` feature is enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenizerStats {
    pub steps: u64,
    pub state_transitions: u64,
    pub tokens_emitted: u64,
    pub budget_exhaustions: u64,
    pub bytes_consumed: u64,
}

impl Html5Tokenizer {
    #[inline]
    pub(crate) fn stats_inc_steps(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.steps = self.stats.steps.saturating_add(1);
        }
    }

    #[inline]
    pub(crate) fn stats_inc_state_transitions(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.state_transitions = self.stats.state_transitions.saturating_add(1);
        }
    }

    #[inline]
    pub(crate) fn stats_inc_tokens_emitted(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.tokens_emitted = self.stats.tokens_emitted.saturating_add(1);
        }
    }

    #[inline]
    pub(crate) fn stats_inc_budget_exhaustions(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.budget_exhaustions = self.stats.budget_exhaustions.saturating_add(1);
        }
    }

    #[inline]
    pub(crate) fn stats_set_bytes_consumed(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.bytes_consumed = self.cursor as u64;
        }
    }
}

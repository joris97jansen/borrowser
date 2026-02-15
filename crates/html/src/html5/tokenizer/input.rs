//! Tokenizer input helpers.

use crate::html5::shared::Input;
use crate::html5::tokenizer::Html5Tokenizer;

impl Html5Tokenizer {
    pub(super) fn has_unconsumed_input(&self, input: &Input) -> bool {
        self.cursor < input.as_str().len()
    }

    pub(super) fn consume_all_available_input_scaffold_only(&mut self, input: &Input) -> bool {
        let end = input.as_str().len();
        if self.cursor >= end {
            return false;
        }
        self.cursor = end;
        true
    }
}

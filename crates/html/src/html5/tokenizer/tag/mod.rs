mod attributes;
mod emit;
mod name;
mod open;

use super::Html5Tokenizer;
use super::invariants::TokenizerInvariantKind;
use super::states::TokenizerState;

impl Html5Tokenizer {
    pub(in crate::html5::tokenizer) fn begin_pending_tag(
        &mut self,
        name_start: usize,
        is_end: bool,
    ) {
        self.abandon_pending_tag();
        self.tag_name_start = Some(name_start);
        self.current_tag_is_end = is_end;
    }

    pub(in crate::html5::tokenizer) fn enter_self_closing_start_tag_after_solidus(
        &mut self,
        solidus_position: usize,
    ) {
        self.current_tag_self_closing = false;
        // Replacing the position is intentional: after a failed solidus
        // transition, a later slash is the one that can actually set the
        // self-closing flag.
        self.current_tag_self_closing_solidus_position = Some(solidus_position);
        self.transition_to(TokenizerState::SelfClosingStartTag);
    }

    pub(in crate::html5::tokenizer) fn accept_current_tag_self_closing(&mut self) -> bool {
        if self.current_tag_self_closing_solidus_position.is_none() {
            self.latch_invariant(TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition);
            return false;
        }
        self.current_tag_self_closing = true;
        true
    }

    pub(in crate::html5::tokenizer) fn abandon_pending_tag(&mut self) {
        self.tag_name_start = None;
        self.tag_name_end = None;
        self.tag_name_complete = false;
        self.current_tag_is_end = false;
        self.current_tag_self_closing = false;
        self.current_tag_self_closing_solidus_position = None;
        self.current_tag_attrs.clear();
        self.clear_current_attribute();
        self.end_tag_prefix_consumed = false;
    }
}

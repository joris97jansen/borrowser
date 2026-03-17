use super::super::Html5Tokenizer;
use super::super::machine::Step;
use super::super::scan::{is_attribute_name_stop, is_unquoted_attr_value_stop};
use super::super::states::TokenizerState;
use crate::entities::decode_entities;
use crate::html5::shared::{Attribute, AttributeValue, DocumentParseContext, Input, TextSpan};

impl Html5Tokenizer {
    pub(crate) fn step_before_attribute_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BeforeAttributeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some('/') => {
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('>') => {
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') | Some('?') => {
                // Core v0 recovery policy (broad): in BeforeAttributeName we drop
                // delimiter-like/junk bytes that are not valid attribute-name
                // starts, regardless of how we entered this state (including, but
                // not limited to, unquoted-value recovery). This keeps name
                // tokenization deterministic under malformed input.
                let _ = self.consume(input);
                Step::Progress
            }
            Some(_) => {
                self.begin_current_attribute_at_cursor();
                self.transition_to(TokenizerState::AttributeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_attribute_name(
        &mut self,
        input: &Input,
        _ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeName);
        if self.current_attr_name_start.is_none() {
            self.transition_to(TokenizerState::BeforeAttributeName);
            return Step::Progress;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| !is_attribute_name_stop(ch));
        if consumed > 0 {
            self.current_attr_name_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                self.transition_to(TokenizerState::AfterAttributeName);
                Step::Progress
            }
            Some('/') => {
                // Delimiter handoff: keep '/' unconsumed here so
                // AfterAttributeName can handle self-closing transitions.
                self.transition_to(TokenizerState::AfterAttributeName);
                Step::Progress
            }
            Some('>') => {
                // Delimiter handoff: keep '>' unconsumed here so
                // AfterAttributeName emits/finalizes uniformly.
                self.transition_to(TokenizerState::AfterAttributeName);
                Step::Progress
            }
            Some('=') => {
                let _ = self.consume_if(input, '=');
                self.current_attr_has_value = true;
                self.transition_to(TokenizerState::BeforeAttributeValue);
                Step::Progress
            }
            Some(_) => {
                // Core v0 policy: preserve non-stop bytes in attribute names.
                let _ = self.consume(input);
                self.current_attr_name_end = Some(self.cursor);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_after_attribute_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterAttributeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some('/') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('=') => {
                let _ = self.consume_if(input, '=');
                self.current_attr_has_value = true;
                self.transition_to(TokenizerState::BeforeAttributeValue);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.finalize_current_attribute(input, ctx);
                self.begin_current_attribute_at_cursor();
                self.transition_to(TokenizerState::AttributeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_before_attribute_value(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BeforeAttributeValue);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some('"') => {
                let _ = self.consume_if(input, '"');
                self.begin_current_attribute_value_at_cursor();
                self.transition_to(TokenizerState::AttributeValueDoubleQuoted);
                Step::Progress
            }
            Some('\'') => {
                let _ = self.consume_if(input, '\'');
                self.begin_current_attribute_value_at_cursor();
                self.transition_to(TokenizerState::AttributeValueSingleQuoted);
                Step::Progress
            }
            Some('>') => {
                self.begin_current_attribute_value_at_cursor();
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.begin_current_attribute_value_at_cursor();
                self.transition_to(TokenizerState::AttributeValueUnquoted);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_attribute_value_double_quoted(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeValueDoubleQuoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '"');
        if consumed > 0 {
            self.current_attr_value_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '"') {
            self.transition_to(TokenizerState::AfterAttributeValueQuoted);
            Step::Progress
        } else {
            let _ = self.consume(input);
            self.current_attr_value_end = Some(self.cursor);
            Step::Progress
        }
    }

    pub(crate) fn step_attribute_value_single_quoted(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeValueSingleQuoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '\'');
        if consumed > 0 {
            self.current_attr_value_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '\'') {
            self.transition_to(TokenizerState::AfterAttributeValueQuoted);
            Step::Progress
        } else {
            let _ = self.consume(input);
            self.current_attr_value_end = Some(self.cursor);
            Step::Progress
        }
    }

    pub(crate) fn step_attribute_value_unquoted(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeValueUnquoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| !is_unquoted_attr_value_stop(ch));
        if consumed > 0 {
            self.current_attr_value_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, ch);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some('/') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') | Some('?') => {
                // Core v0 recovery: terminate current unquoted value and
                // reconsume the delimiter in BeforeAttributeName.
                self.finalize_current_attribute(input, ctx);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some(_) => {
                let _ = self.consume(input);
                self.current_attr_value_end = Some(self.cursor);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_after_attribute_value_quoted(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterAttributeValueQuoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, ch);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some('/') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.finalize_current_attribute(input, ctx);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_self_closing_start_tag(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::SelfClosingStartTag);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '>') {
            self.current_tag_self_closing = true;
            self.emit_current_tag(input, ctx);
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        self.transition_to(TokenizerState::BeforeAttributeName);
        Step::Progress
    }

    pub(super) fn clear_current_attribute(&mut self) {
        self.current_attr_name_start = None;
        self.current_attr_name_end = None;
        self.current_attr_has_value = false;
        self.current_attr_value_start = None;
        self.current_attr_value_end = None;
    }

    fn begin_current_attribute_at_cursor(&mut self) {
        self.current_attr_name_start = Some(self.cursor);
        self.current_attr_name_end = None;
        self.current_attr_has_value = false;
        self.current_attr_value_start = None;
        self.current_attr_value_end = None;
    }

    fn begin_current_attribute_value_at_cursor(&mut self) {
        self.current_attr_has_value = true;
        self.current_attr_value_start = Some(self.cursor);
        self.current_attr_value_end = Some(self.cursor);
    }

    pub(super) fn finalize_current_attribute(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) {
        let (name_start, name_end) =
            match (self.current_attr_name_start, self.current_attr_name_end) {
                (Some(start), Some(end)) if start < end => (start, end),
                _ => {
                    self.clear_current_attribute();
                    return;
                }
            };
        if name_end > input.as_str().len() || name_start > name_end {
            self.clear_current_attribute();
            return;
        }
        let raw_name = &input.as_str()[name_start..name_end];
        let name = self.intern_atom_or_invariant(ctx, raw_name, "attribute name");

        // Duplicate attribute policy (Core v0): first-wins per start tag;
        // later duplicates are dropped to match HTML tokenizer semantics.
        if self.current_tag_attrs.iter().any(|attr| attr.name == name) {
            self.clear_current_attribute();
            return;
        }

        let value = if self.current_attr_has_value {
            match (self.current_attr_value_start, self.current_attr_value_end) {
                (Some(start), Some(end))
                    if start <= end
                        && end <= input.as_str().len()
                        && input.as_str().is_char_boundary(start)
                        && input.as_str().is_char_boundary(end) =>
                {
                    let raw = &input.as_str()[start..end];
                    if !raw.as_bytes().contains(&b'&') {
                        Some(AttributeValue::Span(TextSpan::new(start, end)))
                    } else {
                        let decoded = decode_entities(raw);
                        match decoded {
                            std::borrow::Cow::Borrowed(_) => {
                                Some(AttributeValue::Span(TextSpan::new(start, end)))
                            }
                            std::borrow::Cow::Owned(value) => Some(AttributeValue::Owned(value)),
                        }
                    }
                }
                _ => Some(AttributeValue::Owned(String::new())),
            }
        } else {
            None
        };

        self.current_tag_attrs.push(Attribute { name, value });
        self.clear_current_attribute();
    }
}

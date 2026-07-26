use super::super::Html5Tokenizer;
use super::super::limits::{
    LIMIT_DETAIL_ATTRIBUTE_NAME, LIMIT_DETAIL_ATTRIBUTE_VALUE, LIMIT_DETAIL_ATTRIBUTES_PER_TAG,
};
use super::super::machine::Step;
use super::super::scan::{is_attribute_name_stop, is_unquoted_attr_value_stop};
use super::super::states::TokenizerState;
use crate::entities::{CharacterReferenceContext, decode_character_references};
use crate::html5::shared::{
    Attribute, AttributeValue, DocumentParseContext, Input, ParserRecoveryAction,
    ParserResourceLimit, TextSpan, TokenizerExtensionParseErrorCode, WhatwgParseErrorCode,
};

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
                let solidus_position = self.cursor;
                let _ = self.consume_if(input, '/');
                self.enter_self_closing_start_tag_after_solidus(solidus_position);
                Step::Progress
            }
            Some('>') => {
                let tag_end_position = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx, tag_end_position);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('=') => {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::UnexpectedEqualsSignBeforeAttributeName,
                    self.cursor,
                    ParserRecoveryAction::DropInputCharacter { code_point: '=' },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::
                            ERROR_DETAIL_UNEXPECTED_EQUALS_BEFORE_ATTRIBUTE_NAME,
                        Some('=' as u32),
                    ),
                );
                let _ = self.consume(input);
                Step::Progress
            }
            Some('"') | Some('\'') | Some('<') => {
                // Core v0 recovery policy (broad): in BeforeAttributeName we drop
                // delimiter-like/junk bytes that are not valid attribute-name
                // starts, regardless of how we entered this state (including, but
                // not limited to, unquoted-value recovery). This keeps name
                // tokenization deterministic under malformed input.
                let code_point = self
                    .peek(input)
                    .expect("BeforeAttributeName has unconsumed input");
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    self.cursor,
                    ParserRecoveryAction::DropInputCharacter { code_point },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::ERROR_DETAIL_INVALID_ATTRIBUTE_NAME,
                        Some(code_point as u32),
                    ),
                );
                let _ = self.consume(input);
                Step::Progress
            }
            Some('`') => {
                self.record_tokenizer_extension_parse_error_with_recovery(
                    input,
                    ctx,
                    TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName,
                    self.cursor,
                    ParserRecoveryAction::DropInputCharacter { code_point: '`' },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::
                            ERROR_DETAIL_DROPPED_GRAVE_ACCENT_BEFORE_ATTRIBUTE_NAME,
                        Some('`' as u32),
                    ),
                );
                let _ = self.consume(input);
                Step::Progress
            }
            Some('?') => {
                self.record_tokenizer_extension_parse_error_with_recovery(
                    input,
                    ctx,
                    TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName,
                    self.cursor,
                    ParserRecoveryAction::DropInputCharacter { code_point: '?' },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::
                            ERROR_DETAIL_DROPPED_QUESTION_MARK_BEFORE_ATTRIBUTE_NAME,
                        Some('?' as u32),
                    ),
                );
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
                let solidus_position = self.cursor;
                let _ = self.consume_if(input, '/');
                self.enter_self_closing_start_tag_after_solidus(solidus_position);
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
                let tag_end_position = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx, tag_end_position);
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
                self.record_tokenizer_parse_error(
                    input,
                    ctx,
                    WhatwgParseErrorCode::MissingAttributeValue,
                    self.cursor,
                    super::super::normalization::ERROR_DETAIL_INVALID_ATTRIBUTE_VALUE,
                    Some('>' as u32),
                );
                self.begin_current_attribute_value_at_cursor();
                self.finalize_current_attribute(input, ctx);
                let tag_end_position = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx, tag_end_position);
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
                let solidus_position = self.cursor;
                let _ = self.consume_if(input, '/');
                self.enter_self_closing_start_tag_after_solidus(solidus_position);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let tag_end_position = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx, tag_end_position);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('?') => {
                self.record_tokenizer_extension_parse_error_with_recovery(
                    input,
                    ctx,
                    TokenizerExtensionParseErrorCode::
                        TerminatedUnquotedAttributeValueBeforeQuestionMark,
                    self.cursor,
                    ParserRecoveryAction::ReconsumeInputCharacter { code_point: '?' },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::
                            ERROR_DETAIL_TERMINATED_UNQUOTED_VALUE_BEFORE_QUESTION_MARK,
                        Some('?' as u32),
                    ),
                );
                self.finalize_current_attribute(input, ctx);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') => {
                // Core v0 recovery: terminate current unquoted value and
                // reconsume the delimiter in BeforeAttributeName.
                let code_point = self
                    .peek(input)
                    .expect("unquoted attribute value has unconsumed input");
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::UnexpectedCharacterInUnquotedAttributeValue,
                    self.cursor,
                    ParserRecoveryAction::ReconsumeInputCharacter { code_point },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::ERROR_DETAIL_INVALID_ATTRIBUTE_VALUE,
                        Some(code_point as u32),
                    ),
                );
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
                let solidus_position = self.cursor;
                let _ = self.consume_if(input, '/');
                self.enter_self_closing_start_tag_after_solidus(solidus_position);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let tag_end_position = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx, tag_end_position);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                let code_point = self
                    .peek(input)
                    .expect("quoted attribute recovery has unconsumed input");
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::MissingWhitespaceBetweenAttributes,
                    self.cursor,
                    ParserRecoveryAction::ReconsumeInputCharacter { code_point },
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::
                            ERROR_DETAIL_MISSING_WHITESPACE_BETWEEN_ATTRIBUTES,
                        Some(code_point as u32),
                    ),
                );
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
        if self.peek(input) == Some('>') {
            let tag_end_position = self.cursor;
            let _ = self.consume_if(input, '>');
            if self.accept_current_tag_self_closing() {
                self.emit_current_tag(input, ctx, tag_end_position);
            } else {
                self.abandon_pending_tag();
            }
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        let code_point = self
            .peek(input)
            .expect("self-closing start tag has unconsumed input");
        self.record_tokenizer_parse_error_with_recovery(
            input,
            ctx,
            WhatwgParseErrorCode::UnexpectedSolidusInTag,
            self.cursor,
            ParserRecoveryAction::ReconsumeInputCharacter { code_point },
            super::super::normalization::legacy_diagnostic(
                super::super::normalization::ERROR_DETAIL_UNEXPECTED_SOLIDUS_IN_TAG,
                Some(code_point as u32),
            ),
        );
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
        if self.current_tag_attrs.len() >= self.max_attributes_per_tag() {
            self.record_limit_error(
                input,
                ctx,
                name_start,
                ParserResourceLimit::AttributesPerTag,
                LIMIT_DETAIL_ATTRIBUTES_PER_TAG,
                self.max_attributes_per_tag(),
            );
            self.clear_current_attribute();
            return;
        }
        let raw_name = &input.as_str()[name_start..name_end];
        let (raw_name, name_truncated) =
            self.truncate_str_to_bytes(raw_name, self.max_attribute_name_bytes());
        if name_truncated {
            self.record_limit_error(
                input,
                ctx,
                name_start,
                ParserResourceLimit::AttributeNameBytes,
                LIMIT_DETAIL_ATTRIBUTE_NAME,
                self.max_attribute_name_bytes(),
            );
        }
        self.record_attribute_name_parse_errors(input, ctx, raw_name, name_start);
        let normalized_name = self.replace_nulls_for_token_text(input, ctx, raw_name, name_start);
        let atom_text = normalized_name.as_deref().unwrap_or(raw_name);
        let name = self.intern_atom_or_invariant(ctx, atom_text, "attribute name");

        // Duplicate attribute policy (Core v0): first-wins per start tag;
        // later duplicates are dropped to match HTML tokenizer semantics.
        if self.current_tag_attrs.iter().any(|attr| attr.name == name) {
            self.record_tokenizer_parse_error_with_recovery(
                input,
                ctx,
                WhatwgParseErrorCode::DuplicateAttribute,
                name_start,
                ParserRecoveryAction::DropDuplicateAttribute,
                super::super::normalization::legacy_diagnostic(
                    super::super::normalization::ERROR_DETAIL_DUPLICATE_ATTRIBUTE,
                    None,
                ),
            );
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
                    let (raw, value_truncated) =
                        self.truncate_str_to_bytes(raw, self.max_attribute_value_bytes());
                    let truncated_end = start + raw.len();
                    if value_truncated {
                        self.record_limit_error(
                            input,
                            ctx,
                            start,
                            ParserResourceLimit::AttributeValueBytes,
                            LIMIT_DETAIL_ATTRIBUTE_VALUE,
                            self.max_attribute_value_bytes(),
                        );
                    }
                    let null_normalized = self.replace_nulls_for_token_text(input, ctx, raw, start);
                    let normalized = null_normalized.as_deref().unwrap_or(raw);
                    if !normalized.as_bytes().contains(&b'&') && null_normalized.is_none() {
                        AttributeValue::Span(TextSpan::new(start, truncated_end))
                    } else if !normalized.as_bytes().contains(&b'&') {
                        AttributeValue::Owned(normalized.to_string())
                    } else {
                        let decoded = decode_character_references(
                            normalized,
                            CharacterReferenceContext::AttributeValue,
                        );
                        self.record_character_reference_parse_errors(
                            input,
                            ctx,
                            start,
                            &decoded.diagnostics,
                        );
                        match decoded.text {
                            std::borrow::Cow::Borrowed(_) if null_normalized.is_none() => {
                                AttributeValue::Span(TextSpan::new(start, truncated_end))
                            }
                            std::borrow::Cow::Borrowed(value) => {
                                AttributeValue::Owned(value.to_string())
                            }
                            std::borrow::Cow::Owned(value) => AttributeValue::Owned(value),
                        }
                    }
                }
                _ => AttributeValue::Owned(String::new()),
            }
        } else {
            AttributeValue::Owned(String::new())
        };

        self.current_tag_attrs.push(Attribute { name, value });
        self.clear_current_attribute();
    }

    fn record_attribute_name_parse_errors(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        raw_name: &str,
        base_position: usize,
    ) {
        for (offset, ch) in raw_name.char_indices() {
            if matches!(ch, '"' | '\'' | '<') {
                self.record_tokenizer_parse_error(
                    input,
                    ctx,
                    WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    base_position + offset,
                    super::super::normalization::ERROR_DETAIL_INVALID_ATTRIBUTE_NAME,
                    Some(ch as u32),
                );
            } else if ch == '`' {
                self.record_tokenizer_extension_parse_error(
                    input,
                    ctx,
                    TokenizerExtensionParseErrorCode::GraveAccentInAttributeName,
                    base_position + offset,
                    super::super::normalization::ERROR_DETAIL_GRAVE_ACCENT_IN_ATTRIBUTE_NAME,
                    Some(ch as u32),
                );
            }
        }
    }
}

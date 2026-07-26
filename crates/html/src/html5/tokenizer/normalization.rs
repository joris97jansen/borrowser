use super::Html5Tokenizer;
use crate::entities::{CharacterReferenceDiagnostic, CharacterReferenceDiagnosticKind};
use crate::html5::shared::{
    DocumentParseContext, Input, ParseErrorCode, ParserRecoveryAction, ParserResourceLimit,
    TokenizerExtensionParseErrorCode, WhatwgParseErrorCode,
};

pub(super) const ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER: &str = "unexpected-null-character";
pub(super) const ERROR_DETAIL_EOF_IN_COMMENT: &str = "eof-in-comment";
pub(super) const ERROR_DETAIL_EOF_IN_DOCTYPE: &str = "eof-in-doctype";
pub(super) const ERROR_DETAIL_EOF_IN_END_TAG_OPEN: &str = "eof-in-end-tag-open";
pub(super) const ERROR_DETAIL_EOF_IN_MARKUP_DECLARATION: &str = "eof-in-markup-declaration";
pub(super) const ERROR_DETAIL_EOF_IN_SELF_CLOSING_START_TAG: &str = "eof-in-self-closing-start-tag";
pub(super) const ERROR_DETAIL_EOF_IN_TAG_NAME: &str = "eof-in-tag-name";
pub(super) const ERROR_DETAIL_EOF_IN_TAG_OPEN: &str = "eof-in-tag-open";
pub(super) const ERROR_DETAIL_EOF_IN_ATTRIBUTE: &str = "eof-in-attribute";
pub(super) const ERROR_DETAIL_EOF_IN_TEXT_MODE: &str = "eof-in-text-mode";
pub(super) const ERROR_DETAIL_EOF_IN_CDATA: &str = "eof-in-cdata";
pub(super) const ERROR_DETAIL_EOF_IN_PROCESSING_INSTRUCTION: &str = "eof-in-processing-instruction";
pub(super) const ERROR_DETAIL_INVALID_ATTRIBUTE_NAME: &str = "invalid-attribute-name";
pub(super) const ERROR_DETAIL_INVALID_ATTRIBUTE_VALUE: &str = "invalid-attribute-value";
pub(super) const ERROR_DETAIL_DUPLICATE_ATTRIBUTE: &str = "duplicate-attribute";
pub(super) const ERROR_DETAIL_INVALID_END_TAG_OPEN: &str = "invalid-end-tag-open";
pub(super) const ERROR_DETAIL_END_TAG_WITH_ATTRIBUTES: &str = "end-tag-with-attributes";
pub(super) const ERROR_DETAIL_END_TAG_WITH_TRAILING_SOLIDUS: &str = "end-tag-with-trailing-solidus";
pub(super) const ERROR_DETAIL_UNEXPECTED_SOLIDUS_IN_TAG: &str = "unexpected-solidus-in-tag";
pub(super) const ERROR_DETAIL_UNEXPECTED_EQUALS_BEFORE_ATTRIBUTE_NAME: &str =
    "unexpected-equals-sign-before-attribute-name";
pub(super) const ERROR_DETAIL_DROPPED_GRAVE_ACCENT_BEFORE_ATTRIBUTE_NAME: &str =
    "core-v0-dropped-grave-accent-before-attribute-name";
pub(super) const ERROR_DETAIL_GRAVE_ACCENT_IN_ATTRIBUTE_NAME: &str =
    "core-v0-grave-accent-in-attribute-name";
pub(super) const ERROR_DETAIL_DROPPED_QUESTION_MARK_BEFORE_ATTRIBUTE_NAME: &str =
    "core-v0-dropped-question-mark-before-attribute-name";
pub(super) const ERROR_DETAIL_TERMINATED_UNQUOTED_VALUE_BEFORE_QUESTION_MARK: &str =
    "core-v0-terminated-unquoted-attribute-value-before-question-mark";
pub(super) const ERROR_DETAIL_INVALID_MARKUP_DECLARATION: &str = "invalid-markup-declaration";
pub(super) const ERROR_DETAIL_INVALID_TAG_OPEN: &str = "invalid-tag-open";
pub(super) const ERROR_DETAIL_INVALID_FIRST_PROCESSING_INSTRUCTION_TARGET: &str =
    "invalid-first-character-of-processing-instruction-target";
pub(super) const ERROR_DETAIL_INVALID_PROCESSING_INSTRUCTION_TARGET: &str =
    "invalid-processing-instruction-target";
pub(super) const ERROR_DETAIL_DISALLOWED_PROCESSING_INSTRUCTION_TARGET: &str =
    "disallowed-processing-instruction-target";
pub(super) const ERROR_DETAIL_MALFORMED_COMMENT: &str = "malformed-comment";
pub(super) const ERROR_DETAIL_MALFORMED_DOCTYPE: &str = "malformed-doctype";
pub(super) const ERROR_DETAIL_MISSING_WHITESPACE_BETWEEN_ATTRIBUTES: &str =
    "missing-whitespace-between-attributes";

pub(in crate::html5::tokenizer) struct TokenizerLegacyDiagnostic {
    description: &'static str,
    aux: Option<u32>,
}

pub(in crate::html5::tokenizer) const fn legacy_diagnostic(
    description: &'static str,
    aux: Option<u32>,
) -> TokenizerLegacyDiagnostic {
    TokenizerLegacyDiagnostic { description, aux }
}

impl Html5Tokenizer {
    pub(in crate::html5::tokenizer) fn record_tokenizer_parse_error(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        code: WhatwgParseErrorCode,
        position: usize,
        detail: &'static str,
        aux: Option<u32>,
    ) {
        ctx.record_tokenizer_parse_error(
            input,
            ParseErrorCode::Standard(code),
            position,
            None,
            Some(detail),
            aux,
        );
    }

    pub(in crate::html5::tokenizer) fn record_tokenizer_parse_error_with_recovery(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        code: WhatwgParseErrorCode,
        position: usize,
        recovery: ParserRecoveryAction,
        legacy: TokenizerLegacyDiagnostic,
    ) {
        ctx.record_tokenizer_parse_error(
            input,
            ParseErrorCode::Standard(code),
            position,
            Some(recovery),
            Some(legacy.description),
            legacy.aux,
        );
    }

    pub(in crate::html5::tokenizer) fn record_tokenizer_extension_parse_error(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        code: TokenizerExtensionParseErrorCode,
        position: usize,
        detail: &'static str,
        aux: Option<u32>,
    ) {
        ctx.record_tokenizer_parse_error(
            input,
            ParseErrorCode::TokenizerExtension(code),
            position,
            None,
            Some(detail),
            aux,
        );
    }

    pub(in crate::html5::tokenizer) fn record_tokenizer_extension_parse_error_with_recovery(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        code: TokenizerExtensionParseErrorCode,
        position: usize,
        recovery: ParserRecoveryAction,
        legacy: TokenizerLegacyDiagnostic,
    ) {
        ctx.record_tokenizer_parse_error(
            input,
            ParseErrorCode::TokenizerExtension(code),
            position,
            Some(recovery),
            Some(legacy.description),
            legacy.aux,
        );
    }

    pub(in crate::html5::tokenizer) fn record_character_reference_parse_errors(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        base_position: usize,
        diagnostics: &[CharacterReferenceDiagnostic],
    ) {
        for diagnostic in diagnostics {
            let position = base_position + diagnostic.offset;
            if diagnostic.kind == CharacterReferenceDiagnosticKind::NumericTooLong {
                ctx.record_resource_limit(
                    input,
                    ParserResourceLimit::NumericCharacterReferenceDigits,
                    diagnostic
                        .configured_limit
                        .expect("numeric-reference limit diagnostic carries its configured limit"),
                    position,
                    Some(diagnostic.kind.detail()),
                );
            } else {
                let code = character_reference_error_code(diagnostic.kind);
                ctx.record_tokenizer_parse_error(
                    input,
                    code,
                    position,
                    character_reference_recovery(diagnostic.kind),
                    Some(diagnostic.kind.detail()),
                    diagnostic.aux,
                );
            }
        }
    }

    pub(in crate::html5::tokenizer) fn replace_nulls_for_token_text(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        raw: &str,
        base_position: usize,
    ) -> Option<String> {
        let mut normalized = None;
        for (offset, ch) in raw.char_indices() {
            if ch == '\0' {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::UnexpectedNullCharacter,
                    base_position + offset,
                    ParserRecoveryAction::ReplaceInvalidInput,
                    legacy_diagnostic(ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER, Some(0)),
                );
                normalized
                    .get_or_insert_with(|| {
                        let mut prefix = String::with_capacity(raw.len());
                        prefix.push_str(&raw[..offset]);
                        prefix
                    })
                    .push('\u{FFFD}');
            } else if let Some(out) = normalized.as_mut() {
                out.push(ch);
            }
        }
        normalized
    }
}

fn character_reference_error_code(kind: CharacterReferenceDiagnosticKind) -> ParseErrorCode {
    match kind {
        CharacterReferenceDiagnosticKind::UnknownNamed => {
            ParseErrorCode::Standard(WhatwgParseErrorCode::UnknownNamedCharacterReference)
        }
        CharacterReferenceDiagnosticKind::MissingNamedSemicolon
        | CharacterReferenceDiagnosticKind::MissingNumericSemicolon => {
            ParseErrorCode::Standard(WhatwgParseErrorCode::MissingSemicolonAfterCharacterReference)
        }
        CharacterReferenceDiagnosticKind::MissingNumericDigits => ParseErrorCode::Standard(
            WhatwgParseErrorCode::AbsenceOfDigitsInNumericCharacterReference,
        ),
        CharacterReferenceDiagnosticKind::MalformedNumeric => ParseErrorCode::TokenizerExtension(
            TokenizerExtensionParseErrorCode::MalformedNumericCharacterReference,
        ),
        CharacterReferenceDiagnosticKind::NumericTooLong => {
            unreachable!("numeric-reference bounds are implementation diagnostics")
        }
        CharacterReferenceDiagnosticKind::SurrogateNumericScalar => {
            ParseErrorCode::Standard(WhatwgParseErrorCode::SurrogateCharacterReference)
        }
        CharacterReferenceDiagnosticKind::OutOfRangeNumericScalar => {
            ParseErrorCode::Standard(WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange)
        }
    }
}

fn character_reference_recovery(
    kind: CharacterReferenceDiagnosticKind,
) -> Option<ParserRecoveryAction> {
    match kind {
        CharacterReferenceDiagnosticKind::SurrogateNumericScalar
        | CharacterReferenceDiagnosticKind::OutOfRangeNumericScalar => {
            Some(ParserRecoveryAction::PreserveCharacterReferenceLiteral)
        }
        CharacterReferenceDiagnosticKind::UnknownNamed
        | CharacterReferenceDiagnosticKind::MissingNamedSemicolon
        | CharacterReferenceDiagnosticKind::MissingNumericSemicolon
        | CharacterReferenceDiagnosticKind::MissingNumericDigits
        | CharacterReferenceDiagnosticKind::MalformedNumeric => None,
        CharacterReferenceDiagnosticKind::NumericTooLong => {
            unreachable!("numeric-reference bounds are implementation diagnostics")
        }
    }
}

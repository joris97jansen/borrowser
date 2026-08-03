use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, fixed_fields, strict_record_lines, validate_u64,
};
use html::ElementNamespace;
use html::conformance::*;
use std::fmt::Write;

const HEADER: &str = "# format: html5-parse-errors-v1";

define_snapshot_types!(ParsedParseErrorsSnapshot, CanonicalParseErrorsSnapshot);

pub(super) fn write(
    state: &ObservationState<Vec<ParseErrorEvent>>,
) -> Result<CanonicalParseErrorsSnapshot, ()> {
    let ObservationState::Captured(events) = state else {
        return Err(());
    };
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    for event in events {
        let (context, token, mode, namespace) = context_fields(event.context.as_ref());
        let line = format!(
            "PARSE_ERROR occurrence={} stage={} code={} recovery={} position={} context={} context-token={} context-mode={} context-namespace={}",
            event.occurrence,
            stage_name(event.stage),
            code_name(event.code),
            recovery_name(event.recovery.as_ref()),
            position_name(&event.position),
            context,
            token,
            mode,
            namespace
        );
        let _ = writeln!(&mut bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("occurrence {}", event.occurrence),
            line,
        });
    }
    Ok(CanonicalParseErrorsSnapshot::new(SnapshotData::new(
        bytes, records,
    )))
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedParseErrorsSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, true)?;
    let mut expected = 1u64;
    let mut records = Vec::new();
    for (line_number, line) in lines {
        let Some(fields) = fixed_fields(
            line,
            "PARSE_ERROR",
            &[
                "occurrence",
                "stage",
                "code",
                "recovery",
                "position",
                "context",
                "context-token",
                "context-mode",
                "context-namespace",
            ],
        ) else {
            return malformed(line_number, "invalid parse-error record shape");
        };
        if !validate_u64(fields[0]) || fields[0].parse::<u64>().ok() != Some(expected) {
            return Err(SnapshotReadError::NonContiguousOrdinal { line: line_number });
        }
        if !valid_stage(fields[1])
            || !valid_code(fields[2])
            || !valid_recovery(fields[3])
            || !valid_position(fields[4])
            || !valid_context(fields[5], fields[6], fields[7], fields[8])
        {
            return malformed(
                line_number,
                "unknown spelling or malformed parse-error field",
            );
        }
        records.push(SnapshotRecord {
            location: format!("occurrence {expected}"),
            line: line.to_string(),
        });
        expected = expected
            .checked_add(1)
            .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?;
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SnapshotReadError::InvalidUtf8)?;
    Ok(ParsedParseErrorsSnapshot::new(SnapshotData::new(
        text.to_string(),
        records,
    )))
}

fn stage_name(stage: ParserStage) -> &'static str {
    match stage {
        ParserStage::InputPreprocessing(InputPreprocessingStage::Utf8Decoding) => {
            "input-preprocessing:utf8-decoding"
        }
        ParserStage::InputPreprocessing(InputPreprocessingStage::NewlineNormalization) => {
            "input-preprocessing:newline-normalization"
        }
        ParserStage::Tokenizer => "tokenizer",
        ParserStage::TreeConstruction => "tree-construction",
        ParserStage::Finalization => "finalization",
    }
}

fn valid_stage(value: &str) -> bool {
    matches!(
        value,
        "input-preprocessing:utf8-decoding"
            | "input-preprocessing:newline-normalization"
            | "tokenizer"
            | "tree-construction"
            | "finalization"
    )
}

fn code_name(code: ParseErrorCode) -> &'static str {
    match code {
        ParseErrorCode::Standard(code) => whatwg_code(code),
        ParseErrorCode::TokenizerExtension(code) => tokenizer_extension_code(code),
        ParseErrorCode::TreeConstruction(code) => tree_code(code),
    }
}

fn whatwg_code(code: WhatwgParseErrorCode) -> &'static str {
    match code {
        WhatwgParseErrorCode::UnexpectedNullCharacter => "standard:unexpected-null-character",
        WhatwgParseErrorCode::EofBeforeTagName => "standard:eof-before-tag-name",
        WhatwgParseErrorCode::InvalidFirstCharacterOfTagName => {
            "standard:invalid-first-character-of-tag-name"
        }
        WhatwgParseErrorCode::MissingEndTagName => "standard:missing-end-tag-name",
        WhatwgParseErrorCode::EofInTag => "standard:eof-in-tag",
        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName => {
            "standard:unexpected-character-in-attribute-name"
        }
        WhatwgParseErrorCode::UnexpectedEqualsSignBeforeAttributeName => {
            "standard:unexpected-equals-sign-before-attribute-name"
        }
        WhatwgParseErrorCode::DuplicateAttribute => "standard:duplicate-attribute",
        WhatwgParseErrorCode::UnexpectedCharacterInUnquotedAttributeValue => {
            "standard:unexpected-character-in-unquoted-attribute-value"
        }
        WhatwgParseErrorCode::MissingAttributeValue => "standard:missing-attribute-value",
        WhatwgParseErrorCode::MissingWhitespaceBetweenAttributes => {
            "standard:missing-whitespace-between-attributes"
        }
        WhatwgParseErrorCode::UnexpectedSolidusInTag => "standard:unexpected-solidus-in-tag",
        WhatwgParseErrorCode::EofInComment => "standard:eof-in-comment",
        WhatwgParseErrorCode::IncorrectlyOpenedComment => "standard:incorrectly-opened-comment",
        WhatwgParseErrorCode::AbruptClosingOfEmptyComment => {
            "standard:abrupt-closing-of-empty-comment"
        }
        WhatwgParseErrorCode::NestedComment => "standard:nested-comment",
        WhatwgParseErrorCode::IncorrectlyClosedComment => "standard:incorrectly-closed-comment",
        WhatwgParseErrorCode::EofInDoctype => "standard:eof-in-doctype",
        WhatwgParseErrorCode::MissingWhitespaceBeforeDoctypeName => {
            "standard:missing-whitespace-before-doctype-name"
        }
        WhatwgParseErrorCode::MissingDoctypeName => "standard:missing-doctype-name",
        WhatwgParseErrorCode::InvalidCharacterSequenceAfterDoctypeName => {
            "standard:invalid-character-sequence-after-doctype-name"
        }
        WhatwgParseErrorCode::EofInCdata => "standard:eof-in-cdata",
        WhatwgParseErrorCode::EndTagWithAttributes => "standard:end-tag-with-attributes",
        WhatwgParseErrorCode::EndTagWithTrailingSolidus => "standard:end-tag-with-trailing-solidus",
        WhatwgParseErrorCode::InvalidFirstCharacterOfProcessingInstructionTarget => {
            "standard:invalid-first-character-of-processing-instruction-target"
        }
        WhatwgParseErrorCode::InvalidProcessingInstructionTarget => {
            "standard:invalid-processing-instruction-target"
        }
        WhatwgParseErrorCode::DisallowedProcessingInstructionTarget => {
            "standard:disallowed-processing-instruction-target"
        }
        WhatwgParseErrorCode::EofInProcessingInstruction => {
            "standard:eof-in-processing-instruction"
        }
        WhatwgParseErrorCode::MissingSemicolonAfterCharacterReference => {
            "standard:missing-semicolon-after-character-reference"
        }
        WhatwgParseErrorCode::UnknownNamedCharacterReference => {
            "standard:unknown-named-character-reference"
        }
        WhatwgParseErrorCode::AbsenceOfDigitsInNumericCharacterReference => {
            "standard:absence-of-digits-in-numeric-character-reference"
        }
        WhatwgParseErrorCode::NullCharacterReference => "standard:null-character-reference",
        WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange => {
            "standard:character-reference-outside-unicode-range"
        }
        WhatwgParseErrorCode::SurrogateCharacterReference => {
            "standard:surrogate-character-reference"
        }
        WhatwgParseErrorCode::NoncharacterCharacterReference => {
            "standard:noncharacter-character-reference"
        }
        WhatwgParseErrorCode::ControlCharacterReference => "standard:control-character-reference",
    }
}

fn tokenizer_extension_code(code: TokenizerExtensionParseErrorCode) -> &'static str {
    match code {
        TokenizerExtensionParseErrorCode::MalformedNumericCharacterReference => {
            "tokenizer-extension:malformed-numeric-character-reference"
        }
        TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName => {
            "tokenizer-extension:dropped-grave-accent-before-attribute-name"
        }
        TokenizerExtensionParseErrorCode::GraveAccentInAttributeName => {
            "tokenizer-extension:grave-accent-in-attribute-name"
        }
        TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName => {
            "tokenizer-extension:dropped-question-mark-before-attribute-name"
        }
        TokenizerExtensionParseErrorCode::TerminatedUnquotedAttributeValueBeforeQuestionMark => {
            "tokenizer-extension:terminated-unquoted-attribute-value-before-question-mark"
        }
    }
}

fn tree_code(code: TreeConstructionParseErrorCode) -> &'static str {
    match code {
        TreeConstructionParseErrorCode::ExpectedDoctypeBeforeNonSpaceToken => {
            "tree-construction:expected-doctype-before-non-space-token"
        }
        TreeConstructionParseErrorCode::DoctypeTokenNotAllowed => {
            "tree-construction:doctype-token-not-allowed"
        }
        TreeConstructionParseErrorCode::StartTagForbiddenByActiveInsertionMode => {
            "tree-construction:start-tag-forbidden-by-active-insertion-mode"
        }
        TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode => {
            "tree-construction:end-tag-forbidden-by-active-insertion-mode"
        }
        TreeConstructionParseErrorCode::HtmlStartTagAfterHtmlElement => {
            "tree-construction:html-start-tag-after-html-element"
        }
        TreeConstructionParseErrorCode::BodyStartTagAfterBodyElement => {
            "tree-construction:body-start-tag-after-body-element"
        }
        TreeConstructionParseErrorCode::TokenForbiddenAfterBody => {
            "tree-construction:token-forbidden-after-body"
        }
        TreeConstructionParseErrorCode::TokenForbiddenAfterAfterBody => {
            "tree-construction:token-forbidden-after-after-body"
        }
        TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag => {
            "tree-construction:unacknowledged-self-closing-flag"
        }
        TreeConstructionParseErrorCode::ElementEndTagNotInRequiredScope => {
            "tree-construction:element-end-tag-not-in-required-scope"
        }
        TreeConstructionParseErrorCode::CurrentNodeMismatchAfterImpliedEndTags => {
            "tree-construction:current-node-mismatch-after-implied-end-tags"
        }
        TreeConstructionParseErrorCode::ParagraphEndTagWithoutParagraphInButtonScope => {
            "tree-construction:paragraph-end-tag-without-paragraph-in-button-scope"
        }
        TreeConstructionParseErrorCode::AnyOtherEndTagBlockedBySpecialElement => {
            "tree-construction:any-other-end-tag-blocked-by-special-element"
        }
        TreeConstructionParseErrorCode::FormStartTagWithActiveFormPointer => {
            "tree-construction:form-start-tag-with-active-form-pointer"
        }
        TreeConstructionParseErrorCode::FormEndTagWithoutFormElement => {
            "tree-construction:form-end-tag-without-form-element"
        }
        TreeConstructionParseErrorCode::FormEndTagFormElementNotInScope => {
            "tree-construction:form-end-tag-form-element-not-in-scope"
        }
        TreeConstructionParseErrorCode::SelectStartTagWithSelectInScope => {
            "tree-construction:select-start-tag-with-select-in-scope"
        }
        TreeConstructionParseErrorCode::SelectFamilyElementRemainsAfterImpliedEndTags => {
            "tree-construction:select-family-element-remains-after-implied-end-tags"
        }
        TreeConstructionParseErrorCode::ActiveAnchorStartTag => {
            "tree-construction:active-anchor-start-tag"
        }
        TreeConstructionParseErrorCode::NobrStartTagWithNobrInScope => {
            "tree-construction:nobr-start-tag-with-nobr-in-scope"
        }
        TreeConstructionParseErrorCode::AdoptionFormattingElementMissingFromOpenElements => {
            "tree-construction:adoption-formatting-element-missing-from-open-elements"
        }
        TreeConstructionParseErrorCode::AdoptionFormattingElementNotInScope => {
            "tree-construction:adoption-formatting-element-not-in-scope"
        }
        TreeConstructionParseErrorCode::AdoptionFormattingElementNotCurrentNode => {
            "tree-construction:adoption-formatting-element-not-current-node"
        }
        TreeConstructionParseErrorCode::FormStartTagInTable => {
            "tree-construction:form-start-tag-in-table"
        }
        TreeConstructionParseErrorCode::HiddenInputStartTagInTable => {
            "tree-construction:hidden-input-start-tag-in-table"
        }
        TreeConstructionParseErrorCode::NonSpaceCharacterInTableText => {
            "tree-construction:non-space-character-in-table-text"
        }
        TreeConstructionParseErrorCode::NonTableTokenInTable => {
            "tree-construction:non-table-token-in-table"
        }
        TreeConstructionParseErrorCode::NestedTableStartTag => {
            "tree-construction:nested-table-start-tag"
        }
        TreeConstructionParseErrorCode::CellStartTagWithoutOpenRow => {
            "tree-construction:cell-start-tag-without-open-row"
        }
        TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope => {
            "tree-construction:table-context-element-not-in-required-scope"
        }
        TreeConstructionParseErrorCode::CurrentNodeNotColgroup => {
            "tree-construction:current-node-not-colgroup"
        }
        TreeConstructionParseErrorCode::EofWithOpenTemplate => {
            "tree-construction:eof-with-open-template"
        }
        TreeConstructionParseErrorCode::EofInTextMode => "tree-construction:eof-in-text-mode",
        TreeConstructionParseErrorCode::HtmlTokenNotAllowedInForeignContent => {
            "tree-construction:html-token-not-allowed-in-foreign-content"
        }
        TreeConstructionParseErrorCode::NullCharacterInForeignContent => {
            "tree-construction:null-character-in-foreign-content"
        }
        TreeConstructionParseErrorCode::ForeignEndTagCurrentNodeMismatch => {
            "tree-construction:foreign-end-tag-current-node-mismatch"
        }
    }
}

fn recovery_name(recovery: Option<&ParserRecoveryAction>) -> String {
    match recovery {
        None => "null".to_string(),
        Some(ParserRecoveryAction::IgnoreToken) => "ignore-token".to_string(),
        Some(ParserRecoveryAction::ReprocessToken) => "reprocess-token".to_string(),
        Some(ParserRecoveryAction::DropDuplicateAttribute) => {
            "drop-duplicate-attribute".to_string()
        }
        Some(ParserRecoveryAction::DropInputCharacter { code_point }) => {
            format!("drop-input-character:U+{:06X}", u32::from(*code_point))
        }
        Some(ParserRecoveryAction::ReconsumeInputCharacter { code_point }) => {
            format!("reconsume-input-character:U+{:06X}", u32::from(*code_point))
        }
        Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData) => {
            "emit-current-comment-and-switch-to-data".to_string()
        }
        Some(ParserRecoveryAction::EmitCurrentCommentAtEof) => {
            "emit-current-comment-at-eof".to_string()
        }
        Some(ParserRecoveryAction::StartBogusComment) => "start-bogus-comment".to_string(),
        Some(ParserRecoveryAction::RetainNestedCommentDelimiterAndReconsumeInCommentEnd {
            code_point,
        }) => format!(
            "retain-nested-comment-delimiter-and-reconsume-in-comment-end:U+{:06X}",
            u32::from(*code_point)
        ),
        Some(ParserRecoveryAction::DropEndTagAttributes) => "drop-end-tag-attributes".to_string(),
        Some(ParserRecoveryAction::IgnoreEndTagTrailingSolidus) => {
            "ignore-end-tag-trailing-solidus".to_string()
        }
        Some(ParserRecoveryAction::PreserveCharacterReferenceLiteral) => {
            "preserve-character-reference-literal".to_string()
        }
        Some(ParserRecoveryAction::InsertImpliedElement) => "insert-implied-element".to_string(),
        Some(ParserRecoveryAction::GenerateImpliedEndTags) => {
            "generate-implied-end-tags".to_string()
        }
        Some(ParserRecoveryAction::FosterParent) => "foster-parent".to_string(),
        Some(ParserRecoveryAction::PopOpenElements) => "pop-open-elements".to_string(),
        Some(ParserRecoveryAction::ReplaceInvalidInput) => "replace-invalid-input".to_string(),
        Some(ParserRecoveryAction::IgnoreSelfClosingFlag) => "ignore-self-closing-flag".to_string(),
    }
}

fn position_name(position: &EventPosition) -> String {
    match position {
        EventPosition::Unavailable(PositionUnavailableReason::ParserDidNotProvidePosition) => {
            "unavailable:parser-did-not-provide-position".to_string()
        }
        EventPosition::Known(position) => {
            let source = match position.source_bytes {
                SourceBytePosition::Exact(offset) => format!("exact:{offset}"),
                SourceBytePosition::Unavailable(
                    SourcePositionUnavailableReason::NoInputProvenanceMap,
                ) => "unavailable:no-input-provenance-map".to_string(),
            };
            match position.normalized.space {
                InputCoordinateSpace::NormalizedUtf8 => format!(
                    "normalized-utf8:{}:{}:{}:source-{source}",
                    position.normalized.utf8_byte_offset,
                    position.normalized.line.get(),
                    position.normalized.column.get()
                ),
            }
        }
    }
}

fn context_fields(
    context: Option<&ParserContextSummary>,
) -> (&'static str, &'static str, &'static str, &'static str) {
    let Some(context) = context else {
        return ("absent", "null", "null", "null");
    };
    (
        "present",
        context.token_kind.map_or("null", token_kind_name),
        context.insertion_mode.map_or("null", insertion_mode_name),
        context
            .adjusted_current_node_namespace
            .map_or("null", namespace_name),
    )
}

fn token_kind_name(value: ParserTokenKind) -> &'static str {
    match value {
        ParserTokenKind::Doctype => "doctype",
        ParserTokenKind::StartTag => "start-tag",
        ParserTokenKind::EndTag => "end-tag",
        ParserTokenKind::Character => "character",
        ParserTokenKind::Comment => "comment",
        ParserTokenKind::ProcessingInstruction => "processing-instruction",
        ParserTokenKind::Eof => "eof",
    }
}
pub(crate) fn insertion_mode_name(value: ObservedInsertionMode) -> &'static str {
    match value {
        ObservedInsertionMode::Initial => "initial",
        ObservedInsertionMode::BeforeHtml => "before-html",
        ObservedInsertionMode::BeforeHead => "before-head",
        ObservedInsertionMode::InHead => "in-head",
        ObservedInsertionMode::AfterHead => "after-head",
        ObservedInsertionMode::InBody => "in-body",
        ObservedInsertionMode::AfterBody => "after-body",
        ObservedInsertionMode::AfterAfterBody => "after-after-body",
        ObservedInsertionMode::InTable => "in-table",
        ObservedInsertionMode::InTableText => "in-table-text",
        ObservedInsertionMode::InCaption => "in-caption",
        ObservedInsertionMode::InColumnGroup => "in-column-group",
        ObservedInsertionMode::InTableBody => "in-table-body",
        ObservedInsertionMode::InRow => "in-row",
        ObservedInsertionMode::InCell => "in-cell",
        ObservedInsertionMode::InTemplate => "in-template",
        ObservedInsertionMode::Text => "text",
    }
}
fn namespace_name(value: ElementNamespace) -> &'static str {
    match value {
        ElementNamespace::Html => "html",
        ElementNamespace::Svg => "svg",
        ElementNamespace::MathMl => "mathml",
    }
}

fn valid_code(value: &str) -> bool {
    if let Some(value) = value.strip_prefix("standard:") {
        return matches!(
            value,
            "unexpected-null-character"
                | "eof-before-tag-name"
                | "invalid-first-character-of-tag-name"
                | "missing-end-tag-name"
                | "eof-in-tag"
                | "unexpected-character-in-attribute-name"
                | "unexpected-equals-sign-before-attribute-name"
                | "duplicate-attribute"
                | "unexpected-character-in-unquoted-attribute-value"
                | "missing-attribute-value"
                | "missing-whitespace-between-attributes"
                | "unexpected-solidus-in-tag"
                | "eof-in-comment"
                | "incorrectly-opened-comment"
                | "abrupt-closing-of-empty-comment"
                | "nested-comment"
                | "incorrectly-closed-comment"
                | "eof-in-doctype"
                | "missing-whitespace-before-doctype-name"
                | "missing-doctype-name"
                | "invalid-character-sequence-after-doctype-name"
                | "eof-in-cdata"
                | "end-tag-with-attributes"
                | "end-tag-with-trailing-solidus"
                | "invalid-first-character-of-processing-instruction-target"
                | "invalid-processing-instruction-target"
                | "disallowed-processing-instruction-target"
                | "eof-in-processing-instruction"
                | "missing-semicolon-after-character-reference"
                | "unknown-named-character-reference"
                | "absence-of-digits-in-numeric-character-reference"
                | "null-character-reference"
                | "character-reference-outside-unicode-range"
                | "surrogate-character-reference"
                | "noncharacter-character-reference"
                | "control-character-reference"
        );
    }
    if let Some(value) = value.strip_prefix("tokenizer-extension:") {
        return matches!(
            value,
            "malformed-numeric-character-reference"
                | "dropped-grave-accent-before-attribute-name"
                | "grave-accent-in-attribute-name"
                | "dropped-question-mark-before-attribute-name"
                | "terminated-unquoted-attribute-value-before-question-mark"
        );
    }
    if let Some(value) = value.strip_prefix("tree-construction:") {
        return matches!(
            value,
            "expected-doctype-before-non-space-token"
                | "doctype-token-not-allowed"
                | "start-tag-forbidden-by-active-insertion-mode"
                | "end-tag-forbidden-by-active-insertion-mode"
                | "html-start-tag-after-html-element"
                | "body-start-tag-after-body-element"
                | "token-forbidden-after-body"
                | "token-forbidden-after-after-body"
                | "unacknowledged-self-closing-flag"
                | "element-end-tag-not-in-required-scope"
                | "current-node-mismatch-after-implied-end-tags"
                | "paragraph-end-tag-without-paragraph-in-button-scope"
                | "any-other-end-tag-blocked-by-special-element"
                | "form-start-tag-with-active-form-pointer"
                | "form-end-tag-without-form-element"
                | "form-end-tag-form-element-not-in-scope"
                | "select-start-tag-with-select-in-scope"
                | "select-family-element-remains-after-implied-end-tags"
                | "active-anchor-start-tag"
                | "nobr-start-tag-with-nobr-in-scope"
                | "adoption-formatting-element-missing-from-open-elements"
                | "adoption-formatting-element-not-in-scope"
                | "adoption-formatting-element-not-current-node"
                | "form-start-tag-in-table"
                | "hidden-input-start-tag-in-table"
                | "non-space-character-in-table-text"
                | "non-table-token-in-table"
                | "nested-table-start-tag"
                | "cell-start-tag-without-open-row"
                | "table-context-element-not-in-required-scope"
                | "current-node-not-colgroup"
                | "eof-with-open-template"
                | "eof-in-text-mode"
                | "html-token-not-allowed-in-foreign-content"
                | "null-character-in-foreign-content"
                | "foreign-end-tag-current-node-mismatch"
        );
    }
    false
}
fn valid_recovery(value: &str) -> bool {
    if matches!(
        value,
        "null"
            | "ignore-token"
            | "reprocess-token"
            | "drop-duplicate-attribute"
            | "emit-current-comment-and-switch-to-data"
            | "emit-current-comment-at-eof"
            | "start-bogus-comment"
            | "drop-end-tag-attributes"
            | "ignore-end-tag-trailing-solidus"
            | "preserve-character-reference-literal"
            | "insert-implied-element"
            | "generate-implied-end-tags"
            | "foster-parent"
            | "pop-open-elements"
            | "replace-invalid-input"
            | "ignore-self-closing-flag"
    ) {
        return true;
    }
    [
        "drop-input-character:U+",
        "reconsume-input-character:U+",
        "retain-nested-comment-delimiter-and-reconsume-in-comment-end:U+",
    ]
    .into_iter()
    .find_map(|prefix| value.strip_prefix(prefix))
    .is_some_and(valid_code_point)
}
fn valid_code_point(value: &str) -> bool {
    value.len() == 6
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        && u32::from_str_radix(value, 16)
            .ok()
            .and_then(char::from_u32)
            .is_some()
}
fn valid_position(value: &str) -> bool {
    if value == "unavailable:parser-did-not-provide-position" {
        return true;
    }
    let Some(rest) = value.strip_prefix("normalized-utf8:") else {
        return false;
    };
    let Some((offset, rest)) = rest.split_once(':') else {
        return false;
    };
    let Some((line, rest)) = rest.split_once(':') else {
        return false;
    };
    let Some((column, source)) = rest.split_once(":source-") else {
        return false;
    };
    validate_u64(offset)
        && valid_positive(line)
        && valid_positive(column)
        && (source == "unavailable:no-input-provenance-map"
            || source.strip_prefix("exact:").is_some_and(validate_u64))
}
fn valid_positive(value: &str) -> bool {
    validate_u64(value) && value != "0"
}
fn valid_context(context: &str, token: &str, mode: &str, namespace: &str) -> bool {
    match context {
        "absent" => token == "null" && mode == "null" && namespace == "null",
        "present" => {
            valid_token_kind(token)
                && valid_mode(mode)
                && matches!(namespace, "null" | "html" | "svg" | "mathml")
        }
        _ => false,
    }
}
fn valid_token_kind(value: &str) -> bool {
    matches!(
        value,
        "null"
            | "doctype"
            | "start-tag"
            | "end-tag"
            | "character"
            | "comment"
            | "processing-instruction"
            | "eof"
    )
}
fn valid_mode(value: &str) -> bool {
    matches!(
        value,
        "null"
            | "initial"
            | "before-html"
            | "before-head"
            | "in-head"
            | "after-head"
            | "in-body"
            | "after-body"
            | "after-after-body"
            | "in-table"
            | "in-table-text"
            | "in-caption"
            | "in-column-group"
            | "in-table-body"
            | "in-row"
            | "in-cell"
            | "in-template"
            | "text"
    )
}

fn malformed<T>(line: usize, reason: &'static str) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord { line, reason })
}

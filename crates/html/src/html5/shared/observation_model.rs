//! Parser-owned semantic identities for deterministic conformance observation.
//!
//! These types are always compiled with the HTML5 parser. The public
//! `html::conformance` module re-exports the same definitions behind the
//! non-default `parser-conformance` feature.

use crate::ElementNamespace;
use std::num::NonZeroU64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedTokenAttribute {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedToken {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        force_quirks: bool,
    },
    StartTag {
        name: String,
        attributes: Vec<ObservedTokenAttribute>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Character {
        data: String,
    },
    Comment {
        data: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum ParserStage {
    InputPreprocessing(InputPreprocessingStage),
    Tokenizer,
    TreeConstruction,
    Finalization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum InputPreprocessingStage {
    Utf8Decoding,
    NewlineNormalization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum ParseErrorCode {
    Standard(WhatwgParseErrorCode),
    TokenizerExtension(TokenizerExtensionParseErrorCode),
    TreeConstruction(TreeConstructionParseErrorCode),
}

/// Exact parser-error conditions reported by the supported tokenizer.
///
/// Human-readable descriptions and legacy broad error classes are not
/// authoritative identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum WhatwgParseErrorCode {
    UnexpectedNullCharacter,
    EofBeforeTagName,
    InvalidFirstCharacterOfTagName,
    MissingEndTagName,
    EofInTag,
    UnexpectedCharacterInAttributeName,
    UnexpectedEqualsSignBeforeAttributeName,
    DuplicateAttribute,
    UnexpectedCharacterInUnquotedAttributeValue,
    MissingAttributeValue,
    MissingWhitespaceBetweenAttributes,
    UnexpectedSolidusInTag,
    EofInComment,
    IncorrectlyOpenedComment,
    AbruptClosingOfEmptyComment,
    NestedComment,
    IncorrectlyClosedComment,
    EofInDoctype,
    MissingWhitespaceBeforeDoctypeName,
    MissingDoctypeName,
    InvalidCharacterSequenceAfterDoctypeName,
    EofInCdata,
    EndTagWithAttributes,
    EndTagWithTrailingSolidus,
    InvalidFirstCharacterOfProcessingInstructionTarget,
    InvalidProcessingInstructionTarget,
    DisallowedProcessingInstructionTarget,
    EofInProcessingInstruction,
    MissingSemicolonAfterCharacterReference,
    UnknownNamedCharacterReference,
    AbsenceOfDigitsInNumericCharacterReference,
    NullCharacterReference,
    CharacterReferenceOutsideUnicodeRange,
    SurrogateCharacterReference,
    NoncharacterCharacterReference,
    ControlCharacterReference,
}

/// Exact Borrowser tokenizer recovery conditions that do not have dedicated
/// WHATWG HTML parse-error identities.
///
/// These are never aliases for broad legacy categories. Each variant names one
/// production recovery condition supported by the tokenizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenizerExtensionParseErrorCode {
    /// Borrowser's explicit text-mode session reached EOF while the tokenizer
    /// still had an active RCDATA, RAWTEXT, or script-data control.
    EofInTextMode,
    /// Core-v0's deliberately limited character-reference decoder preserved a
    /// numeric reference containing unsupported trailing syntax.
    MalformedNumericCharacterReference,
    /// Core-v0 drops a grave accent encountered before an attribute name.
    DroppedGraveAccentBeforeAttributeName,
    /// Core-v0 retains a grave accent in an attribute name but reports its
    /// legacy hardening diagnostic.
    GraveAccentInAttributeName,
    /// Core-v0 drops a question mark encountered before an attribute name.
    DroppedQuestionMarkBeforeAttributeName,
    /// Core-v0 terminates an unquoted attribute value before a question mark
    /// instead of appending it to that value.
    TerminatedUnquotedAttributeValueBeforeQuestionMark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum TreeConstructionParseErrorCode {
    UnexpectedDoctypeInBody,
    EndTagElementNotInScope,
    UnmatchedParagraphEndTag,
    NestedFormStartTag,
    NestedSelectStartTag,
    UnexpectedTokenInSelect,
    UnexpectedTokenInTable,
    UnexpectedTokenInTableBody,
    UnexpectedTokenInRow,
    UnexpectedTokenInCell,
    UnexpectedHtmlTokenInForeignContent,
    EofWithOpenTemplate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum ParserRecoveryAction {
    IgnoreToken,
    ReprocessToken,
    DropDuplicateAttribute,
    DropInputCharacter { code_point: char },
    ReconsumeInputCharacter { code_point: char },
    EmitCurrentCommentAndSwitchToData,
    EmitCurrentCommentAtEof,
    StartBogusComment,
    RetainNestedCommentDelimiterAndReconsumeInCommentEnd { code_point: char },
    DropEndTagAttributes,
    IgnoreEndTagTrailingSolidus,
    PreserveCharacterReferenceLiteral,
    InsertImpliedElement,
    GenerateImpliedEndTags,
    FosterParent,
    PopOpenElements,
    ReplaceInvalidInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseErrorEvent {
    pub occurrence: u64,
    pub stage: ParserStage,
    pub code: ParseErrorCode,
    pub recovery: Option<ParserRecoveryAction>,
    pub position: EventPosition,
    pub context: Option<ParserContextSummary>,
    pub description: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Utf8ReplacementReason {
    InvalidSequence,
    IncompleteSequenceAtEof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserResourceLimit {
    TokenBatchCapacity,
    TagNameBytes,
    AttributeNameBytes,
    AttributeValueBytes,
    AttributesPerTag,
    CommentBytes,
    ProcessingInstructionTargetBytes,
    ProcessingInstructionDataBytes,
    DoctypeBytes,
    EndTagMatchScanBytes,
    NumericCharacterReferenceDigits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserGuardrail {
    TokenizerStallRecovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum ImplementationDiagnosticCode {
    InvalidUtf8Replaced(Utf8ReplacementReason),
    ParserResourceLimitActivated(ParserResourceLimit),
    ParserGuardrailActivated(ParserGuardrail),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Utf8ReplacementPayload {
    pub affected_byte_count: NonZeroU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserResourceLimitPayload {
    pub configured_limit: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserGuardrailPayload {
    pub consecutive_stall_steps: NonZeroU64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticEventMetadata {
    pub occurrence: u64,
    pub stage: ParserStage,
    pub position: EventPosition,
    pub context: Option<ParserContextSummary>,
    pub description: Option<&'static str>,
}

/// Payload-safe implementation diagnostic.
///
/// Runtime values are structurally paired with the condition that owns them,
/// so an invalid code/payload combination cannot be constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImplementationDiagnosticEvent {
    InvalidUtf8Replaced {
        metadata: DiagnosticEventMetadata,
        reason: Utf8ReplacementReason,
        payload: Utf8ReplacementPayload,
    },
    ParserResourceLimitActivated {
        metadata: DiagnosticEventMetadata,
        limit: ParserResourceLimit,
        payload: ParserResourceLimitPayload,
    },
    ParserGuardrailActivated {
        metadata: DiagnosticEventMetadata,
        guardrail: ParserGuardrail,
        payload: ParserGuardrailPayload,
    },
}

#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
impl ImplementationDiagnosticEvent {
    pub fn occurrence(&self) -> u64 {
        self.metadata().occurrence
    }

    pub fn code(&self) -> ImplementationDiagnosticCode {
        match self {
            Self::InvalidUtf8Replaced { reason, .. } => {
                ImplementationDiagnosticCode::InvalidUtf8Replaced(*reason)
            }
            Self::ParserResourceLimitActivated { limit, .. } => {
                ImplementationDiagnosticCode::ParserResourceLimitActivated(*limit)
            }
            Self::ParserGuardrailActivated { guardrail, .. } => {
                ImplementationDiagnosticCode::ParserGuardrailActivated(*guardrail)
            }
        }
    }

    pub fn metadata(&self) -> &DiagnosticEventMetadata {
        match self {
            Self::InvalidUtf8Replaced { metadata, .. }
            | Self::ParserResourceLimitActivated { metadata, .. }
            | Self::ParserGuardrailActivated { metadata, .. } => metadata,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventPosition {
    Known(InputPosition),
    Unavailable(PositionUnavailableReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputPosition {
    /// Position in the decoded and newline-normalized parser input.
    pub normalized: NormalizedInputPosition,
    /// Original byte position when, and only when, input provenance exists.
    ///
    /// AE13b1 does not retain a source-byte-to-normalized-input provenance map,
    /// so parser observations use `Unavailable(NoInputProvenanceMap)`.
    pub source_bytes: SourceBytePosition,
}

/// A position in the production parser's normalized Unicode input buffer.
///
/// `utf8_byte_offset` is a zero-based byte offset into the decoded,
/// CR/LF-preprocessed UTF-8 string owned by `html5::Input`; it is never an
/// offset into original fixture or network bytes. `line` and `column` are
/// one-based. A column counts Unicode scalar values from the beginning of the
/// current normalized line, not UTF-8 bytes, UTF-16 code units, or grapheme
/// clusters.
///
/// A non-EOF event identifies the insertion point immediately before the
/// normalized scalar that triggered the event. An EOF event identifies the
/// terminal insertion point after the last normalized scalar. A normalized LF
/// itself is on the preceding line; the next scalar begins at line + 1,
/// column 1.
///
/// CRLF and lone CR are each represented by one normalized LF before these
/// coordinates are assigned. Invalid UTF-8 replacement is likewise reflected
/// only as the resulting U+FFFD scalar, which occupies three bytes in the
/// normalized UTF-8 buffer and one scalar column. These rules make coordinates
/// independent of input delivery chunks. Recovering original byte positions
/// requires a separate provenance map, which AE13b1 deliberately does not add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalizedInputPosition {
    pub space: InputCoordinateSpace,
    /// Zero-based byte offset in normalized parser-input UTF-8.
    pub utf8_byte_offset: u64,
    pub line: NormalizedLineNumber,
    pub column: NormalizedScalarColumn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCoordinateSpace {
    /// Decoded UTF-8 after CRLF and lone-CR preprocessing.
    NormalizedUtf8,
}

/// One-based line number in normalized parser input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NormalizedLineNumber(NonZeroU64);

impl NormalizedLineNumber {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// One-based Unicode-scalar column in normalized parser input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NormalizedScalarColumn(NonZeroU64);

impl NormalizedScalarColumn {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum SourceBytePosition {
    Exact(u64),
    Unavailable(SourcePositionUnavailableReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcePositionUnavailableReason {
    NoInputProvenanceMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionUnavailableReason {
    #[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
    ParserDidNotProvidePosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserContextSummary {
    pub token_kind: Option<ParserTokenKind>,
    pub insertion_mode: Option<ObservedInsertionMode>,
    pub adjusted_current_node_namespace: Option<ElementNamespace>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum ParserTokenKind {
    Doctype,
    StartTag,
    EndTag,
    Character,
    Comment,
    ProcessingInstruction,
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub enum ObservedInsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    AfterBody,
    AfterAfterBody,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InTemplate,
    Text,
}

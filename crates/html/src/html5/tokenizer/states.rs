//! Tokenizer state machine definitions.
//!
//! These states map to the Core v0 tokenizer matrix IDs in
//! `docs/html5/spec-matrix-tokenizer.md`. Most states are currently placeholders
//! and will be implemented incrementally in Milestone E.

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenizerState {
    Data,
    RawText,
    Rcdata,
    ScriptData,
    ScriptDataEscaped,
    ScriptDataEscapedDash,
    ScriptDataEscapedDashDash,
    ScriptDataDoubleEscaped,
    ScriptDataDoubleEscapedDash,
    ScriptDataDoubleEscapedDashDash,
    TagOpen,
    ProcessingInstructionOpen,
    ProcessingInstructionTarget,
    AfterProcessingInstructionTarget,
    ProcessingInstructionData,
    ProcessingInstructionQuestionable,
    EndTagOpen,
    TagName,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    MarkupDeclarationOpen,
    CdataSection,
    CdataSectionBracket,
    CdataSectionEnd,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentEndDash,
    CommentEnd,
    CommentEndBang,
    BogusComment,
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    BogusDoctype,
    CharacterReference,
    NamedCharacterReference,
    AmbiguousAmpersand,
    NumericCharacterReference,
}

impl TokenizerState {
    pub(crate) const fn is_processing_instruction(self) -> bool {
        matches!(
            self,
            Self::ProcessingInstructionOpen
                | Self::ProcessingInstructionTarget
                | Self::AfterProcessingInstructionTarget
                | Self::ProcessingInstructionData
                | Self::ProcessingInstructionQuestionable
        )
    }
}

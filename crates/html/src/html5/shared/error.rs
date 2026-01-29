//! Parse errors for tokenization/tree-building.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseErrorCode {
    UnexpectedNullCharacter,
    UnexpectedEof,
    InvalidCharacterReference,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub code: ParseErrorCode,
    pub position: usize,
}

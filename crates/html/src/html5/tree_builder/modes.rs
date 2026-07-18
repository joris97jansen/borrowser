//! HTML5 insertion modes used by the tree builder state machine.
//!
//! Core v0 implements only the subset needed by the current milestone. The enum
//! is still complete for the v0 contract so callers can assert mode transitions
//! in tests without exposing internal implementation details.

/// HTML5 tree-construction insertion mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum InsertionMode {
    #[default]
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

impl InsertionMode {
    pub(crate) fn digest_tag(self) -> u8 {
        match self {
            Self::Initial => 0,
            Self::BeforeHtml => 1,
            Self::BeforeHead => 2,
            Self::InHead => 3,
            Self::AfterHead => 4,
            Self::InBody => 5,
            Self::AfterBody => 6,
            Self::AfterAfterBody => 7,
            Self::InTable => 8,
            Self::InTableText => 9,
            Self::InCaption => 10,
            Self::InColumnGroup => 11,
            Self::InTableBody => 12,
            Self::InRow => 13,
            Self::InCell => 14,
            Self::InTemplate => 15,
            Self::Text => 16,
        }
    }
}

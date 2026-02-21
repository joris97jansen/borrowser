//! HTML5 insertion modes used by the tree builder state machine.
//!
//! Core v0 implements only the subset needed by the current milestone. The enum
//! is still complete for the v0 contract so callers can assert mode transitions
//! in tests without exposing internal implementation details.

/// HTML5 tree-construction insertion mode (Core v0 subset).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum InsertionMode {
    #[default]
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    Text,
}

mod common;
mod emit;
mod end_tag;
mod script;

pub(crate) use end_tag::{PendingTextModeEndTag, TextModeEndTagMatch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScriptFamilyState {
    ScriptData,
    Escaped,
}

use crate::html5::shared::AtomId;

/// Raw-text family selected by the tree builder after inserting a start tag.
///
/// Contract:
/// - The tree builder enters one of these modes immediately after it inserts the
///   corresponding element and before the tokenizer consumes the next code point.
/// - The tokenizer stays in the selected mode until the tree builder explicitly
///   sends [`TokenizerControl::ExitTextMode`].
/// - Mismatched end tags and other parse errors do not implicitly reset this
///   state; the tree builder remains authoritative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextModeKind {
    RawText,
    Rcdata,
    ScriptData,
}

/// Namespace discriminator for future foreign-content text-mode handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextModeNamespace {
    Html,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextModeMatcher {
    // Core v0/G2 note: only `Style` currently drives tokenizer-side
    // RAWTEXT close-tag recognition. The other variants are reserved so the
    // builder/tokenizer contract stays stable as later milestones add RCDATA
    // and script-specific tokenization.
    Style,
    Title,
    Textarea,
    Script,
}

/// Context required for tokenizer text-mode switching.
///
/// Core v0 carries the text parsing family plus the canonical end-tag name.
/// Namespace is included explicitly so later milestones can extend the contract
/// without introducing hidden side channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextModeSpec {
    pub kind: TextModeKind,
    pub end_tag_name: AtomId,
    pub namespace: TextModeNamespace,
    matcher: TextModeMatcher,
}

impl TextModeSpec {
    pub fn rawtext_style(end_tag_name: AtomId) -> Self {
        Self {
            kind: TextModeKind::RawText,
            end_tag_name,
            namespace: TextModeNamespace::Html,
            matcher: TextModeMatcher::Style,
        }
    }

    pub fn rcdata_title(end_tag_name: AtomId) -> Self {
        Self {
            kind: TextModeKind::Rcdata,
            end_tag_name,
            namespace: TextModeNamespace::Html,
            matcher: TextModeMatcher::Title,
        }
    }

    pub fn rcdata_textarea(end_tag_name: AtomId) -> Self {
        Self {
            kind: TextModeKind::Rcdata,
            end_tag_name,
            namespace: TextModeNamespace::Html,
            matcher: TextModeMatcher::Textarea,
        }
    }

    pub fn script_data(end_tag_name: AtomId) -> Self {
        Self {
            kind: TextModeKind::ScriptData,
            end_tag_name,
            namespace: TextModeNamespace::Html,
            matcher: TextModeMatcher::Script,
        }
    }

    pub(crate) fn rawtext_end_tag_literal(self) -> Option<&'static [u8]> {
        match self.matcher {
            TextModeMatcher::Style => Some(b"style"),
            // Reserved for later text-mode milestones.
            _ => None,
        }
    }
}

/// Explicit control channel from tree builder to tokenizer.
///
/// The runtime must apply these commands between tokens, before it allows the
/// tokenizer to consume additional input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenizerControl {
    EnterTextMode(TextModeSpec),
    ExitTextMode,
}

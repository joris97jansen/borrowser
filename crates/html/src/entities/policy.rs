use std::borrow::Cow;

#[cfg(test)]
use super::minimal::decode_entities_minimal;
use super::minimal::decode_entities_minimal_with_diagnostics;

#[cfg(feature = "html5-entities")]
use super::html5::decode_entities_html5;

/// Internal policy boundary. Minimal is stable and used by default.
/// Html5 is a placeholder for future spec-complete decoding and must not
/// change Minimal behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
enum EntityDecodingPolicy {
    Minimal,
    #[cfg(feature = "html5-entities")]
    #[allow(dead_code)]
    // Until a real parser/tokenizer policy toggle constructs this in non-test code.
    Html5,
}

#[cfg(feature = "html5-entities")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Html5EntityContext {
    Text,
    AttributeValue,
}

/// Tokenizer state context for the active Core-v0 character-reference policy.
///
/// RAWTEXT and script-data callers must not enter this API; they preserve
/// entity-looking text literally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CharacterReferenceContext {
    DataText,
    AttributeValue,
    RcdataText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CharacterReferenceDiagnosticKind {
    UnknownNamed,
    MissingNamedSemicolon,
    MissingNumericSemicolon,
    MissingNumericDigits,
    MalformedNumeric,
    NumericTooLong,
    InvalidNumericScalar,
}

impl CharacterReferenceDiagnosticKind {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::UnknownNamed => "unknown-named-character-reference",
            Self::MissingNamedSemicolon => "missing-semicolon-after-named-character-reference",
            Self::MissingNumericSemicolon => "missing-semicolon-after-numeric-character-reference",
            Self::MissingNumericDigits => "missing-digits-in-numeric-character-reference",
            Self::MalformedNumeric => "malformed-numeric-character-reference",
            Self::NumericTooLong => "numeric-character-reference-too-long",
            Self::InvalidNumericScalar => "invalid-numeric-character-reference",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CharacterReferenceDiagnostic {
    /// Byte offset relative to the string passed to the entity decoder.
    pub(crate) offset: usize,
    pub(crate) kind: CharacterReferenceDiagnosticKind,
    pub(crate) aux: Option<u32>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CharacterReferenceDecode<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) diagnostics: Vec<CharacterReferenceDiagnostic>,
}

/// Decode a minimal, explicitly limited subset of HTML entities.
///
/// Contract:
/// - Named entities decoded: `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;`.
/// - Numeric entities decoded only when well-formed and semicolon-terminated:
///   `&#123;` (decimal) and `&#x1F4A9;` (hex).
/// - Only valid Unicode scalar values decode; invalid scalars pass through unchanged.
/// - Missing semicolons, unknown names, malformed numerics, or overlong digit runs are left
///   unchanged.
/// - Returns a borrowed `Cow` when no `&` is present in the input.
///
/// This is intentionally not HTML5-spec-complete. Keep the behavior narrow and stable.
#[cfg(test)]
pub(crate) fn decode_entities(s: &str) -> Cow<'_, str> {
    decode_entities_with_policy(s, EntityDecodingPolicy::Minimal)
}

pub(crate) fn decode_character_references(
    s: &str,
    context: CharacterReferenceContext,
) -> CharacterReferenceDecode<'_> {
    decode_entities_minimal_with_diagnostics(s, context)
}

#[cfg(test)]
fn decode_entities_with_policy(s: &str, policy: EntityDecodingPolicy) -> Cow<'_, str> {
    match policy {
        EntityDecodingPolicy::Minimal => decode_entities_minimal(s),
        #[cfg(feature = "html5-entities")]
        EntityDecodingPolicy::Html5 => decode_entities_html5(s, Html5EntityContext::Text),
    }
}

#[cfg(feature = "html5-entities")]
#[allow(dead_code)]
// Kept as explicit internal entrypoints for future tokenizer/parser policy wiring.
pub(crate) fn decode_entities_html5_in_text(s: &str) -> Cow<'_, str> {
    decode_entities_html5(s, Html5EntityContext::Text)
}

#[cfg(feature = "html5-entities")]
#[allow(dead_code)]
// Kept as explicit internal entrypoints for future tokenizer/parser policy wiring.
pub(crate) fn decode_entities_html5_in_attribute(s: &str) -> Cow<'_, str> {
    decode_entities_html5(s, Html5EntityContext::AttributeValue)
}

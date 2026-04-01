#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Invariant {
    FullEqualsChunkedDom,
    DecodesNamedEntities,
    DecodesNumericEntities,
    PreservesUtf8Text,
    ScriptRawtextVerbatim,
    AcceptsMixedAttributeSyntax,
    HasDoctypeToken,
    HasCommentToken,
    TagBoundariesStable,
    PartialEntityRemainsLiteral,
    RawtextCloseTagRecognized,
    RawtextNearMatchStaysText,
    CustomTagRecognized,
    NamespacedTagRecognized,
    AttributesParsedWithSpacing,
    EmptyAttributeValuePreserved,
    BooleanAttributePresent,
    StrayEndTagRecovered,
    QuirksTableKeepsOpenP,
    NoQuirksTableClosesOpenP,
}

impl Invariant {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FullEqualsChunkedDom => "full equals chunked dom",
            Self::DecodesNamedEntities => "decodes named entities",
            Self::DecodesNumericEntities => "decodes numeric entities",
            Self::PreservesUtf8Text => "preserves utf-8 text",
            Self::ScriptRawtextVerbatim => "script rawtext verbatim",
            Self::AcceptsMixedAttributeSyntax => "accepts mixed attribute syntax",
            Self::HasDoctypeToken => "has doctype token",
            Self::HasCommentToken => "has comment token",
            Self::TagBoundariesStable => "tag boundaries stable",
            Self::PartialEntityRemainsLiteral => "partial entity remains literal",
            Self::RawtextCloseTagRecognized => "rawtext close tag recognized",
            Self::RawtextNearMatchStaysText => "rawtext near match stays text",
            Self::CustomTagRecognized => "custom tag recognized",
            Self::NamespacedTagRecognized => "namespaced tag recognized",
            Self::AttributesParsedWithSpacing => "attributes parsed with spacing",
            Self::EmptyAttributeValuePreserved => "empty attribute value preserved",
            Self::BooleanAttributePresent => "boolean attribute present",
            Self::StrayEndTagRecovered => "stray end tag recovered",
            Self::QuirksTableKeepsOpenP => "quirks table keeps open p",
            Self::NoQuirksTableClosesOpenP => "no-quirks table closes open p",
        }
    }
}

impl std::fmt::Display for Invariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expectation {
    MustPass,
    AllowedToFail { allowed: &'static [AllowedFailure] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllowedFailure {
    pub invariant: Invariant,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FixtureKind {
    Utf8,
    Entity,
    Attribute,
    Comment,
    Doctype,
    Rawtext,
    TagName,
    Recovery,
    Quirks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ParityCategory {
    SupportedSubsetDom,
    MalformedMarkupRecovery,
    SpecCorrectQuirksBehavior,
}

impl ParityCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SupportedSubsetDom => "supported subset dom structure",
            Self::MalformedMarkupRecovery => "malformed markup recovery",
            Self::SpecCorrectQuirksBehavior => "spec-correct quirks behavior",
        }
    }
}

impl std::fmt::Display for ParityCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyParity {
    MustMatch,
    MayDiffer { reason: &'static str },
}

#[derive(Clone, Copy, Debug)]
pub struct GoldenFixture {
    pub name: &'static str,
    pub input: &'static str,
    pub covers: &'static str,
    pub tags: &'static [&'static str],
    pub invariants: &'static [Invariant],
    pub expectation: Expectation,
    pub kind: FixtureKind,
    pub parity_category: ParityCategory,
    pub legacy_parity: LegacyParity,
}

use crate::model::{DeclarationValue, ValueComponent, ValueSymbol, ValueText, ValueToken};
use crate::selectors::{SelectorListMatchOutcome, Specificity};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Canonical property identifiers in Borrowser's current cascade subset.
///
/// These are the only properties Milestone R resolves through the structured
/// winner-resolution/defaulting pipeline. The set intentionally matches the
/// current engine-facing property surface that later computed-style work already
/// knows how to interpret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum CascadePropertyId {
    BackgroundColor,
    Color,
    Display,
    FontSize,
    Height,
    MarginBottom,
    MarginLeft,
    MarginRight,
    MarginTop,
    MaxWidth,
    MinWidth,
    PaddingBottom,
    PaddingLeft,
    PaddingRight,
    PaddingTop,
    Width,
}

impl CascadePropertyId {
    pub const ALL: [Self; 16] = [
        Self::BackgroundColor,
        Self::Color,
        Self::Display,
        Self::FontSize,
        Self::Height,
        Self::MarginBottom,
        Self::MarginLeft,
        Self::MarginRight,
        Self::MarginTop,
        Self::MaxWidth,
        Self::MinWidth,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::PaddingRight,
        Self::PaddingTop,
        Self::Width,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BackgroundColor => "background-color",
            Self::Color => "color",
            Self::Display => "display",
            Self::FontSize => "font-size",
            Self::Height => "height",
            Self::MarginBottom => "margin-bottom",
            Self::MarginLeft => "margin-left",
            Self::MarginRight => "margin-right",
            Self::MarginTop => "margin-top",
            Self::MaxWidth => "max-width",
            Self::MinWidth => "min-width",
            Self::PaddingBottom => "padding-bottom",
            Self::PaddingLeft => "padding-left",
            Self::PaddingRight => "padding-right",
            Self::PaddingTop => "padding-top",
            Self::Width => "width",
        }
    }

    /// Maps a canonical property name from the model layer into the supported
    /// cascade property subset.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "background-color" => Some(Self::BackgroundColor),
            "color" => Some(Self::Color),
            "display" => Some(Self::Display),
            "font-size" => Some(Self::FontSize),
            "height" => Some(Self::Height),
            "margin-bottom" => Some(Self::MarginBottom),
            "margin-left" => Some(Self::MarginLeft),
            "margin-right" => Some(Self::MarginRight),
            "margin-top" => Some(Self::MarginTop),
            "max-width" => Some(Self::MaxWidth),
            "min-width" => Some(Self::MinWidth),
            "padding-bottom" => Some(Self::PaddingBottom),
            "padding-left" => Some(Self::PaddingLeft),
            "padding-right" => Some(Self::PaddingRight),
            "padding-top" => Some(Self::PaddingTop),
            "width" => Some(Self::Width),
            _ => None,
        }
    }

    pub fn metadata(self) -> CascadePropertyMetadata {
        match self {
            Self::BackgroundColor => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::TransparentColor)
            }
            Self::Color => CascadePropertyMetadata::inherited(InitialStyleValue::ColorBlack),
            Self::Display => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::DisplayInline)
            }
            Self::FontSize => CascadePropertyMetadata::inherited(InitialStyleValue::FontSizePx16),
            Self::Height => CascadePropertyMetadata::not_inherited(InitialStyleValue::AutoKeyword),
            Self::MarginBottom
            | Self::MarginLeft
            | Self::MarginRight
            | Self::MarginTop
            | Self::PaddingBottom
            | Self::PaddingLeft
            | Self::PaddingRight
            | Self::PaddingTop => CascadePropertyMetadata::not_inherited(InitialStyleValue::ZeroPx),
            Self::MaxWidth => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::NoneKeyword)
            }
            Self::MinWidth | Self::Width => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::AutoKeyword)
            }
        }
    }
}

/// Per-property inheritance/default metadata owned by the cascade layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CascadePropertyMetadata {
    pub inheritance: CascadeInheritance,
    pub initial: InitialStyleValue,
}

impl CascadePropertyMetadata {
    pub const fn inherited(initial: InitialStyleValue) -> Self {
        Self {
            inheritance: CascadeInheritance::Inherited,
            initial,
        }
    }

    pub const fn not_inherited(initial: InitialStyleValue) -> Self {
        Self {
            inheritance: CascadeInheritance::NotInherited,
            initial,
        }
    }
}

/// Whether a property inherits when no local winning declaration exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeInheritance {
    Inherited,
    NotInherited,
}

/// Initial/default values for Borrowser's current cascade subset.
///
/// These are cascade-owned defaults, not computed-value normalization results.
/// Later computed-style work remains responsible for parsing authored values,
/// resolving units, and applying any UA-level quirks that stay outside the
/// Milestone R cascade contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitialStyleValue {
    ColorBlack,
    TransparentColor,
    DisplayInline,
    FontSizePx16,
    ZeroPx,
    AutoKeyword,
    NoneKeyword,
}

impl InitialStyleValue {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::ColorBlack => "black",
            Self::TransparentColor => "transparent",
            Self::DisplayInline => "inline",
            Self::FontSizePx16 => "16px",
            Self::ZeroPx => "0px",
            Self::AutoKeyword => "auto",
            Self::NoneKeyword => "none",
        }
    }
}

/// Cascading origin for the current and future Borrowser cascade model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeOrigin {
    UserAgent,
    User,
    Author,
}

/// Importance bucket preserved by the model and consumed by cascade ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeImportance {
    Normal,
    Important,
}

/// Ordered origin/importance band used by winner resolution.
///
/// This ordering preserves the long-term CSS cascade hierarchy Borrowser is
/// growing toward, including reserved bands for animations and transitions that
/// are not emitted by the current Milestone R implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CascadeOriginBand {
    UserAgentNormal,
    UserNormal,
    AuthorNormal,
    Animation,
    AuthorImportant,
    UserImportant,
    UserAgentImportant,
    Transition,
}

impl CascadeOriginBand {
    pub const fn from_origin_and_importance(
        origin: CascadeOrigin,
        importance: CascadeImportance,
    ) -> Self {
        match (origin, importance) {
            (CascadeOrigin::UserAgent, CascadeImportance::Normal) => Self::UserAgentNormal,
            (CascadeOrigin::User, CascadeImportance::Normal) => Self::UserNormal,
            (CascadeOrigin::Author, CascadeImportance::Normal) => Self::AuthorNormal,
            (CascadeOrigin::Author, CascadeImportance::Important) => Self::AuthorImportant,
            (CascadeOrigin::User, CascadeImportance::Important) => Self::UserImportant,
            (CascadeOrigin::UserAgent, CascadeImportance::Important) => Self::UserAgentImportant,
        }
    }

    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::UserAgentNormal => "user-agent-normal",
            Self::UserNormal => "user-normal",
            Self::AuthorNormal => "author-normal",
            Self::Animation => "animation",
            Self::AuthorImportant => "author-important",
            Self::UserImportant => "user-important",
            Self::UserAgentImportant => "user-agent-important",
            Self::Transition => "transition",
        }
    }
}

/// Specificity surface consumed by cascade ordering.
///
/// Stylesheet rules carry selector-derived specificity. Inline style
/// declarations occupy a dedicated top slot within the current author-origin
/// scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CascadeSpecificity {
    Selector(Specificity),
    InlineStyle,
}

/// Fully ordered cascade comparison key for one declaration candidate.
///
/// Comparison is lexicographic by:
/// 1. origin/importance band
/// 2. selector specificity or inline-style sentinel
/// 3. rule order in stylesheet insertion/source order
/// 4. declaration order within the source rule or inline style attribute
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct CascadePriority {
    pub band: CascadeOriginBand,
    pub specificity: CascadeSpecificity,
    pub rule_order: u32,
    pub declaration_order: u32,
}

impl CascadePriority {
    pub const fn new(
        band: CascadeOriginBand,
        specificity: CascadeSpecificity,
        rule_order: u32,
        declaration_order: u32,
    ) -> Self {
        Self {
            band,
            specificity,
            rule_order,
            declaration_order,
        }
    }
}

/// Selector-match handoff shape consumed by the cascade candidate builder for
/// one stylesheet rule against one element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeRuleMatch {
    pub stylesheet_index: u32,
    pub rule_index: u32,
    pub outcome: SelectorListMatchOutcome,
}

impl CascadeRuleMatch {
    pub fn effective_specificity(&self) -> Option<Specificity> {
        self.outcome.highest_specificity()
    }

    pub fn contributes_candidates(&self) -> bool {
        self.outcome.is_matchable() && self.outcome.matched_any()
    }
}

/// Stable source identity for one stylesheet declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StylesheetDeclarationRef {
    pub stylesheet_index: u32,
    pub rule_index: u32,
    pub declaration_index: u32,
}

/// Stable source identity for one inline style declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineStyleDeclarationRef {
    pub declaration_index: u32,
}

/// Source reference for a declaration that survived candidate filtering and won
/// a property in the cascade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationSource {
    Stylesheet(StylesheetDeclarationRef),
    InlineStyle(InlineStyleDeclarationRef),
}

/// Engine-owned specified-value surface carried by authored cascade winners.
///
/// This wraps the structured model-layer declaration value, so downstream
/// computed-style work can consume the winning authored value directly from
/// `ResolvedStyle` without re-looking it up through stylesheet storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeSpecifiedValue {
    value: DeclarationValue,
}

impl CascadeSpecifiedValue {
    pub fn from_declaration_value(value: &DeclarationValue) -> Self {
        Self {
            value: value.clone(),
        }
    }

    pub fn declaration_value(&self) -> &DeclarationValue {
        &self.value
    }

    pub fn to_css_text(&self) -> Option<String> {
        serialize_declaration_value_for_css(&self.value).map(|value| value.trim().to_string())
    }
}

/// One winning declaration selected by cascade ordering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeWinner {
    pub source: CascadeDeclarationSource,
    pub priority: CascadePriority,
    pub value: CascadeSpecifiedValue,
}

/// How one supported property obtained its final resolved value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedValueSource {
    Winner(CascadeWinner),
    Inherited,
    Initial(InitialStyleValue),
}

/// One supported property in a resolved style object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedStyleEntry {
    property: CascadePropertyId,
    source: ResolvedValueSource,
}

impl ResolvedStyleEntry {
    pub fn property(&self) -> CascadePropertyId {
        self.property
    }

    pub fn source(&self) -> &ResolvedValueSource {
        &self.source
    }

    pub fn winner(&self) -> Option<&CascadeWinner> {
        match &self.source {
            ResolvedValueSource::Winner(winner) => Some(winner),
            ResolvedValueSource::Inherited | ResolvedValueSource::Initial(_) => None,
        }
    }
}

/// Deterministic resolved-style surface produced by cascade.
///
/// The final engine is expected to populate every supported property exactly
/// once. Entries are stored in canonical property order rather than insertion
/// order so snapshots and regression tests remain stable. `ResolvedStyle`
/// therefore represents a total final output for the supported property subset,
/// not a sparse intermediate map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedStyle {
    entries: Vec<ResolvedStyleEntry>,
}

impl ResolvedStyle {
    pub fn entries(&self) -> &[ResolvedStyleEntry] {
        &self.entries
    }

    pub fn get(&self, property: CascadePropertyId) -> Option<&ResolvedStyleEntry> {
        self.entries
            .iter()
            .find(|entry| entry.property() == property)
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "resolved-style").expect("write snapshot");
        for entry in &self.entries {
            writeln!(
                &mut out,
                "  {}: {}",
                entry.property.name(),
                source_snapshot_label(entry.source())
            )
            .expect("write snapshot");
        }
        out
    }
}

/// Error returned when a final `ResolvedStyle` is missing supported
/// properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedStyleBuildError {
    missing_properties: Vec<CascadePropertyId>,
}

impl ResolvedStyleBuildError {
    pub fn missing_properties(&self) -> &[CascadePropertyId] {
        &self.missing_properties
    }
}

impl std::fmt::Display for ResolvedStyleBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolved style is missing supported properties: ")?;
        for (index, property) in self.missing_properties.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", property.name())?;
        }
        Ok(())
    }
}

impl std::error::Error for ResolvedStyleBuildError {}

/// Deterministic builder for `ResolvedStyle`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedStyleBuilder {
    entries: BTreeMap<CascadePropertyId, ResolvedValueSource>,
}

impl ResolvedStyleBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_winner(&mut self, property: CascadePropertyId, winner: CascadeWinner) {
        let previous = self
            .entries
            .insert(property, ResolvedValueSource::Winner(winner));
        debug_assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub fn record_inherited(&mut self, property: CascadePropertyId) {
        debug_assert_eq!(
            property.metadata().inheritance,
            CascadeInheritance::Inherited,
            "only inherited properties may resolve through inheritance"
        );
        let previous = self
            .entries
            .insert(property, ResolvedValueSource::Inherited);
        debug_assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub fn record_initial(&mut self, property: CascadePropertyId) {
        let previous = self.entries.insert(
            property,
            ResolvedValueSource::Initial(property.metadata().initial),
        );
        debug_assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub fn build(self) -> Result<ResolvedStyle, ResolvedStyleBuildError> {
        let missing_properties = CascadePropertyId::ALL
            .into_iter()
            .filter(|property| !self.entries.contains_key(property))
            .collect::<Vec<_>>();

        if !missing_properties.is_empty() {
            return Err(ResolvedStyleBuildError { missing_properties });
        }

        Ok(ResolvedStyle {
            entries: self
                .entries
                .into_iter()
                .map(|(property, source)| ResolvedStyleEntry { property, source })
                .collect(),
        })
    }
}

pub(crate) fn serialize_declaration_value_for_css(value: &DeclarationValue) -> Option<String> {
    let mut out = String::new();
    for component in &value.components {
        append_value_component(&mut out, component)?;
    }
    Some(out)
}

fn source_snapshot_label(source: &ResolvedValueSource) -> String {
    match source {
        ResolvedValueSource::Winner(winner) => {
            let value = winner
                .value
                .to_css_text()
                .unwrap_or_else(|| "<unresolved-value>".to_string());
            format!(
                "winner(source={}, band={}, specificity={}, rule-order={}, declaration-order={}, value={})",
                winner_source_label(winner.source),
                winner.priority.band.as_debug_label(),
                specificity_label(winner.priority.specificity),
                winner.priority.rule_order,
                winner.priority.declaration_order,
                quoted_snapshot_text(&value),
            )
        }
        ResolvedValueSource::Inherited => "inherited".to_string(),
        ResolvedValueSource::Initial(initial) => {
            format!("initial({})", initial.as_debug_label())
        }
    }
}

fn winner_source_label(source: CascadeDeclarationSource) -> String {
    match source {
        CascadeDeclarationSource::Stylesheet(source) => format!(
            "stylesheet[{}/{}]/declaration[{}]",
            source.stylesheet_index, source.rule_index, source.declaration_index
        ),
        CascadeDeclarationSource::InlineStyle(source) => {
            format!("inline-style/declaration[{}]", source.declaration_index)
        }
    }
}

fn specificity_label(specificity: CascadeSpecificity) -> String {
    match specificity {
        CascadeSpecificity::Selector(specificity) => format!(
            "selector({},{},{})",
            specificity.ids(),
            specificity.classes(),
            specificity.types()
        ),
        CascadeSpecificity::InlineStyle => "inline-style".to_string(),
    }
}

fn quoted_snapshot_text(text: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn append_value_component(out: &mut String, component: &ValueComponent) -> Option<()> {
    match component {
        ValueComponent::Token(token) => append_value_token(out, token),
        ValueComponent::SimpleBlock(block) => {
            let (open, close) = match block.kind {
                crate::syntax::CssBlockKind::Curly => ('{', '}'),
                crate::syntax::CssBlockKind::Square => ('[', ']'),
                crate::syntax::CssBlockKind::Parenthesis => ('(', ')'),
            };
            out.push(open);
            for component in &block.components {
                append_value_component(out, component)?;
            }
            out.push(close);
            Some(())
        }
        ValueComponent::Function(function) => {
            out.push_str(function.name.text.as_deref()?);
            out.push('(');
            for component in &function.components {
                append_value_component(out, component)?;
            }
            out.push(')');
            Some(())
        }
    }
}

fn append_value_token(out: &mut String, token: &ValueToken) -> Option<()> {
    match token {
        ValueToken::Whitespace { .. } | ValueToken::Comment { .. } => {
            push_ascii_space(out);
            Some(())
        }
        ValueToken::Ident { text, .. } => append_text(out, text),
        ValueToken::AtKeyword { text, .. } => {
            out.push('@');
            append_text(out, text)
        }
        ValueToken::Hash { text, .. } => {
            out.push('#');
            append_text(out, text)
        }
        ValueToken::String { text, .. } => {
            out.push('"');
            append_quoted_text(out, text)?;
            out.push('"');
            Some(())
        }
        ValueToken::BadString { .. } | ValueToken::BadUrl { .. } => None,
        ValueToken::Url { text, .. } => {
            out.push_str("url(");
            append_text(out, text)?;
            out.push(')');
            Some(())
        }
        ValueToken::Delim { value, .. } => {
            out.push(*value);
            Some(())
        }
        ValueToken::Number { text, .. } => append_text(out, text),
        ValueToken::Percentage { text, .. } => {
            append_text(out, text)?;
            out.push('%');
            Some(())
        }
        ValueToken::Dimension { number, unit, .. } => {
            append_text(out, number)?;
            append_text(out, unit)
        }
        ValueToken::UnicodeRange { range, .. } => {
            out.push_str(&format!("U+{:X}-{:X}", range.start(), range.end()));
            Some(())
        }
        ValueToken::Symbol { kind, .. } => {
            out.push_str(match kind {
                ValueSymbol::Colon => ":",
                ValueSymbol::Semicolon => ";",
                ValueSymbol::Comma => ",",
                ValueSymbol::LeftSquareBracket => "[",
                ValueSymbol::RightSquareBracket => "]",
                ValueSymbol::LeftParenthesis => "(",
                ValueSymbol::RightParenthesis => ")",
                ValueSymbol::LeftCurlyBracket => "{",
                ValueSymbol::RightCurlyBracket => "}",
                ValueSymbol::IncludeMatch => "~=",
                ValueSymbol::DashMatch => "|=",
                ValueSymbol::PrefixMatch => "^=",
                ValueSymbol::SuffixMatch => "$=",
                ValueSymbol::SubstringMatch => "*=",
                ValueSymbol::Column => "||",
                ValueSymbol::Cdo => "<!--",
                ValueSymbol::Cdc => "-->",
            });
            Some(())
        }
    }
}

fn append_text(out: &mut String, text: &ValueText) -> Option<()> {
    out.push_str(text.text.as_deref()?);
    Some(())
}

fn append_quoted_text(out: &mut String, text: &ValueText) -> Option<()> {
    for ch in text.text.as_deref()?.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    Some(())
}

fn push_ascii_space(out: &mut String) {
    if !out.chars().last().is_some_and(char::is_whitespace) {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CascadeDeclarationSource, CascadeImportance, CascadeOrigin, CascadeOriginBand,
        CascadePriority, CascadePropertyId, CascadeRuleMatch, CascadeSpecificity,
        CascadeSpecifiedValue, InitialStyleValue, InlineStyleDeclarationRef,
        ResolvedStyleBuildError, ResolvedStyleBuilder, ResolvedValueSource,
        StylesheetDeclarationRef,
    };
    use crate::selectors::{SelectorListMatchOutcome, Specificity};
    use crate::{ParseOptions, Rule, parse_stylesheet_with_options};

    #[test]
    fn supported_property_metadata_matches_current_subset_contract() {
        let color = CascadePropertyId::from_name("color").expect("supported property");
        assert_eq!(color, CascadePropertyId::Color);
        assert_eq!(
            color.metadata().inheritance,
            super::CascadeInheritance::Inherited
        );
        assert_eq!(color.metadata().initial, InitialStyleValue::ColorBlack);

        let display = CascadePropertyId::Display.metadata();
        assert_eq!(display.inheritance, super::CascadeInheritance::NotInherited);
        assert_eq!(display.initial, InitialStyleValue::DisplayInline);
    }

    #[test]
    fn cascade_rule_match_uses_highest_selector_specificity() {
        let mut builder = SelectorListMatchOutcome::builder();
        builder.record_match(0, Specificity::TYPE);
        builder.record_match(2, Specificity::CLASS);

        let rule_match = CascadeRuleMatch {
            stylesheet_index: 0,
            rule_index: 1,
            outcome: builder.build(),
        };

        assert!(rule_match.contributes_candidates());
        assert_eq!(rule_match.effective_specificity(), Some(Specificity::CLASS));
    }

    #[test]
    fn cascade_priority_orders_inline_style_above_selector_specificity() {
        let author_normal = CascadeOriginBand::from_origin_and_importance(
            CascadeOrigin::Author,
            CascadeImportance::Normal,
        );
        let selector_priority = CascadePriority::new(
            author_normal,
            CascadeSpecificity::Selector(Specificity::new(1, 0, 0)),
            4,
            0,
        );
        let inline_priority =
            CascadePriority::new(author_normal, CascadeSpecificity::InlineStyle, 0, 0);

        assert!(inline_priority > selector_priority);
    }

    #[test]
    fn cascade_origin_bands_preserve_future_css_ordering() {
        assert!(CascadeOriginBand::AuthorNormal > CascadeOriginBand::UserNormal);
        assert!(CascadeOriginBand::Animation > CascadeOriginBand::AuthorNormal);
        assert!(CascadeOriginBand::UserImportant > CascadeOriginBand::AuthorImportant);
        assert!(CascadeOriginBand::Transition > CascadeOriginBand::UserAgentImportant);
    }

    #[test]
    fn resolved_style_builder_rejects_missing_supported_properties() {
        let error = ResolvedStyleBuilder::new()
            .build()
            .expect_err("partial style");
        assert_eq!(
            error,
            ResolvedStyleBuildError {
                missing_properties: CascadePropertyId::ALL.to_vec(),
            }
        );
    }

    #[test]
    fn resolved_style_builder_is_deterministic_and_property_sorted() {
        let mut builder =
            builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
        builder.record_winner(
            CascadePropertyId::Display,
            super::CascadeWinner {
                source: CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
                    stylesheet_index: 0,
                    rule_index: 0,
                    declaration_index: 1,
                }),
                priority: CascadePriority::new(
                    CascadeOriginBand::AuthorNormal,
                    CascadeSpecificity::Selector(Specificity::TYPE),
                    0,
                    1,
                ),
                value: parsed_value("display: block"),
            },
        );
        builder.record_inherited(CascadePropertyId::Color);

        let style = builder.build().expect("total style");

        assert_eq!(
            style.entries()[0].property(),
            CascadePropertyId::BackgroundColor
        );
        assert_eq!(style.entries()[1].property(), CascadePropertyId::Color);
        assert_eq!(style.entries()[2].property(), CascadePropertyId::Display);
        assert_eq!(
            style.get(CascadePropertyId::Width).expect("width").source(),
            &ResolvedValueSource::Initial(InitialStyleValue::AutoKeyword)
        );
        assert_eq!(
            style.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "resolved-style\n",
                "  background-color: initial(transparent)\n",
                "  color: inherited\n",
                "  display: winner(source=stylesheet[0/0]/declaration[1], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=1, value=\"block\")\n",
                "  font-size: initial(16px)\n",
                "  height: initial(auto)\n",
                "  margin-bottom: initial(0px)\n",
                "  margin-left: initial(0px)\n",
                "  margin-right: initial(0px)\n",
                "  margin-top: initial(0px)\n",
                "  max-width: initial(none)\n",
                "  min-width: initial(auto)\n",
                "  padding-bottom: initial(0px)\n",
                "  padding-left: initial(0px)\n",
                "  padding-right: initial(0px)\n",
                "  padding-top: initial(0px)\n",
                "  width: initial(auto)\n",
            )
        );
    }

    #[test]
    fn resolved_style_snapshot_formats_inline_winners() {
        let mut builder = builder_with_initials_except(&[CascadePropertyId::Color]);
        builder.record_winner(
            CascadePropertyId::Color,
            super::CascadeWinner {
                source: CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef {
                    declaration_index: 2,
                }),
                priority: CascadePriority::new(
                    CascadeOriginBand::AuthorNormal,
                    CascadeSpecificity::InlineStyle,
                    0,
                    2,
                ),
                value: parsed_value("color: red"),
            },
        );

        let snapshot = builder.build().expect("total style").to_debug_snapshot();
        assert!(snapshot.contains(
            "winner(source=inline-style/declaration[2], band=author-normal, specificity=inline-style, rule-order=0, declaration-order=2, value=\"red\")"
        ));
    }

    fn builder_with_initials_except(skip: &[CascadePropertyId]) -> ResolvedStyleBuilder {
        let mut builder = ResolvedStyleBuilder::new();
        for property in CascadePropertyId::ALL {
            if skip.contains(&property) {
                continue;
            }
            builder.record_initial(property);
        }
        builder
    }

    fn parsed_value(declaration: &str) -> CascadeSpecifiedValue {
        let parse = parse_stylesheet_with_options(
            &format!("div {{ {declaration}; }}"),
            &ParseOptions::stylesheet(),
        );
        let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
            panic!("expected style rule");
        };
        CascadeSpecifiedValue::from_declaration_value(&rule.declarations.declarations[0].value)
    }
}

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

/// Explicit origin/priority model emitted by Borrowser's current CSS scope.
///
/// This is the currently supported cross-product of rule origin and
/// declaration-level importance. Future cascade levels such as animations and
/// transitions remain outside this hot-path type and are integrated through the
/// broader `CascadeOriginBand` ordering model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CurrentScopeCascadePriorityBand {
    UserAgentNormal,
    UserNormal,
    AuthorNormal,
    AuthorImportant,
    UserImportant,
    UserAgentImportant,
}

impl CurrentScopeCascadePriorityBand {
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

    pub const fn as_origin_band(self) -> CascadeOriginBand {
        match self {
            Self::UserAgentNormal => CascadeOriginBand::UserAgentNormal,
            Self::UserNormal => CascadeOriginBand::UserNormal,
            Self::AuthorNormal => CascadeOriginBand::AuthorNormal,
            Self::AuthorImportant => CascadeOriginBand::AuthorImportant,
            Self::UserImportant => CascadeOriginBand::UserImportant,
            Self::UserAgentImportant => CascadeOriginBand::UserAgentImportant,
        }
    }

    /// Debug label for the current emitted priority model.
    ///
    /// The label intentionally matches the corresponding current-scope
    /// `CascadeOriginBand` label. Debug surfaces that need to distinguish the
    /// emitted current-scope model from the broader future precedence space
    /// should do so by carrying the type context explicitly rather than by
    /// expecting different string payloads here.
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::UserAgentNormal => "user-agent-normal",
            Self::UserNormal => "user-normal",
            Self::AuthorNormal => "author-normal",
            Self::AuthorImportant => "author-important",
            Self::UserImportant => "user-important",
            Self::UserAgentImportant => "user-agent-important",
        }
    }
}

/// Ordered origin/importance band used by winner resolution.
///
/// This ordering preserves the long-term CSS cascade hierarchy Borrowser is
/// growing toward. The current engine scope emits only the bands reachable
/// through `CurrentScopeCascadePriorityBand`; animation and transition remain
/// reserved for later milestones.
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
    pub const fn from_current_scope_band(band: CurrentScopeCascadePriorityBand) -> Self {
        band.as_origin_band()
    }

    /// Returns the matching current-scope priority band when this precedence
    /// level is emitted by today's engine path.
    ///
    /// This is an inspection helper, not a total conversion: reserved future
    /// precedence levels such as `Animation` and `Transition` intentionally
    /// return `None`.
    pub const fn current_scope_band(self) -> Option<CurrentScopeCascadePriorityBand> {
        match self {
            Self::UserAgentNormal => Some(CurrentScopeCascadePriorityBand::UserAgentNormal),
            Self::UserNormal => Some(CurrentScopeCascadePriorityBand::UserNormal),
            Self::AuthorNormal => Some(CurrentScopeCascadePriorityBand::AuthorNormal),
            Self::AuthorImportant => Some(CurrentScopeCascadePriorityBand::AuthorImportant),
            Self::UserImportant => Some(CurrentScopeCascadePriorityBand::UserImportant),
            Self::UserAgentImportant => Some(CurrentScopeCascadePriorityBand::UserAgentImportant),
            Self::Animation | Self::Transition => None,
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

    /// Returns the matching current-scope band when this priority was produced
    /// by Borrowser's current emitted origin/priority model.
    ///
    /// This remains an inspection helper only. Priorities built from reserved
    /// future precedence bands are expected to return `None`.
    pub const fn current_scope_band(self) -> Option<CurrentScopeCascadePriorityBand> {
        self.band.current_scope_band()
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

    pub fn rule_ref(&self) -> StylesheetRuleRef {
        StylesheetRuleRef {
            stylesheet_index: self.stylesheet_index,
            rule_index: self.rule_index,
        }
    }
}

/// Stable source identity for one matched stylesheet rule entering cascade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StylesheetRuleRef {
    pub stylesheet_index: u32,
    pub rule_index: u32,
}

impl StylesheetRuleRef {
    pub const fn new(stylesheet_index: u32, rule_index: u32) -> Self {
        Self {
            stylesheet_index,
            rule_index,
        }
    }

    pub fn from_rule_match(rule_match: &CascadeRuleMatch) -> Self {
        rule_match.rule_ref()
    }
}

/// Stable rule-level source identity for one inline style attribute entering
/// cascade.
///
/// The caller assigns a stable per-element scope id within the current style
/// resolution pass so inline styles remain distinguishable in debug surfaces
/// and invariant checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineStyleRuleRef {
    pub scope_id: u32,
}

impl InlineStyleRuleRef {
    pub const fn new(scope_id: u32) -> Self {
        Self { scope_id }
    }
}

/// Stable rule-level source identity for cascade inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeRuleSource {
    Stylesheet(StylesheetRuleRef),
    InlineStyle(InlineStyleRuleRef),
}

impl CascadeRuleSource {
    pub fn from_rule_match(rule_match: &CascadeRuleMatch) -> Self {
        Self::Stylesheet(rule_match.rule_ref())
    }

    fn owns_declaration_source(self, source: CascadeDeclarationSource) -> bool {
        match (self, source) {
            (Self::Stylesheet(rule), CascadeDeclarationSource::Stylesheet(declaration_source)) => {
                rule.stylesheet_index == declaration_source.stylesheet_index
                    && rule.rule_index == declaration_source.rule_index
            }
            (
                Self::InlineStyle(rule),
                CascadeDeclarationSource::InlineStyle(declaration_source),
            ) => rule == declaration_source.inline_style,
            (Self::Stylesheet(_), CascadeDeclarationSource::InlineStyle(_))
            | (Self::InlineStyle(_), CascadeDeclarationSource::Stylesheet(_)) => false,
        }
    }
}

/// Rule-level cascade ordering metadata carried forward from selector matching
/// into declaration-candidate generation.
///
/// The rule context keeps rule-level origin and specificity separate from
/// declaration-level importance. `CascadePriority` and its final
/// `CascadeOriginBand` are synthesized only when a declaration becomes a
/// comparable candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CascadeRuleContext {
    pub origin: CascadeOrigin,
    pub specificity: CascadeSpecificity,
    pub rule_order: u32,
}

impl CascadeRuleContext {
    pub const fn new(
        origin: CascadeOrigin,
        specificity: CascadeSpecificity,
        rule_order: u32,
    ) -> Self {
        Self {
            origin,
            specificity,
            rule_order,
        }
    }

    pub fn from_stylesheet_match(
        origin: CascadeOrigin,
        rule_order: u32,
        rule_match: &CascadeRuleMatch,
    ) -> Option<Self> {
        if !rule_match.contributes_candidates() {
            return None;
        }

        Some(Self::new(
            origin,
            CascadeSpecificity::Selector(rule_match.effective_specificity()?),
            rule_order,
        ))
    }

    pub const fn for_inline_style(rule_order: u32) -> Self {
        Self::new(
            CascadeOrigin::Author,
            CascadeSpecificity::InlineStyle,
            rule_order,
        )
    }

    pub fn priority_for_declaration(
        self,
        importance: CascadeImportance,
        declaration_order: u32,
    ) -> CascadePriority {
        let current_scope_band =
            CurrentScopeCascadePriorityBand::from_origin_and_importance(self.origin, importance);
        CascadePriority::new(
            CascadeOriginBand::from_current_scope_band(current_scope_band),
            self.specificity,
            self.rule_order,
            declaration_order,
        )
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
    pub inline_style: InlineStyleRuleRef,
    pub declaration_index: u32,
}

/// Source reference for a declaration that survived candidate filtering and won
/// a property in the cascade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationSource {
    Stylesheet(StylesheetDeclarationRef),
    InlineStyle(InlineStyleDeclarationRef),
}

/// Structured property surface preserved on one declaration entering cascade.
///
/// This carries the authored property-name category without overloading a loose
/// `Option<String>` into multiple meanings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationProperty {
    Supported(CascadePropertyId),
    Unsupported(String),
    Custom(String),
    Invalid,
}

impl CascadeDeclarationProperty {
    pub fn applicability(&self) -> CascadeDeclarationApplicability {
        match self {
            Self::Supported(property) => CascadeDeclarationApplicability::Supported(*property),
            Self::Unsupported(_) => CascadeDeclarationApplicability::UnsupportedProperty,
            Self::Custom(_) => CascadeDeclarationApplicability::CustomProperty,
            Self::Invalid => CascadeDeclarationApplicability::InvalidPropertyName,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Supported(property) => Some(property.name()),
            Self::Unsupported(name) | Self::Custom(name) => Some(name.as_str()),
            Self::Invalid => None,
        }
    }

    pub fn supported_property(&self) -> Option<CascadePropertyId> {
        match self {
            Self::Supported(property) => Some(*property),
            Self::Unsupported(_) | Self::Custom(_) | Self::Invalid => None,
        }
    }
}

/// Applicability state for one declaration after it has crossed into cascade.
///
/// Only `Supported` declarations generate winner-resolution candidates. The
/// other states remain explicit so filtering stays testable and deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationApplicability {
    Supported(CascadePropertyId),
    UnsupportedProperty,
    CustomProperty,
    InvalidPropertyName,
}

impl CascadeDeclarationApplicability {
    pub fn supported_property(self) -> Option<CascadePropertyId> {
        match self {
            Self::Supported(property) => Some(property),
            Self::UnsupportedProperty | Self::CustomProperty | Self::InvalidPropertyName => None,
        }
    }

    pub fn is_supported(self) -> bool {
        self.supported_property().is_some()
    }
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

/// One declaration attached to a matched cascade rule input.
///
/// This preserves source order, declaration-level importance, applicability
/// state, the structured property-name surface, and the authored value without
/// yet collapsing into a winner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeDeclarationInput {
    source: CascadeDeclarationSource,
    declaration_order: u32,
    importance: CascadeImportance,
    property: CascadeDeclarationProperty,
    value: CascadeSpecifiedValue,
}

impl CascadeDeclarationInput {
    pub fn supported(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property: CascadePropertyId,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Supported(property),
            value,
        }
    }

    pub fn unsupported_property(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property_name: impl Into<String>,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Unsupported(property_name.into()),
            value,
        }
    }

    pub fn custom_property(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property_name: impl Into<String>,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Custom(property_name.into()),
            value,
        }
    }

    pub fn invalid_property_name(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Invalid,
            value,
        }
    }

    pub fn source(&self) -> CascadeDeclarationSource {
        self.source
    }

    pub fn declaration_order(&self) -> u32 {
        self.declaration_order
    }

    pub fn importance(&self) -> CascadeImportance {
        self.importance
    }

    pub fn property(&self) -> &CascadeDeclarationProperty {
        &self.property
    }

    pub fn property_name(&self) -> Option<&str> {
        self.property.name()
    }

    pub fn applicability(&self) -> CascadeDeclarationApplicability {
        self.property.applicability()
    }

    pub fn value(&self) -> &CascadeSpecifiedValue {
        &self.value
    }

    pub fn candidate(&self, context: CascadeRuleContext) -> Option<CascadeDeclarationCandidate> {
        let property = self.property.supported_property()?;

        Some(CascadeDeclarationCandidate {
            property,
            source: self.source,
            priority: context.priority_for_declaration(self.importance, self.declaration_order),
            value: self.value.clone(),
        })
    }
}

/// One matched rule entering cascade with explicit rule-level metadata and
/// ordered declaration inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeRuleInput {
    source: CascadeRuleSource,
    context: CascadeRuleContext,
    declarations: Vec<CascadeDeclarationInput>,
}

/// Error returned when a rule input is built with declarations that do not
/// belong to the claimed rule source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeRuleInputBuildError {
    rule_source: CascadeRuleSource,
    declaration_source: CascadeDeclarationSource,
    declaration_position: usize,
}

impl CascadeRuleInputBuildError {
    pub fn rule_source(&self) -> CascadeRuleSource {
        self.rule_source
    }

    pub fn declaration_source(&self) -> CascadeDeclarationSource {
        self.declaration_source
    }

    pub fn declaration_position(&self) -> usize {
        self.declaration_position
    }
}

impl std::fmt::Display for CascadeRuleInputBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cascade rule input declaration at position {} does not belong to source {:?}: {:?}",
            self.declaration_position, self.rule_source, self.declaration_source
        )
    }
}

impl std::error::Error for CascadeRuleInputBuildError {}

impl CascadeRuleInput {
    pub fn new(
        source: CascadeRuleSource,
        context: CascadeRuleContext,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        if let Some((declaration_position, declaration)) = declarations
            .iter()
            .enumerate()
            .find(|(_, declaration)| !source.owns_declaration_source(declaration.source()))
        {
            return Err(CascadeRuleInputBuildError {
                rule_source: source,
                declaration_source: declaration.source(),
                declaration_position,
            });
        }

        Ok(Self {
            source,
            context,
            declarations,
        })
    }

    pub fn from_stylesheet_match(
        rule_match: &CascadeRuleMatch,
        origin: CascadeOrigin,
        rule_order: u32,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Option<Self>, CascadeRuleInputBuildError> {
        let Some(context) =
            CascadeRuleContext::from_stylesheet_match(origin, rule_order, rule_match)
        else {
            return Ok(None);
        };

        Self::new(
            CascadeRuleSource::from_rule_match(rule_match),
            context,
            declarations,
        )
        .map(Some)
    }

    pub fn from_inline_style(
        inline_style: InlineStyleRuleRef,
        rule_order: u32,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        Self::new(
            CascadeRuleSource::InlineStyle(inline_style),
            CascadeRuleContext::for_inline_style(rule_order),
            declarations,
        )
    }

    pub fn source(&self) -> CascadeRuleSource {
        self.source
    }

    pub fn context(&self) -> CascadeRuleContext {
        self.context
    }

    pub fn declarations(&self) -> &[CascadeDeclarationInput] {
        &self.declarations
    }

    /// Materializes winner-resolution candidates in declaration source order.
    ///
    /// Unsupported, custom-property, and invalid-name declarations remain
    /// present on the rule input but do not emit candidates.
    pub fn candidates(&self) -> Vec<CascadeDeclarationCandidate> {
        self.declarations
            .iter()
            .filter_map(|declaration| declaration.candidate(self.context))
            .collect()
    }
}

/// Sorts declaration candidates into deterministic cascade order.
///
/// The sort is stable by contract, so equal candidate keys preserve their
/// incoming order. That gives later winner-resolution work an explicit,
/// testable behavior for degenerate equal-key cases.
pub fn sort_candidates_by_cascade_order(candidates: &mut [CascadeDeclarationCandidate]) {
    sort_by_candidate_key(candidates, CascadeDeclarationCandidate::sort_key);
}

fn sort_by_candidate_key<T>(
    candidates: &mut [T],
    mut sort_key: impl FnMut(&T) -> CascadeDeclarationCandidateKey,
) {
    candidates.sort_by_key(|candidate| sort_key(candidate));
}

/// Deterministic ordering key for cascade declaration candidates.
///
/// Sorting by this key groups candidates by property and then orders them by
/// the lexicographic cascade precedence defined by `CascadePriority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct CascadeDeclarationCandidateKey {
    pub property: CascadePropertyId,
    pub priority: CascadePriority,
}

/// One supported declaration ready for cascade winner comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeDeclarationCandidate {
    property: CascadePropertyId,
    source: CascadeDeclarationSource,
    priority: CascadePriority,
    value: CascadeSpecifiedValue,
}

impl CascadeDeclarationCandidate {
    pub fn property(&self) -> CascadePropertyId {
        self.property
    }

    pub fn source(&self) -> CascadeDeclarationSource {
        self.source
    }

    pub fn priority(&self) -> CascadePriority {
        self.priority
    }

    pub fn value(&self) -> &CascadeSpecifiedValue {
        &self.value
    }

    pub fn sort_key(&self) -> CascadeDeclarationCandidateKey {
        CascadeDeclarationCandidateKey {
            property: self.property,
            priority: self.priority,
        }
    }

    pub fn to_winner(&self) -> CascadeWinner {
        CascadeWinner {
            source: self.source,
            priority: self.priority,
            value: self.value.clone(),
        }
    }

    pub fn into_winner(self) -> CascadeWinner {
        CascadeWinner {
            source: self.source,
            priority: self.priority,
            value: self.value,
        }
    }
}

/// One winning declaration selected by cascade ordering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeWinner {
    pub source: CascadeDeclarationSource,
    pub priority: CascadePriority,
    pub value: CascadeSpecifiedValue,
}

/// One resolved winning declaration for a supported property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeWinnerEntry {
    property: CascadePropertyId,
    winner: CascadeWinner,
}

impl CascadeWinnerEntry {
    pub fn property(&self) -> CascadePropertyId {
        self.property
    }

    pub fn winner(&self) -> &CascadeWinner {
        &self.winner
    }
}

/// Sparse, deterministic winner-resolution output produced before
/// inheritance/default fill.
///
/// `CascadeWinnerSet` contains only properties with authored winning
/// declarations. Entries are stored in canonical property order, not discovery
/// order, so snapshots and downstream transformations stay stable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CascadeWinnerSet {
    entries: Vec<CascadeWinnerEntry>,
}

impl CascadeWinnerSet {
    pub fn entries(&self) -> &[CascadeWinnerEntry] {
        &self.entries
    }

    pub fn get(&self, property: CascadePropertyId) -> Option<&CascadeWinner> {
        self.entries
            .iter()
            .find(|entry| entry.property() == property)
            .map(|entry| entry.winner())
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "cascade-winners").expect("write snapshot");
        for entry in &self.entries {
            writeln!(
                &mut out,
                "  {}: {}",
                entry.property.name(),
                winner_snapshot_label(entry.winner())
            )
            .expect("write snapshot");
        }
        out
    }
}

/// Resolves one winning authored declaration per supported property from an
/// arbitrary candidate set.
///
/// Candidates are compared by the lexicographic cascade precedence encoded in
/// `CascadeDeclarationCandidateKey`. Equal keys use a deterministic degenerate
/// tie rule: because candidate ordering is stable, the later candidate in the
/// input slice wins.
pub fn resolve_cascade_winners(candidates: &[CascadeDeclarationCandidate]) -> CascadeWinnerSet {
    let mut ordered_candidates = candidates.iter().collect::<Vec<_>>();
    sort_by_candidate_key(&mut ordered_candidates, |candidate| candidate.sort_key());

    let mut entries = Vec::new();
    let mut index = 0;
    while index < ordered_candidates.len() {
        let property = ordered_candidates[index].property();
        let mut winner_index = index;
        while winner_index + 1 < ordered_candidates.len()
            && ordered_candidates[winner_index + 1].property() == property
        {
            winner_index += 1;
        }

        entries.push(CascadeWinnerEntry {
            property,
            winner: ordered_candidates[winner_index].to_winner(),
        });
        index = winner_index + 1;
    }

    CascadeWinnerSet { entries }
}

/// Resolves authored winners directly from matched rule inputs.
///
/// This keeps the rule-input -> candidate -> winner staircase explicit while
/// avoiding any re-derivation of applicability or selector-match semantics.
pub fn resolve_cascade_winners_from_rule_inputs(
    rule_inputs: &[CascadeRuleInput],
) -> CascadeWinnerSet {
    let candidates = rule_inputs
        .iter()
        .flat_map(|rule_input| rule_input.candidates())
        .collect::<Vec<_>>();

    resolve_cascade_winners(&candidates)
}

/// How one supported property obtained its final resolved value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedValueSource {
    Winner(CascadeWinner),
    /// Inherit the value from the parent resolved style.
    ///
    /// This source is emitted only when the property inherits and a parent
    /// resolved style is available for the current element. Root-level fallback
    /// for inherited properties resolves through `Initial(...)` instead.
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

/// Resolves a total `ResolvedStyle` from authored winners plus optional parent
/// resolved style.
///
/// This is the explicit inheritance/default-fill step in Borrowser's cascade
/// pipeline. Local winning authored declarations take precedence. If no local
/// winner exists, inherited properties record `Inherited` when a parent
/// resolved style is present and otherwise fall back to their initial value.
/// Non-inherited properties always fall back to their initial value when no
/// local winner exists.
pub fn resolve_cascade_style(
    winners: &CascadeWinnerSet,
    parent_style: Option<&ResolvedStyle>,
) -> ResolvedStyle {
    let mut builder = ResolvedStyleBuilder::new();

    for property in CascadePropertyId::ALL {
        if let Some(winner) = winners.get(property) {
            builder.record_winner(property, winner.clone());
            continue;
        }

        match (property.metadata().inheritance, parent_style) {
            (CascadeInheritance::Inherited, Some(_)) => builder.record_inherited(property),
            (CascadeInheritance::Inherited, None)
            | (CascadeInheritance::NotInherited, Some(_))
            | (CascadeInheritance::NotInherited, None) => builder.record_initial(property),
        }
    }

    builder
        .build()
        .expect("cascade style resolution must produce a total supported-property output")
}

/// Resolves a total `ResolvedStyle` directly from matched rule inputs plus
/// optional parent resolved style.
///
/// This keeps the rule-input -> winner-set -> resolved-style staircase
/// explicit while offering one current-scope convenience entrypoint for the
/// full Milestone R cascade path.
pub fn resolve_cascade_style_from_rule_inputs(
    rule_inputs: &[CascadeRuleInput],
    parent_style: Option<&ResolvedStyle>,
) -> ResolvedStyle {
    let winners = resolve_cascade_winners_from_rule_inputs(rule_inputs);
    resolve_cascade_style(&winners, parent_style)
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
        assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub fn record_inherited(&mut self, property: CascadePropertyId) {
        assert_eq!(
            property.metadata().inheritance,
            CascadeInheritance::Inherited,
            "only inherited properties may resolve through inheritance"
        );
        let previous = self
            .entries
            .insert(property, ResolvedValueSource::Inherited);
        assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub fn record_initial(&mut self, property: CascadePropertyId) {
        let previous = self.entries.insert(
            property,
            ResolvedValueSource::Initial(property.metadata().initial),
        );
        assert!(
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
        ResolvedValueSource::Winner(winner) => winner_snapshot_label(winner),
        ResolvedValueSource::Inherited => "inherited".to_string(),
        ResolvedValueSource::Initial(initial) => {
            format!("initial({})", initial.as_debug_label())
        }
    }
}

fn winner_snapshot_label(winner: &CascadeWinner) -> String {
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

fn winner_source_label(source: CascadeDeclarationSource) -> String {
    match source {
        CascadeDeclarationSource::Stylesheet(source) => format!(
            "stylesheet[{}/{}]/declaration[{}]",
            source.stylesheet_index, source.rule_index, source.declaration_index
        ),
        CascadeDeclarationSource::InlineStyle(source) => format!(
            "inline-style[{}]/declaration[{}]",
            source.inline_style.scope_id, source.declaration_index
        ),
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
        CascadeDeclarationApplicability, CascadeDeclarationInput, CascadeDeclarationProperty,
        CascadeDeclarationSource, CascadeImportance, CascadeOrigin, CascadeOriginBand,
        CascadePriority, CascadePropertyId, CascadeRuleContext, CascadeRuleInput,
        CascadeRuleInputBuildError, CascadeRuleMatch, CascadeRuleSource, CascadeSpecificity,
        CascadeSpecifiedValue, CascadeWinnerSet, CurrentScopeCascadePriorityBand,
        InitialStyleValue, InlineStyleDeclarationRef, InlineStyleRuleRef, ResolvedStyleBuildError,
        ResolvedStyleBuilder, ResolvedValueSource, StylesheetDeclarationRef, StylesheetRuleRef,
        resolve_cascade_style, resolve_cascade_style_from_rule_inputs, resolve_cascade_winners,
        resolve_cascade_winners_from_rule_inputs, sort_candidates_by_cascade_order,
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
        let author_normal = CurrentScopeCascadePriorityBand::AuthorNormal.as_origin_band();
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
    fn current_scope_priority_bands_map_origin_and_importance_explicitly() {
        assert_eq!(
            CurrentScopeCascadePriorityBand::from_origin_and_importance(
                CascadeOrigin::UserAgent,
                CascadeImportance::Normal
            ),
            CurrentScopeCascadePriorityBand::UserAgentNormal
        );
        assert_eq!(
            CurrentScopeCascadePriorityBand::from_origin_and_importance(
                CascadeOrigin::User,
                CascadeImportance::Normal
            ),
            CurrentScopeCascadePriorityBand::UserNormal
        );
        assert_eq!(
            CurrentScopeCascadePriorityBand::from_origin_and_importance(
                CascadeOrigin::Author,
                CascadeImportance::Normal
            ),
            CurrentScopeCascadePriorityBand::AuthorNormal
        );
        assert_eq!(
            CurrentScopeCascadePriorityBand::from_origin_and_importance(
                CascadeOrigin::Author,
                CascadeImportance::Important
            ),
            CurrentScopeCascadePriorityBand::AuthorImportant
        );
        assert_eq!(
            CurrentScopeCascadePriorityBand::from_origin_and_importance(
                CascadeOrigin::User,
                CascadeImportance::Important
            ),
            CurrentScopeCascadePriorityBand::UserImportant
        );
        assert_eq!(
            CurrentScopeCascadePriorityBand::from_origin_and_importance(
                CascadeOrigin::UserAgent,
                CascadeImportance::Important
            ),
            CurrentScopeCascadePriorityBand::UserAgentImportant
        );
        assert_eq!(
            CurrentScopeCascadePriorityBand::AuthorImportant.as_origin_band(),
            CascadeOriginBand::AuthorImportant
        );
        assert_eq!(
            CascadeOriginBand::UserImportant.current_scope_band(),
            Some(CurrentScopeCascadePriorityBand::UserImportant)
        );
        assert_eq!(CascadeOriginBand::Animation.current_scope_band(), None);
        assert!(
            CurrentScopeCascadePriorityBand::AuthorNormal
                > CurrentScopeCascadePriorityBand::UserNormal
        );
        assert!(
            CurrentScopeCascadePriorityBand::UserImportant
                > CurrentScopeCascadePriorityBand::AuthorImportant
        );
    }

    #[test]
    fn cascade_priority_current_scope_band_is_an_inspection_helper_for_future_bands() {
        let current_scope_priority = CascadePriority::new(
            CascadeOriginBand::AuthorNormal,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
            0,
        );
        let animation_priority = CascadePriority::new(
            CascadeOriginBand::Animation,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
            0,
        );
        let transition_priority = CascadePriority::new(
            CascadeOriginBand::Transition,
            CascadeSpecificity::InlineStyle,
            0,
            0,
        );

        assert_eq!(
            current_scope_priority.current_scope_band(),
            Some(CurrentScopeCascadePriorityBand::AuthorNormal)
        );
        assert_eq!(animation_priority.current_scope_band(), None);
        assert_eq!(transition_priority.current_scope_band(), None);
    }

    #[test]
    fn cascade_origin_bands_preserve_future_css_ordering() {
        assert!(CascadeOriginBand::AuthorNormal > CascadeOriginBand::UserNormal);
        assert!(CascadeOriginBand::Animation > CascadeOriginBand::AuthorNormal);
        assert!(CascadeOriginBand::UserImportant > CascadeOriginBand::AuthorImportant);
        assert!(CascadeOriginBand::Transition > CascadeOriginBand::UserAgentImportant);
    }

    #[test]
    fn cascade_rule_input_materializes_supported_candidates_with_explicit_priority() {
        let rule_match = matched_rule(2, 5, &[Specificity::TYPE, Specificity::CLASS]);
        let source = CascadeRuleSource::Stylesheet(StylesheetRuleRef::from_rule_match(&rule_match));
        let rule = CascadeRuleInput::from_stylesheet_match(
            &rule_match,
            CascadeOrigin::Author,
            11,
            vec![
                CascadeDeclarationInput::supported(
                    stylesheet_declaration_source(2, 5, 0),
                    0,
                    CascadeImportance::Normal,
                    CascadePropertyId::Color,
                    parsed_value("color: red"),
                ),
                CascadeDeclarationInput::supported(
                    stylesheet_declaration_source(2, 5, 1),
                    1,
                    CascadeImportance::Important,
                    CascadePropertyId::Color,
                    parsed_value("color: blue"),
                ),
            ],
        )
        .expect("valid matched stylesheet rule")
        .expect("matched rule contributes");

        let candidates = rule.candidates();
        let context = rule.context();
        assert_eq!(rule.source(), source);
        assert_eq!(rule.context(), context);
        assert_eq!(rule.declarations().len(), 2);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].property(), CascadePropertyId::Color);
        assert_eq!(
            candidates[0].source(),
            stylesheet_declaration_source(2, 5, 0)
        );
        assert_eq!(
            candidates[0].priority(),
            context.priority_for_declaration(CascadeImportance::Normal, 0)
        );
        assert_eq!(
            candidates[1].priority(),
            context.priority_for_declaration(CascadeImportance::Important, 1)
        );
        assert_eq!(candidates[1].value().to_css_text().as_deref(), Some("blue"));

        let winner = candidates[1].to_winner();
        assert_eq!(winner.source, stylesheet_declaration_source(2, 5, 1));
        assert_eq!(
            winner.priority,
            context.priority_for_declaration(CascadeImportance::Important, 1)
        );
        assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    }

    #[test]
    fn cascade_rule_input_keeps_declaration_filter_state_explicit() {
        let inline_style = InlineStyleRuleRef::new(7);
        let rule = CascadeRuleInput::from_inline_style(
            inline_style,
            0,
            vec![
                CascadeDeclarationInput::supported(
                    inline_declaration_source(inline_style, 0),
                    0,
                    CascadeImportance::Normal,
                    CascadePropertyId::Color,
                    parsed_value("color: red"),
                ),
                CascadeDeclarationInput::unsupported_property(
                    inline_declaration_source(inline_style, 1),
                    1,
                    CascadeImportance::Normal,
                    "zoom",
                    parsed_value("zoom: 2"),
                ),
                CascadeDeclarationInput::custom_property(
                    inline_declaration_source(inline_style, 2),
                    2,
                    CascadeImportance::Normal,
                    "--brand",
                    parsed_value("--brand: teal"),
                ),
                CascadeDeclarationInput::invalid_property_name(
                    inline_declaration_source(inline_style, 3),
                    3,
                    CascadeImportance::Normal,
                    parsed_value("color: green"),
                ),
            ],
        )
        .expect("valid inline style rule");
        let context = rule.context();

        assert_eq!(
            rule.declarations()[0].applicability(),
            CascadeDeclarationApplicability::Supported(CascadePropertyId::Color)
        );
        assert_eq!(
            rule.declarations()[0].property(),
            &CascadeDeclarationProperty::Supported(CascadePropertyId::Color)
        );
        assert_eq!(rule.declarations()[1].property_name(), Some("zoom"));
        assert_eq!(
            rule.declarations()[1].applicability(),
            CascadeDeclarationApplicability::UnsupportedProperty
        );
        assert_eq!(
            rule.declarations()[1].property(),
            &CascadeDeclarationProperty::Unsupported("zoom".to_string())
        );
        assert_eq!(rule.declarations()[2].property_name(), Some("--brand"));
        assert_eq!(
            rule.declarations()[2].applicability(),
            CascadeDeclarationApplicability::CustomProperty
        );
        assert_eq!(
            rule.declarations()[2].property(),
            &CascadeDeclarationProperty::Custom("--brand".to_string())
        );
        assert_eq!(rule.declarations()[3].property_name(), None);
        assert_eq!(
            rule.declarations()[3].applicability(),
            CascadeDeclarationApplicability::InvalidPropertyName
        );
        assert_eq!(
            rule.declarations()[3].property(),
            &CascadeDeclarationProperty::Invalid
        );

        let candidates = rule.candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].source(),
            inline_declaration_source(inline_style, 0)
        );
        assert_eq!(
            candidates[0].priority(),
            context.priority_for_declaration(CascadeImportance::Normal, 0)
        );
    }

    #[test]
    fn cascade_rule_input_rejects_declarations_from_a_different_inline_style_source() {
        let inline_style = InlineStyleRuleRef::new(1);
        let other_inline_style = InlineStyleRuleRef::new(2);
        let error = CascadeRuleInput::from_inline_style(
            inline_style,
            0,
            vec![CascadeDeclarationInput::supported(
                inline_declaration_source(other_inline_style, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            )],
        )
        .expect_err("mismatched inline source");

        assert_eq!(
            error,
            CascadeRuleInputBuildError {
                rule_source: CascadeRuleSource::InlineStyle(inline_style),
                declaration_source: inline_declaration_source(other_inline_style, 0),
                declaration_position: 0,
            }
        );
    }

    #[test]
    fn cascade_candidate_sort_key_is_property_first_then_priority() {
        let author_rule = CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::TYPE),
            4,
        );
        let inline_style = InlineStyleRuleRef::new(3);
        let inline_rule = CascadeRuleContext::for_inline_style(0);

        let mut candidates = vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Width,
                parsed_value("width: 10px"),
            )
            .candidate(author_rule)
            .expect("supported candidate"),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 1),
                1,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            )
            .candidate(author_rule)
            .expect("supported candidate"),
            CascadeDeclarationInput::supported(
                inline_declaration_source(inline_style, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            )
            .candidate(inline_rule)
            .expect("supported candidate"),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 0),
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                parsed_value("color: green"),
            )
            .candidate(author_rule)
            .expect("supported candidate"),
        ];

        sort_candidates_by_cascade_order(&mut candidates);

        assert_eq!(candidates[0].property(), CascadePropertyId::Color);
        assert_eq!(candidates[0].value().to_css_text().as_deref(), Some("red"));
        assert_eq!(candidates[1].property(), CascadePropertyId::Color);
        assert_eq!(candidates[1].value().to_css_text().as_deref(), Some("blue"));
        assert_eq!(candidates[2].property(), CascadePropertyId::Color);
        assert_eq!(
            candidates[2].value().to_css_text().as_deref(),
            Some("green")
        );
        assert_eq!(candidates[3].property(), CascadePropertyId::Width);
    }

    #[test]
    fn cascade_candidate_sorting_preserves_incoming_order_for_equal_keys() {
        let context = CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::CLASS),
            4,
        );
        let mut candidates = vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            )
            .candidate(context)
            .expect("supported candidate"),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            )
            .candidate(context)
            .expect("supported candidate"),
        ];

        sort_candidates_by_cascade_order(&mut candidates);

        assert_eq!(candidates[0].value().to_css_text().as_deref(), Some("red"));
        assert_eq!(candidates[1].value().to_css_text().as_deref(), Some("blue"));
    }

    #[test]
    fn cascade_winner_resolution_prefers_higher_specificity_over_later_rule_order() {
        let high_specificity = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::CLASS),
            0,
        ))
        .expect("supported candidate");
        let later_lower_specificity = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::TYPE),
            10,
        ))
        .expect("supported candidate");

        let winners = resolve_cascade_winners(&[later_lower_specificity, high_specificity]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("red"));
        assert_eq!(
            winner.priority.specificity,
            CascadeSpecificity::Selector(Specificity::CLASS)
        );
        assert_eq!(winner.priority.rule_order, 0);
    }

    #[test]
    fn cascade_winner_resolution_prefers_author_over_user_over_user_agent_in_current_normal_scope()
    {
        let user_agent = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: gray"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::UserAgent,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
        ))
        .expect("supported candidate");
        let user = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: green"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::User,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
        ))
        .expect("supported candidate");
        let author = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 2, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
        ))
        .expect("supported candidate");

        let winners = resolve_cascade_winners(&[user, author, user_agent]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("red"));
        assert_eq!(
            winner.priority.current_scope_band(),
            Some(CurrentScopeCascadePriorityBand::AuthorNormal)
        );
    }

    #[test]
    fn cascade_winner_resolution_prefers_important_band_over_higher_specificity_normal_band() {
        let high_specificity_normal = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::new(1, 0, 0)),
            10,
        ))
        .expect("supported candidate");
        let low_specificity_important = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Important,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
        ))
        .expect("supported candidate");

        let winners =
            resolve_cascade_winners(&[high_specificity_normal, low_specificity_important]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
        assert_eq!(
            winner.priority.current_scope_band(),
            Some(CurrentScopeCascadePriorityBand::AuthorImportant)
        );
    }

    #[test]
    fn cascade_winner_resolution_prefers_user_important_over_author_important_in_current_scope() {
        let author_important = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Important,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::new(1, 0, 0)),
            10,
        ))
        .expect("supported candidate");
        let user_important = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Important,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::User,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
        ))
        .expect("supported candidate");

        let winners = resolve_cascade_winners(&[author_important, user_important]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
        assert_eq!(
            winner.priority.current_scope_band(),
            Some(CurrentScopeCascadePriorityBand::UserImportant)
        );
    }

    #[test]
    fn cascade_winner_resolution_prefers_later_rule_order_when_specificity_ties() {
        let earlier_rule = CascadeRuleInput::from_stylesheet_match(
            &matched_rule(0, 0, &[Specificity::CLASS]),
            CascadeOrigin::Author,
            0,
            vec![CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            )],
        )
        .expect("valid rule")
        .expect("matching rule");
        let later_rule = CascadeRuleInput::from_stylesheet_match(
            &matched_rule(0, 1, &[Specificity::CLASS]),
            CascadeOrigin::Author,
            1,
            vec![CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            )],
        )
        .expect("valid rule")
        .expect("matching rule");

        let winners = resolve_cascade_winners_from_rule_inputs(&[later_rule, earlier_rule]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
        assert_eq!(winner.priority.rule_order, 1);
    }

    #[test]
    fn cascade_winner_resolution_prefers_later_declaration_order_within_one_rule() {
        let rule = CascadeRuleInput::from_stylesheet_match(
            &matched_rule(0, 0, &[Specificity::TYPE]),
            CascadeOrigin::Author,
            0,
            vec![
                CascadeDeclarationInput::supported(
                    stylesheet_declaration_source(0, 0, 0),
                    0,
                    CascadeImportance::Normal,
                    CascadePropertyId::Color,
                    parsed_value("color: red"),
                ),
                CascadeDeclarationInput::supported(
                    stylesheet_declaration_source(0, 0, 1),
                    1,
                    CascadeImportance::Normal,
                    CascadePropertyId::Color,
                    parsed_value("color: blue"),
                ),
            ],
        )
        .expect("valid rule")
        .expect("matching rule");

        let winners = resolve_cascade_winners_from_rule_inputs(&[rule]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
        assert_eq!(winner.priority.declaration_order, 1);
    }

    #[test]
    fn cascade_winner_resolution_ignores_unsupported_custom_and_invalid_declarations() {
        let inline_style = InlineStyleRuleRef::new(12);
        let rule = CascadeRuleInput::from_inline_style(
            inline_style,
            0,
            vec![
                CascadeDeclarationInput::unsupported_property(
                    inline_declaration_source(inline_style, 0),
                    0,
                    CascadeImportance::Normal,
                    "zoom",
                    parsed_value("zoom: 2"),
                ),
                CascadeDeclarationInput::custom_property(
                    inline_declaration_source(inline_style, 1),
                    1,
                    CascadeImportance::Normal,
                    "--brand",
                    parsed_value("--brand: teal"),
                ),
                CascadeDeclarationInput::invalid_property_name(
                    inline_declaration_source(inline_style, 2),
                    2,
                    CascadeImportance::Normal,
                    parsed_value("color: green"),
                ),
                CascadeDeclarationInput::supported(
                    inline_declaration_source(inline_style, 3),
                    3,
                    CascadeImportance::Normal,
                    CascadePropertyId::Color,
                    parsed_value("color: red"),
                ),
            ],
        )
        .expect("valid inline rule");

        let winners = resolve_cascade_winners_from_rule_inputs(&[rule]);

        assert_eq!(winners.entries().len(), 1);
        assert_eq!(winners.entries()[0].property(), CascadePropertyId::Color);
        assert_eq!(
            winners.entries()[0].winner().value.to_css_text().as_deref(),
            Some("red")
        );
    }

    #[test]
    fn cascade_winner_resolution_uses_later_input_for_equal_candidate_keys() {
        let context = CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::CLASS),
            4,
        );
        let first = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(context)
        .expect("supported candidate");
        let second = CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(context)
        .expect("supported candidate");

        let winners = resolve_cascade_winners(&[first, second]);
        let winner = winners.get(CascadePropertyId::Color).expect("color winner");

        assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
        assert_eq!(
            winner.priority,
            context.priority_for_declaration(CascadeImportance::Normal, 0)
        );
    }

    #[test]
    fn cascade_winner_set_is_property_sorted_and_snapshot_stable() {
        let winners = resolve_cascade_winners(&[
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Width,
                parsed_value("width: 10px"),
            )
            .candidate(CascadeRuleContext::new(
                CascadeOrigin::Author,
                CascadeSpecificity::Selector(Specificity::TYPE),
                0,
            ))
            .expect("supported candidate"),
            CascadeDeclarationInput::supported(
                inline_declaration_source(InlineStyleRuleRef::new(15), 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            )
            .candidate(CascadeRuleContext::for_inline_style(0))
            .expect("supported candidate"),
        ]);

        assert_eq!(winners.entries()[0].property(), CascadePropertyId::Color);
        assert_eq!(winners.entries()[1].property(), CascadePropertyId::Width);
        assert_eq!(
            winners.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "cascade-winners\n",
                "  color: winner(source=inline-style[15]/declaration[0], band=author-normal, specificity=inline-style, rule-order=0, declaration-order=0, value=\"blue\")\n",
                "  width: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"10px\")\n",
            )
        );
    }

    #[test]
    fn resolve_cascade_style_marks_inherited_properties_only_when_parent_is_present() {
        let mut parent_builder = builder_with_initials_except(&[CascadePropertyId::Color]);
        parent_builder.record_winner(
            CascadePropertyId::Color,
            super::CascadeWinner {
                source: stylesheet_declaration_source(0, 0, 0),
                priority: CascadePriority::new(
                    CascadeOriginBand::AuthorNormal,
                    CascadeSpecificity::Selector(Specificity::TYPE),
                    0,
                    0,
                ),
                value: parsed_value("color: red"),
            },
        );
        let parent_style = parent_builder.build().expect("total parent style");

        let child = resolve_cascade_style(&CascadeWinnerSet::default(), Some(&parent_style));

        assert_eq!(
            child.get(CascadePropertyId::Color).expect("color").source(),
            &ResolvedValueSource::Inherited
        );
        assert_eq!(
            child
                .get(CascadePropertyId::FontSize)
                .expect("font-size")
                .source(),
            &ResolvedValueSource::Inherited
        );
        assert_eq!(
            child
                .get(CascadePropertyId::Display)
                .expect("display")
                .source(),
            &ResolvedValueSource::Initial(InitialStyleValue::DisplayInline)
        );
        assert_eq!(
            child.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "resolved-style\n",
                "  background-color: initial(transparent)\n",
                "  color: inherited\n",
                "  display: initial(inline)\n",
                "  font-size: inherited\n",
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
    fn resolve_cascade_style_uses_initial_for_inherited_properties_at_the_root() {
        let root_style = resolve_cascade_style(&CascadeWinnerSet::default(), None);

        assert_eq!(
            root_style
                .get(CascadePropertyId::Color)
                .expect("color")
                .source(),
            &ResolvedValueSource::Initial(InitialStyleValue::ColorBlack)
        );
        assert_eq!(
            root_style
                .get(CascadePropertyId::FontSize)
                .expect("font-size")
                .source(),
            &ResolvedValueSource::Initial(InitialStyleValue::FontSizePx16)
        );
    }

    #[test]
    fn resolve_cascade_style_explicit_winner_overrides_parent_inheritance_and_defaults() {
        let mut parent_builder =
            builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
        parent_builder.record_winner(
            CascadePropertyId::Color,
            super::CascadeWinner {
                source: stylesheet_declaration_source(0, 0, 0),
                priority: CascadePriority::new(
                    CascadeOriginBand::AuthorNormal,
                    CascadeSpecificity::Selector(Specificity::TYPE),
                    0,
                    0,
                ),
                value: parsed_value("color: red"),
            },
        );
        parent_builder.record_winner(
            CascadePropertyId::Display,
            super::CascadeWinner {
                source: stylesheet_declaration_source(0, 0, 1),
                priority: CascadePriority::new(
                    CascadeOriginBand::AuthorNormal,
                    CascadeSpecificity::Selector(Specificity::TYPE),
                    0,
                    1,
                ),
                value: parsed_value("display: block"),
            },
        );
        let parent_style = parent_builder.build().expect("total parent style");

        let child_winners = resolve_cascade_winners(&[CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::CLASS),
            1,
        ))
        .expect("supported candidate")]);

        let child = resolve_cascade_style(&child_winners, Some(&parent_style));

        assert_eq!(
            child
                .get(CascadePropertyId::Color)
                .and_then(|entry| entry.winner())
                .and_then(|winner| winner.value.to_css_text())
                .as_deref(),
            Some("blue")
        );
        assert_eq!(
            child
                .get(CascadePropertyId::FontSize)
                .expect("font-size")
                .source(),
            &ResolvedValueSource::Inherited
        );
        assert_eq!(
            child
                .get(CascadePropertyId::Display)
                .expect("display")
                .source(),
            &ResolvedValueSource::Initial(InitialStyleValue::DisplayInline)
        );
    }

    #[test]
    fn resolve_cascade_style_from_rule_inputs_applies_inheritance_without_rederiving_priority() {
        let parent_style = resolve_cascade_style(&CascadeWinnerSet::default(), None);
        let child_rule = CascadeRuleInput::from_stylesheet_match(
            &matched_rule(0, 0, &[Specificity::CLASS]),
            CascadeOrigin::Author,
            0,
            vec![CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            )],
        )
        .expect("valid rule")
        .expect("matching rule");

        let child_style =
            resolve_cascade_style_from_rule_inputs(&[child_rule], Some(&parent_style));

        assert_eq!(
            child_style
                .get(CascadePropertyId::Color)
                .and_then(|entry| entry.winner())
                .and_then(|winner| winner.value.to_css_text())
                .as_deref(),
            Some("blue")
        );
        assert_eq!(
            child_style
                .get(CascadePropertyId::FontSize)
                .expect("font-size")
                .source(),
            &ResolvedValueSource::Inherited
        );
        assert_eq!(
            child_style
                .get(CascadePropertyId::BackgroundColor)
                .expect("background-color")
                .source(),
            &ResolvedValueSource::Initial(InitialStyleValue::TransparentColor)
        );
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
    #[should_panic(expected = "resolved style must not record the same property twice")]
    fn resolved_style_builder_rejects_duplicate_property_insertion_in_all_builds() {
        let mut builder = ResolvedStyleBuilder::new();
        builder.record_initial(CascadePropertyId::Color);
        builder.record_initial(CascadePropertyId::Color);
    }

    #[test]
    #[should_panic(expected = "only inherited properties may resolve through inheritance")]
    fn resolved_style_builder_rejects_inherited_source_for_non_inherited_property_in_all_builds() {
        let mut builder = ResolvedStyleBuilder::new();
        builder.record_inherited(CascadePropertyId::Display);
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
                    inline_style: InlineStyleRuleRef::new(9),
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
            "winner(source=inline-style[9]/declaration[2], band=author-normal, specificity=inline-style, rule-order=0, declaration-order=2, value=\"red\")"
        ));
    }

    fn matched_rule(
        stylesheet_index: u32,
        rule_index: u32,
        specificities: &[Specificity],
    ) -> CascadeRuleMatch {
        let mut builder = SelectorListMatchOutcome::builder();
        for (selector_index, specificity) in specificities.iter().copied().enumerate() {
            builder.record_match(selector_index, specificity);
        }
        CascadeRuleMatch {
            stylesheet_index,
            rule_index,
            outcome: builder.build(),
        }
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

    fn stylesheet_declaration_source(
        stylesheet_index: u32,
        rule_index: u32,
        declaration_index: u32,
    ) -> CascadeDeclarationSource {
        CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
            stylesheet_index,
            rule_index,
            declaration_index,
        })
    }

    fn inline_declaration_source(
        inline_style: InlineStyleRuleRef,
        declaration_index: u32,
    ) -> CascadeDeclarationSource {
        CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef {
            inline_style,
            declaration_index,
        })
    }
}

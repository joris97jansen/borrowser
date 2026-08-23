use super::order::{
    DeclarationOrder, DeclarationSourceIndex, RawRuleIndex, StylesheetRuleOrder, StylesheetSourceId,
};
use crate::selectors::{SelectorDomElementId, SelectorListMatchOutcome, Specificity};

use super::priority::{
    CascadeImportance, CascadeOrigin, CascadePriority, CurrentScopeCascadePriorityBand,
};

/// Exact AF4 match result and self-contained rule provenance used by cascade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeRuleMatch {
    rule: StylesheetRuleRef,
    outcome: SelectorListMatchOutcome,
}

impl CascadeRuleMatch {
    pub fn new(rule: StylesheetRuleRef, outcome: SelectorListMatchOutcome) -> Self {
        Self { rule, outcome }
    }

    pub fn effective_specificity(&self) -> Option<Specificity> {
        self.outcome.highest_specificity()
    }

    pub fn contributes_candidates(&self) -> bool {
        self.outcome.is_matchable() && self.outcome.matched_any()
    }

    pub const fn rule_ref(&self) -> StylesheetRuleRef {
        self.rule
    }

    pub fn outcome(&self) -> &SelectorListMatchOutcome {
        &self.outcome
    }
}

/// Self-contained identity for one parsed stylesheet rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StylesheetRuleRef {
    source_id: StylesheetSourceId,
    raw_rule_index: RawRuleIndex,
}

impl StylesheetRuleRef {
    pub const fn new(source_id: StylesheetSourceId, raw_rule_index: RawRuleIndex) -> Self {
        Self {
            source_id,
            raw_rule_index,
        }
    }

    pub const fn source_id(self) -> StylesheetSourceId {
        self.source_id
    }

    pub const fn raw_rule_index(self) -> RawRuleIndex {
        self.raw_rule_index
    }

    #[cfg(test)]
    pub const fn from_rule_match(rule_match: &CascadeRuleMatch) -> Self {
        rule_match.rule_ref()
    }
}

/// Inline-style identity in one selector-DOM/style execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InlineStyleRuleRef {
    Element(SelectorDomElementId),
    Diagnostic,
    #[cfg(test)]
    CompatibilityScope(u32),
}

impl InlineStyleRuleRef {
    pub const fn from_selector_element(element: SelectorDomElementId) -> Self {
        Self::Element(element)
    }

    #[cfg(test)]
    pub(crate) const fn diagnostic() -> Self {
        Self::Diagnostic
    }

    /// Compatibility-only identity for direct cascade contract callers. The
    /// production DOM path uses `from_selector_element`.
    #[cfg(test)]
    pub const fn new(scope_id: u32) -> Self {
        Self::CompatibilityScope(scope_id)
    }

    pub const fn element(self) -> Option<SelectorDomElementId> {
        match self {
            Self::Element(element) => Some(element),
            Self::Diagnostic => None,
            #[cfg(test)]
            Self::CompatibilityScope(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeRuleSource {
    Stylesheet(StylesheetRuleRef),
    InlineStyle(InlineStyleRuleRef),
}

impl CascadeRuleSource {
    pub(crate) fn owns_declaration_source(self, source: CascadeDeclarationSource) -> bool {
        match (self, source) {
            (Self::Stylesheet(rule), CascadeDeclarationSource::Stylesheet(declaration)) => {
                rule.source_id == declaration.source_id
                    && rule.raw_rule_index == declaration.raw_rule_index
            }
            (Self::InlineStyle(rule), CascadeDeclarationSource::InlineStyle(declaration)) => {
                rule == declaration.inline_style
            }
            (Self::Stylesheet(_), CascadeDeclarationSource::InlineStyle(_))
            | (Self::InlineStyle(_), CascadeDeclarationSource::Stylesheet(_)) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeRuleContext {
    Stylesheet {
        origin: CascadeOrigin,
        specificity: Specificity,
        source_order: StylesheetRuleOrder,
    },
    InlineStyle,
}

impl CascadeRuleContext {
    pub const fn for_stylesheet(
        origin: CascadeOrigin,
        specificity: Specificity,
        source_order: StylesheetRuleOrder,
    ) -> Self {
        Self::Stylesheet {
            origin,
            specificity,
            source_order,
        }
    }

    pub fn from_stylesheet_match(
        origin: CascadeOrigin,
        source_order: StylesheetRuleOrder,
        rule_match: &CascadeRuleMatch,
    ) -> Option<Self> {
        if !rule_match.contributes_candidates() {
            return None;
        }
        Some(Self::for_stylesheet(
            origin,
            rule_match.effective_specificity()?,
            source_order,
        ))
    }

    pub const fn for_inline_style() -> Self {
        Self::InlineStyle
    }

    pub const fn origin(self) -> CascadeOrigin {
        match self {
            Self::Stylesheet { origin, .. } => origin,
            Self::InlineStyle => CascadeOrigin::Author,
        }
    }

    pub const fn specificity(self) -> Option<Specificity> {
        match self {
            Self::Stylesheet { specificity, .. } => Some(specificity),
            Self::InlineStyle => None,
        }
    }

    pub const fn source_order(self) -> Option<StylesheetRuleOrder> {
        match self {
            Self::Stylesheet { source_order, .. } => Some(source_order),
            Self::InlineStyle => None,
        }
    }

    pub fn priority_for_declaration(
        self,
        importance: CascadeImportance,
        declaration_order: impl Into<DeclarationOrder>,
    ) -> CascadePriority {
        match self {
            Self::Stylesheet {
                origin,
                specificity,
                source_order,
            } => CascadePriority::for_style_rule(
                CurrentScopeCascadePriorityBand::from_origin_and_importance(origin, importance),
                specificity,
                source_order,
                declaration_order.into(),
            ),
            Self::InlineStyle => {
                CascadePriority::for_element_attached(importance, declaration_order.into())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StylesheetDeclarationRef {
    source_id: StylesheetSourceId,
    raw_rule_index: RawRuleIndex,
    declaration_index: DeclarationSourceIndex,
}

impl StylesheetDeclarationRef {
    pub const fn new(
        source_id: StylesheetSourceId,
        raw_rule_index: RawRuleIndex,
        declaration_index: DeclarationSourceIndex,
    ) -> Self {
        Self {
            source_id,
            raw_rule_index,
            declaration_index,
        }
    }

    pub const fn source_id(self) -> StylesheetSourceId {
        self.source_id
    }

    pub const fn raw_rule_index(self) -> RawRuleIndex {
        self.raw_rule_index
    }

    pub const fn declaration_index(self) -> DeclarationSourceIndex {
        self.declaration_index
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineStyleDeclarationRef {
    inline_style: InlineStyleRuleRef,
    declaration_index: DeclarationSourceIndex,
}

impl InlineStyleDeclarationRef {
    pub const fn new(
        inline_style: InlineStyleRuleRef,
        declaration_index: DeclarationSourceIndex,
    ) -> Self {
        Self {
            inline_style,
            declaration_index,
        }
    }

    pub const fn inline_style(self) -> InlineStyleRuleRef {
        self.inline_style
    }

    pub const fn declaration_index(self) -> DeclarationSourceIndex {
        self.declaration_index
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationSource {
    Stylesheet(StylesheetDeclarationRef),
    InlineStyle(InlineStyleDeclarationRef),
}

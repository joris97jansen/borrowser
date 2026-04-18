use crate::selectors::{SelectorListMatchOutcome, Specificity};

use super::priority::{
    CascadeImportance, CascadeOrigin, CascadeOriginBand, CascadePriority, CascadeSpecificity,
    CurrentScopeCascadePriorityBand,
};

/// Stable source identity and rule-level context for cascade inputs.
///
/// This module owns selector-match handoff, rule/declaration source identity,
/// and rule-level ordering context. It does not own declaration applicability,
/// winner resolution, or resolved-style materialization.
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

    pub(crate) fn owns_declaration_source(self, source: CascadeDeclarationSource) -> bool {
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

/// Source reference for a declaration that survived candidate filtering and
/// won a property in the cascade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationSource {
    Stylesheet(StylesheetDeclarationRef),
    InlineStyle(InlineStyleDeclarationRef),
}

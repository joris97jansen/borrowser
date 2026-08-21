use crate::selectors::SelectorListMatchOutcome;

use super::declarations::CascadeDeclarationInput;
use super::order::StylesheetRuleOrder;
#[cfg(test)]
use super::order::{StyleRulePosition, StylesheetOrder};
#[cfg(test)]
use super::priority::CascadeOrigin;
use super::sources::{
    CascadeDeclarationSource, CascadeRuleContext, CascadeRuleMatch, CascadeRuleSource,
    InlineStyleRuleRef,
};
use super::winners::CascadeDeclarationCandidate;

/// Authoritative matched rule input. Stylesheet declarations are borrowed from
/// the pass-scoped collection arena; inline declarations are owned by the one
/// element whose style attribute was parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeRuleInput<'collection> {
    Stylesheet(MatchedStylesheetRuleInput<'collection>),
    Inline(InlineStyleRuleInput),
    #[cfg(test)]
    Compatibility(CompatibilityRuleInput),
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityRuleInput {
    source: CascadeRuleSource,
    context: CascadeRuleContext,
    declarations: Vec<CascadeDeclarationInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchedStylesheetRuleInput<'collection> {
    rule_ref: super::sources::StylesheetRuleRef,
    rule_match: CascadeRuleMatch,
    context: CascadeRuleContext,
    declarations: &'collection [CascadeDeclarationInput],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineStyleRuleInput {
    source: InlineStyleRuleRef,
    context: CascadeRuleContext,
    declarations: Vec<CascadeDeclarationInput>,
}

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
            "cascade rule input declaration at position {} does not belong to its rule source",
            self.declaration_position
        )
    }
}

impl std::error::Error for CascadeRuleInputBuildError {}

impl<'collection> CascadeRuleInput<'collection> {
    pub(crate) fn from_stylesheet_match_collected(
        rule_ref: super::sources::StylesheetRuleRef,
        origin: super::priority::CascadeOrigin,
        source_order: StylesheetRuleOrder,
        outcome: SelectorListMatchOutcome,
        declarations: &'collection [CascadeDeclarationInput],
    ) -> Result<Option<Self>, CascadeRuleInputBuildError> {
        let rule_match = CascadeRuleMatch::new(rule_ref, outcome);
        let Some(context) =
            CascadeRuleContext::from_stylesheet_match(origin, source_order, &rule_match)
        else {
            return Ok(None);
        };
        let source = CascadeRuleSource::Stylesheet(rule_ref);
        validate_declaration_sources(source, declarations)?;
        Ok(Some(Self::Stylesheet(MatchedStylesheetRuleInput {
            rule_ref,
            rule_match,
            context,
            declarations,
        })))
    }

    pub fn from_inline_style_collected(
        inline_style: InlineStyleRuleRef,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        let source = CascadeRuleSource::InlineStyle(inline_style);
        validate_declaration_sources(source, &declarations)?;
        Ok(Self::Inline(InlineStyleRuleInput {
            source: inline_style,
            context: CascadeRuleContext::for_inline_style(),
            declarations,
        }))
    }

    #[cfg(test)]
    pub fn new(
        source: CascadeRuleSource,
        context: CascadeRuleContext,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        validate_declaration_sources(source, &declarations)?;
        Ok(Self::Compatibility(CompatibilityRuleInput {
            source,
            context,
            declarations,
        }))
    }

    #[cfg(test)]
    pub fn from_stylesheet_match(
        rule_match: &CascadeRuleMatch,
        origin: CascadeOrigin,
        rule_order: u32,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Option<Self>, CascadeRuleInputBuildError> {
        let Some(context) = CascadeRuleContext::from_stylesheet_match(
            origin,
            StylesheetRuleOrder::new(StylesheetOrder::new(0), StyleRulePosition::new(rule_order)),
            rule_match,
        ) else {
            return Ok(None);
        };
        Self::new(
            CascadeRuleSource::Stylesheet(rule_match.rule_ref()),
            context,
            declarations,
        )
        .map(Some)
    }

    #[cfg(test)]
    pub fn from_inline_style(
        inline_style: InlineStyleRuleRef,
        _legacy_rule_order: u32,
        declarations: Vec<CascadeDeclarationInput>,
    ) -> Result<Self, CascadeRuleInputBuildError> {
        Self::new(
            CascadeRuleSource::InlineStyle(inline_style),
            CascadeRuleContext::for_inline_style(),
            declarations,
        )
    }

    pub fn source(&self) -> CascadeRuleSource {
        match self {
            Self::Stylesheet(input) => CascadeRuleSource::Stylesheet(input.rule_ref),
            Self::Inline(input) => CascadeRuleSource::InlineStyle(input.source),
            #[cfg(test)]
            Self::Compatibility(input) => input.source,
        }
    }

    pub fn context(&self) -> CascadeRuleContext {
        match self {
            Self::Stylesheet(input) => input.context,
            Self::Inline(input) => input.context,
            #[cfg(test)]
            Self::Compatibility(input) => input.context,
        }
    }

    pub fn declarations(&self) -> &[CascadeDeclarationInput] {
        match self {
            Self::Stylesheet(input) => input.declarations,
            Self::Inline(input) => &input.declarations,
            #[cfg(test)]
            Self::Compatibility(input) => &input.declarations,
        }
    }

    pub fn stylesheet_match_outcome(&self) -> Option<&SelectorListMatchOutcome> {
        match self {
            Self::Stylesheet(input) => Some(input.rule_match.outcome()),
            Self::Inline(_) => None,
            #[cfg(test)]
            Self::Compatibility(_) => None,
        }
    }

    pub fn stylesheet_rule_order(&self) -> Option<StylesheetRuleOrder> {
        match self {
            Self::Stylesheet(input) => match input.context {
                CascadeRuleContext::Stylesheet { source_order, .. } => Some(source_order),
                CascadeRuleContext::InlineStyle => None,
            },
            Self::Inline(_) => None,
            #[cfg(test)]
            Self::Compatibility(_) => None,
        }
    }

    pub fn candidates(&self) -> Vec<CascadeDeclarationCandidate> {
        self.declarations()
            .iter()
            .filter_map(|declaration| declaration.candidate(self.context()))
            .collect()
    }
}

fn validate_declaration_sources(
    rule_source: CascadeRuleSource,
    declarations: &[CascadeDeclarationInput],
) -> Result<(), CascadeRuleInputBuildError> {
    if let Some((declaration_position, declaration)) = declarations
        .iter()
        .enumerate()
        .find(|(_, declaration)| !rule_source.owns_declaration_source(declaration.source()))
    {
        return Err(CascadeRuleInputBuildError {
            rule_source,
            declaration_source: declaration.source(),
            declaration_position,
        });
    }
    Ok(())
}

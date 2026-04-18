use super::declarations::CascadeDeclarationInput;
use super::priority::CascadeOrigin;
use super::sources::{
    CascadeDeclarationSource, CascadeRuleContext, CascadeRuleMatch, CascadeRuleSource,
    InlineStyleRuleRef,
};
use super::winners::CascadeDeclarationCandidate;

/// Matched rule inputs entering the cascade candidate pipeline.
///
/// This module owns rule-level aggregation and validation of declaration
/// sources against rule sources. It does not own winner ordering or resolved
/// style fill.
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

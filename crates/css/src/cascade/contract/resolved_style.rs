use std::collections::BTreeMap;

use crate::property_registry;
use crate::values::CssWideKeyword;

use super::properties::{CascadeInheritance, CascadePropertyId, InitialStyleValue};
#[cfg(test)]
use super::rules::{CascadeRuleInput, ValidatedCascadeRuleInputs};
#[cfg(test)]
use super::winners::{
    CascadeResolutionBudget, CascadeResolutionWorkspace, resolve_cascade_winners,
};
use super::winners::{CascadeWinner, CascadeWinnerSet};

/// Total specified-value/defaulting source resolution for the current cascade
/// subset.
///
/// This source remains symbolic for inheritance. The computed-value layer
/// later obtains the effective value from the immediate parent's computed
/// style; this artifact never copies a parent winner or specified value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedValueSource {
    Winner(CascadeWinner),
    /// Inherit the value from the immediate parent's computed style.
    ///
    /// This source is emitted only when the property inherits and a parent
    /// exists for the current element. Root-level fallback
    /// for inherited properties resolves through `Initial(...)` instead.
    Inherited,
    Initial(InitialStyleValue),
    CssWideKeyword(CssWideResolvedSource),
}

/// Resolved behavior for one explicit winning CSS-wide keyword declaration.
///
/// This preserves authored winner provenance while making the cascade-owned
/// semantic decision explicit before computed-style materialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssWideResolvedSource {
    Initial {
        keyword: CssWideKeyword,
        winner: CascadeWinner,
        initial: InitialStyleValue,
    },
    Inherited {
        keyword: CssWideKeyword,
        winner: CascadeWinner,
    },
}

impl CssWideResolvedSource {
    pub fn keyword(&self) -> CssWideKeyword {
        match self {
            Self::Initial { keyword, .. } | Self::Inherited { keyword, .. } => *keyword,
        }
    }

    pub fn winner(&self) -> &CascadeWinner {
        match self {
            Self::Initial { winner, .. } | Self::Inherited { winner, .. } => winner,
        }
    }

    pub fn initial(&self) -> Option<InitialStyleValue> {
        match self {
            Self::Initial { initial, .. } => Some(*initial),
            Self::Inherited { .. } => None,
        }
    }
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
            ResolvedValueSource::Inherited
            | ResolvedValueSource::Initial(_)
            | ResolvedValueSource::CssWideKeyword(_) => None,
        }
    }
}

/// Deterministic specified-value/defaulting source-resolution surface.
///
/// The final engine is expected to populate every supported property exactly
/// once. Entries are stored in canonical property order rather than insertion
/// order so snapshots and regression tests remain stable. `ResolvedStyle`
/// therefore represents a total source-resolution output for the supported
/// property subset, not a sparse winner set or a computed style.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedStyle {
    entries: Vec<ResolvedStyleEntry>,
}

/// Whether CSS defaulting has an immediate parent from which computed-value
/// materialization can inherit later.
///
/// The authoritative AF7 classifier receives only this topology fact. It is
/// therefore unable to inspect a parent's cascade winner, resolved source, or
/// specified value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InheritanceParentPresence {
    Absent,
    Present,
}

impl InheritanceParentPresence {
    fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }
}

enum WinnerInput<'a> {
    Borrowed(&'a CascadeWinner),
    Owned(CascadeWinner),
}

impl WinnerInput<'_> {
    fn winner(&self) -> &CascadeWinner {
        match self {
            Self::Borrowed(winner) => winner,
            Self::Owned(winner) => winner,
        }
    }

    fn into_owned(self) -> CascadeWinner {
        match self {
            Self::Borrowed(winner) => winner.clone(),
            Self::Owned(winner) => winner,
        }
    }
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
}

/// Materializes the total initial/default resolved style for the supported
/// cascade subset.
///
/// This is the canonical default style surface for cases where no authored
/// declarations win and no parent computed context contributes inheritance. It
/// is intentionally still a `ResolvedStyle`, not a computed style: values
/// remain cascade-owned initial/default tokens until the computed-value layer
/// consumes them.
pub fn resolve_initial_style() -> ResolvedStyle {
    let mut builder = ResolvedStyleBuilder::new();

    for property in property_registry().ids() {
        builder.record_initial(property);
    }

    builder
        .build()
        .expect("initial style resolution must produce a total supported-property output")
}

/// Resolves a total `ResolvedStyle` from authored winners plus compatibility
/// parent presence.
///
/// This is the explicit inheritance/default-fill step in Borrowser's cascade
/// pipeline. Local winning authored declarations take precedence. If no local
/// winner exists, inherited properties record `Inherited` when an immediate
/// parent is present and otherwise fall back to their initial value.
/// Non-inherited properties always fall back to their initial value when no
/// local winner exists. The parent style's contents are deliberately ignored;
/// only `Some` versus `None` crosses into the authoritative classifier.
pub fn resolve_cascade_style(
    winners: &CascadeWinnerSet,
    parent_style: Option<&ResolvedStyle>,
) -> ResolvedStyle {
    let parent_presence = match parent_style {
        Some(_) => InheritanceParentPresence::Present,
        None => InheritanceParentPresence::Absent,
    };
    resolve_cascade_style_with_parent_presence(winners, parent_presence)
}

pub(crate) fn resolve_cascade_style_with_parent_presence(
    winners: &CascadeWinnerSet,
    parent_presence: InheritanceParentPresence,
) -> ResolvedStyle {
    resolve_cascade_style_with_winners(
        |property| winners.get(property).map(WinnerInput::Borrowed),
        parent_presence,
    )
}

/// Production resolved-style construction that consumes sparse winners so a
/// winning specified value is not cloned again after AF6 materialization.
pub(crate) fn resolve_cascade_style_owned(
    winners: CascadeWinnerSet,
    parent_presence: InheritanceParentPresence,
) -> ResolvedStyle {
    let mut winners = winners.into_entries().peekable();
    let style = resolve_cascade_style_with_winners(
        |property| {
            if winners
                .peek()
                .is_none_or(|entry| entry.property() != property)
            {
                return None;
            }

            let (winner_property, winner) = winners
                .next()
                .expect("peeked cascade winner must remain available")
                .into_parts();
            debug_assert_eq!(winner_property, property);
            Some(WinnerInput::Owned(winner))
        },
        parent_presence,
    );
    debug_assert!(winners.next().is_none());
    style
}

fn resolve_cascade_style_with_winners<'a>(
    mut winner_for_property: impl FnMut(CascadePropertyId) -> Option<WinnerInput<'a>>,
    parent_presence: InheritanceParentPresence,
) -> ResolvedStyle {
    let mut builder = ResolvedStyleBuilder::new();

    for property in property_registry().ids() {
        let source =
            resolve_property_source(property, winner_for_property(property), parent_presence);
        builder.record_source(property, source);
    }

    builder
        .build()
        .expect("cascade style resolution must produce a total supported-property output")
}

fn resolve_property_source(
    property: CascadePropertyId,
    winner: Option<WinnerInput<'_>>,
    parent_presence: InheritanceParentPresence,
) -> ResolvedValueSource {
    let Some(winner) = winner else {
        return match (property.metadata().inheritance, parent_presence) {
            (CascadeInheritance::Inherited, InheritanceParentPresence::Present) => {
                ResolvedValueSource::Inherited
            }
            (CascadeInheritance::Inherited, InheritanceParentPresence::Absent)
            | (CascadeInheritance::NotInherited, InheritanceParentPresence::Present)
            | (CascadeInheritance::NotInherited, InheritanceParentPresence::Absent) => {
                ResolvedValueSource::Initial(property.initial_value())
            }
        };
    };

    let keyword = winner
        .winner()
        .value
        .css_wide_keyword()
        .map(|value| value.keyword());
    let winner = winner.into_owned();

    match keyword {
        None => ResolvedValueSource::Winner(winner),
        Some(CssWideKeyword::Initial) => ResolvedValueSource::CssWideKeyword(
            css_wide_initial_source(property, CssWideKeyword::Initial, winner),
        ),
        Some(CssWideKeyword::Inherit) if parent_presence.is_present() => {
            ResolvedValueSource::CssWideKeyword(css_wide_inherited_source(
                CssWideKeyword::Inherit,
                winner,
            ))
        }
        Some(CssWideKeyword::Inherit) => ResolvedValueSource::CssWideKeyword(
            css_wide_initial_source(property, CssWideKeyword::Inherit, winner),
        ),
        Some(CssWideKeyword::Unset)
            if property.metadata().inheritance == CascadeInheritance::Inherited
                && parent_presence.is_present() =>
        {
            ResolvedValueSource::CssWideKeyword(css_wide_inherited_source(
                CssWideKeyword::Unset,
                winner,
            ))
        }
        Some(CssWideKeyword::Unset) => ResolvedValueSource::CssWideKeyword(
            css_wide_initial_source(property, CssWideKeyword::Unset, winner),
        ),
        Some(keyword @ (CssWideKeyword::Revert | CssWideKeyword::RevertLayer)) => {
            unreachable!(
                "unsupported CSS-wide keyword '{}' must be rejected before AF7 resolution",
                keyword.as_css_keyword()
            )
        }
    }
}

/// Resolves a total `ResolvedStyle` directly from matched rule inputs plus a
/// compatibility parent-presence adapter.
///
/// This keeps the rule-input -> winner-set -> resolved-style staircase
/// explicit while offering one current-scope convenience entrypoint for the
/// full Milestone R cascade path.
#[cfg(test)]
pub fn resolve_cascade_style_from_rule_inputs(
    rule_inputs: &[CascadeRuleInput],
    parent_style: Option<&ResolvedStyle>,
) -> ResolvedStyle {
    let budget = CascadeResolutionBudget::try_new(usize::from(u16::MAX), 1_024, 4_096)
        .expect("test cascade budget is representable");
    let validated =
        ValidatedCascadeRuleInputs::try_from_checked_inputs(rule_inputs.to_vec(), budget)
            .expect("test rule inputs satisfy AF6 invariants");
    let mut workspace =
        CascadeResolutionWorkspace::try_new(budget).expect("test winner workspace reserves");
    let winners = resolve_cascade_winners(&validated, budget, &mut workspace)
        .expect("test cascade winner resolution succeeds");
    let parent_presence = match parent_style {
        Some(_) => InheritanceParentPresence::Present,
        None => InheritanceParentPresence::Absent,
    };
    resolve_cascade_style_owned(winners, parent_presence)
}

/// Error returned when a final `ResolvedStyle` is missing supported
/// properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedStyleBuildError {
    missing_properties: Vec<CascadePropertyId>,
}

impl ResolvedStyleBuildError {
    #[cfg(test)]
    pub(crate) fn missing_properties(&self) -> &[CascadePropertyId] {
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
pub(crate) struct ResolvedStyleBuilder {
    entries: BTreeMap<CascadePropertyId, ResolvedValueSource>,
}

impl ResolvedStyleBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn record_winner(&mut self, property: CascadePropertyId, winner: CascadeWinner) {
        let previous = self
            .entries
            .insert(property, ResolvedValueSource::Winner(winner));
        assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    #[cfg(test)]
    pub(crate) fn record_inherited(&mut self, property: CascadePropertyId) {
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

    pub(crate) fn record_initial(&mut self, property: CascadePropertyId) {
        let previous = self.entries.insert(
            property,
            ResolvedValueSource::Initial(property.initial_value()),
        );
        assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    fn record_source(&mut self, property: CascadePropertyId, source: ResolvedValueSource) {
        let previous = self.entries.insert(property, source);
        assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub(crate) fn build(self) -> Result<ResolvedStyle, ResolvedStyleBuildError> {
        let missing_properties = property_registry()
            .ids()
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

fn css_wide_initial_source(
    property: CascadePropertyId,
    keyword: CssWideKeyword,
    winner: CascadeWinner,
) -> CssWideResolvedSource {
    CssWideResolvedSource::Initial {
        keyword,
        winner,
        initial: property.initial_value(),
    }
}

fn css_wide_inherited_source(
    keyword: CssWideKeyword,
    winner: CascadeWinner,
) -> CssWideResolvedSource {
    CssWideResolvedSource::Inherited { keyword, winner }
}

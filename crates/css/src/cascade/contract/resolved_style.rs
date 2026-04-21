use std::collections::BTreeMap;

use crate::property_registry;

use super::properties::{CascadeInheritance, CascadePropertyId, InitialStyleValue};
use super::rules::CascadeRuleInput;
use super::winners::{CascadeWinner, CascadeWinnerSet, resolve_cascade_winners_from_rule_inputs};

/// Resolved-style output for the current cascade subset.
///
/// This module owns total property fill after winner resolution, including
/// inheritance/default materialization. It does not perform computed-value
/// normalization or layout-facing interpretation.
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
}

/// Materializes the total initial/default resolved style for the supported
/// cascade subset.
///
/// This is the canonical default style surface for cases where no authored
/// declarations win and no parent resolved style contributes inheritance. It
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

    for property in property_registry().ids() {
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
            ResolvedValueSource::Initial(property.initial_value()),
        );
        assert!(
            previous.is_none(),
            "resolved style must not record the same property twice"
        );
    }

    pub fn build(self) -> Result<ResolvedStyle, ResolvedStyleBuildError> {
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

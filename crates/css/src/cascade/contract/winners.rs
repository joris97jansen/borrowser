use super::declarations::CascadeSpecifiedValue;
use super::priority::{CascadeDeclarationCandidateKey, CascadePriority};
use super::properties::CascadePropertyId;
use super::rules::CascadeRuleInput;
use super::sources::CascadeDeclarationSource;

/// Candidate ordering and winner resolution for the cascade pipeline.
///
/// This module owns deterministic candidate sorting and winner materialization.
/// It does not own rule matching, declaration parsing, or resolved-style fill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeDeclarationCandidate {
    property: CascadePropertyId,
    source: CascadeDeclarationSource,
    priority: CascadePriority,
    value: CascadeSpecifiedValue,
}

impl CascadeDeclarationCandidate {
    pub(crate) fn new(
        property: CascadePropertyId,
        source: CascadeDeclarationSource,
        priority: CascadePriority,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            property,
            source,
            priority,
            value,
        }
    }

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

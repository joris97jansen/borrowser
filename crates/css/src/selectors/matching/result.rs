use crate::selectors::{SelectorListParseResult, Specificity};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Matchability state shared by selector parsing and matching surfaces.
///
/// For the current Milestone Q matcher, every parsed selector in Borrowser's
/// supported selector IR is fully matchable. `Unsupported` therefore currently
/// originates from parser-level unsupported selector input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorMatchability {
    Parsed,
    Unsupported,
    Invalid,
}

impl SelectorMatchability {
    pub fn is_parsed(self) -> bool {
        self == Self::Parsed
    }

    pub fn is_unsupported(self) -> bool {
        self == Self::Unsupported
    }

    pub fn is_invalid(self) -> bool {
        self == Self::Invalid
    }

    pub(super) fn as_snapshot_label(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::Unsupported => "unsupported",
            Self::Invalid => "invalid",
        }
    }
}

impl SelectorListParseResult {
    /// Returns whether a selector parse result is matchable by the selector
    /// engine.
    ///
    /// Parsed selector lists are matchable. Unsupported and invalid lists
    /// remain explicit non-matchable states.
    pub fn matchability(&self) -> SelectorMatchability {
        match self {
            Self::Parsed(_) => SelectorMatchability::Parsed,
            Self::Unsupported(_) => SelectorMatchability::Unsupported,
            Self::Invalid(_) => SelectorMatchability::Invalid,
        }
    }
}

/// One selector-list entry that matched a target element.
///
/// `selector_index` is the authoritative source-order identity inside the
/// selector list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchedSelector {
    selector_index: usize,
    specificity: Specificity,
}

impl MatchedSelector {
    pub(crate) fn new(selector_index: usize, specificity: Specificity) -> Self {
        Self {
            selector_index,
            specificity,
        }
    }

    pub fn selector_index(self) -> usize {
        self.selector_index
    }

    pub fn specificity(self) -> Specificity {
        self.specificity
    }
}

/// Deterministic construction path for parsed selector-list match results.
///
/// The selector matcher should use this builder rather than assembling raw
/// `Vec<MatchedSelector>` values. Duplicate selector indices are coalesced at
/// insertion time, and conflicting specificity values remain a debug-time
/// invariant violation.
///
/// Ordering is defined by `selector_index`, not by discovery or insertion
/// order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectorListMatchBuilder {
    matches: BTreeMap<usize, Specificity>,
}

impl SelectorListMatchBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one matched selector from the selector list.
    ///
    /// Returns `true` if this is the first match recorded for the selector
    /// index. Re-recording the same selector index with the same specificity is
    /// a no-op and returns `false`.
    ///
    /// Recording the same selector index with different specificity is invalid
    /// internal state and triggers a debug assertion.
    ///
    /// `selector_index` is the selector list's source-order identity.
    pub fn record_match(&mut self, selector_index: usize, specificity: Specificity) -> bool {
        if let Some(existing) = self.matches.get(&selector_index) {
            debug_assert_eq!(
                *existing, specificity,
                "duplicate selector index must not disagree on specificity"
            );
            return false;
        }

        self.matches.insert(selector_index, specificity);
        true
    }

    pub fn len(&self) -> usize {
        self.matches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Builds a stable selector-list match outcome ordered by `selector_index`
    /// source order rather than by insertion order.
    pub fn build(self) -> SelectorListMatchOutcome {
        SelectorListMatchOutcome::matched(
            self.matches
                .into_iter()
                .map(|(selector_index, specificity)| {
                    MatchedSelector::new(selector_index, specificity)
                })
                .collect(),
        )
    }
}

/// Deterministic match-result surface for one selector list against one target
/// element.
///
/// If `matchability != Parsed`, `matches` is always empty. If `matchability ==
/// Parsed`, `matches` is kept in source order and deduplicated by
/// `selector_index`. `selector_index` is the authoritative source-order
/// identity, not insertion/discovery order. Duplicate selector indices with
/// differing specificity are invalid internal state.
///
/// This is the selector engine's cascade-facing result boundary. It reports
/// explicit selector-list matchability alongside only those selector entries
/// that actually matched the target element, with specificity values taken
/// directly from selector IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorListMatchOutcome {
    matchability: SelectorMatchability,
    matches: Vec<MatchedSelector>,
}

impl SelectorListMatchOutcome {
    pub fn not_matched() -> Self {
        Self {
            matchability: SelectorMatchability::Parsed,
            matches: Vec::new(),
        }
    }

    pub fn builder() -> SelectorListMatchBuilder {
        SelectorListMatchBuilder::new()
    }

    fn matched(matches: Vec<MatchedSelector>) -> Self {
        let mut outcome = Self {
            matchability: SelectorMatchability::Parsed,
            matches,
        };
        outcome.normalize_matches();
        outcome
    }

    pub fn unsupported() -> Self {
        Self {
            matchability: SelectorMatchability::Unsupported,
            matches: Vec::new(),
        }
    }

    pub fn invalid() -> Self {
        Self {
            matchability: SelectorMatchability::Invalid,
            matches: Vec::new(),
        }
    }

    pub fn matchability(&self) -> SelectorMatchability {
        self.matchability
    }

    pub fn is_invalid(&self) -> bool {
        self.matchability.is_invalid()
    }

    pub fn is_unsupported(&self) -> bool {
        self.matchability.is_unsupported()
    }

    pub fn matched_selectors(&self) -> &[MatchedSelector] {
        &self.matches
    }

    pub fn is_matchable(&self) -> bool {
        self.matchability.is_parsed()
    }

    pub fn matched_any(&self) -> bool {
        !self.matches.is_empty()
    }

    pub fn highest_specificity(&self) -> Option<Specificity> {
        self.matches
            .iter()
            .map(|matched| matched.specificity())
            .max()
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "selector-match").expect("write snapshot");
        write_selector_match_outcome_snapshot_body(&mut out, self, 0);
        out
    }

    fn normalize_matches(&mut self) {
        if self.matchability != SelectorMatchability::Parsed {
            self.matches.clear();
            return;
        }

        self.matches.sort_by_key(|matched| matched.selector_index());
        debug_assert_duplicate_match_specificity_consistency(&self.matches);
        self.matches
            .dedup_by_key(|matched| matched.selector_index());
    }
}

fn debug_assert_duplicate_match_specificity_consistency(matches: &[MatchedSelector]) {
    #[cfg(debug_assertions)]
    {
        for pair in matches.windows(2) {
            let left = pair[0];
            let right = pair[1];
            if left.selector_index() == right.selector_index() {
                debug_assert_eq!(
                    left.specificity(),
                    right.specificity(),
                    "duplicate selector index must not disagree on specificity"
                );
            }
        }
    }
}

pub(crate) fn write_selector_match_outcome_snapshot_body(
    out: &mut String,
    outcome: &SelectorListMatchOutcome,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    writeln!(
        out,
        "{indent_str}matchability: {}",
        outcome.matchability.as_snapshot_label()
    )
    .expect("write snapshot");
    writeln!(
        out,
        "{indent_str}matched: {}",
        if outcome.matched_any() { "yes" } else { "no" }
    )
    .expect("write snapshot");

    match outcome.highest_specificity() {
        Some(specificity) => {
            writeln!(
                out,
                "{indent_str}highest-specificity: ({},{},{})",
                specificity.ids(),
                specificity.classes(),
                specificity.types()
            )
            .expect("write snapshot");
        }
        None => writeln!(out, "{indent_str}highest-specificity: none").expect("write snapshot"),
    }

    for (index, matched) in outcome.matches.iter().enumerate() {
        writeln!(
            out,
            "{indent_str}match[{index}]: selector={} specificity=({},{},{})",
            matched.selector_index(),
            matched.specificity().ids(),
            matched.specificity().classes(),
            matched.specificity().types()
        )
        .expect("write snapshot");
    }
}

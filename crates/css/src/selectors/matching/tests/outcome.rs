use super::super::{MatchedSelector, SelectorListMatchBuilder, SelectorListMatchOutcome};
use crate::selectors::Specificity;

#[test]
fn match_builder_coalesces_duplicates_and_builds_stable_outcome() {
    let mut builder = SelectorListMatchBuilder::new();
    assert!(builder.record_match(3, Specificity::new(0, 1, 2)));
    assert!(builder.record_match(1, Specificity::new(1, 0, 0)));
    assert!(!builder.record_match(3, Specificity::new(0, 1, 2)));
    let outcome = builder.build();

    assert_eq!(
        outcome.matched_selectors(),
        &[
            MatchedSelector::new(1, Specificity::new(1, 0, 0)),
            MatchedSelector::new(3, Specificity::new(0, 1, 2)),
        ]
    );
    assert_eq!(
        outcome.highest_specificity(),
        Some(Specificity::new(1, 0, 0))
    );
    assert_eq!(
        outcome.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-match\n",
            "matchability: parsed\n",
            "matched: yes\n",
            "highest-specificity: (1,0,0)\n",
            "match[0]: selector=1 specificity=(1,0,0)\n",
            "match[1]: selector=3 specificity=(0,1,2)\n",
        )
    );
}

#[test]
fn match_builder_orders_results_by_selector_index_not_insertion_order() {
    let mut builder = SelectorListMatchBuilder::new();
    assert!(builder.record_match(5, Specificity::new(0, 0, 1)));
    assert!(builder.record_match(2, Specificity::new(1, 0, 0)));
    assert!(builder.record_match(4, Specificity::new(0, 2, 0)));
    let outcome = builder.build();

    assert_eq!(
        outcome.matched_selectors(),
        &[
            MatchedSelector::new(2, Specificity::new(1, 0, 0)),
            MatchedSelector::new(4, Specificity::new(0, 2, 0)),
            MatchedSelector::new(5, Specificity::new(0, 0, 1)),
        ]
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "duplicate selector index must not disagree on specificity")]
fn match_builder_rejects_duplicate_selector_indexes_with_different_specificity() {
    let mut builder = SelectorListMatchBuilder::new();
    assert!(builder.record_match(2, Specificity::new(0, 1, 0)));
    let _ = builder.record_match(2, Specificity::new(1, 0, 0));
}

#[test]
fn non_matchable_outcomes_never_report_matches() {
    let unsupported = SelectorListMatchOutcome::unsupported();
    let invalid = SelectorListMatchOutcome::invalid();

    assert!(!unsupported.is_matchable());
    assert!(unsupported.is_unsupported());
    assert!(!unsupported.is_invalid());
    assert!(!unsupported.matched_any());
    assert_eq!(unsupported.highest_specificity(), None);
    assert!(!invalid.is_matchable());
    assert!(invalid.is_invalid());
    assert!(!invalid.is_unsupported());
    assert!(!invalid.matched_any());
    assert_eq!(invalid.highest_specificity(), None);
}

#[test]
fn match_outcome_snapshots_keep_validity_and_specificity_state_explicit() {
    assert_eq!(
        SelectorListMatchOutcome::not_matched().to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-match\n",
            "matchability: parsed\n",
            "matched: no\n",
            "highest-specificity: none\n",
        )
    );
    assert_eq!(
        SelectorListMatchOutcome::unsupported().to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-match\n",
            "matchability: unsupported\n",
            "matched: no\n",
            "highest-specificity: none\n",
        )
    );
    assert_eq!(
        SelectorListMatchOutcome::invalid().to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-match\n",
            "matchability: invalid\n",
            "matched: no\n",
            "highest-specificity: none\n",
        )
    );
}

#[test]
fn match_outcome_exposes_builder_for_matcher_construction() {
    let mut builder = SelectorListMatchOutcome::builder();
    assert!(builder.record_match(4, Specificity::new(0, 2, 0)));
    let outcome = builder.build();

    assert_eq!(
        outcome.matched_selectors(),
        &[MatchedSelector::new(4, Specificity::new(0, 2, 0))]
    );
}

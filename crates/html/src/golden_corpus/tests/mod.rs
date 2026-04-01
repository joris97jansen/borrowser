use super::{FixtureKind, Invariant, LegacyParity, ParityCategory, fixtures};
use crate::test_harness::{
    deterministic_chunk_plans, random_chunk_plan, run_chunked_with_output, run_full,
};
use std::collections::{HashMap, HashSet};

mod harness;
mod invariants;
mod metadata_checks;

use harness::{
    derive_seed, fixture_matches_filter, fuzz_fixture_filter, fuzz_mode, fuzz_seed_base,
    fuzz_seed_count, minimize_plan_for_failure, repro_command, run_golden_fixture,
};
use invariants::{InvariantCtx, allowed_failure_reason, check_invariant};
use metadata_checks::assert_fixture_metadata_is_valid;

#[test]
fn golden_corpus_has_metadata() {
    let corpus = fixtures();
    assert!(!corpus.is_empty(), "expected at least one golden fixture");
    let mut names: HashSet<&'static str> = HashSet::new();
    let mut kind_invariants = HashSet::new();
    for fixture in corpus {
        assert_fixture_metadata_is_valid(fixture, &mut names, &mut kind_invariants);
    }
}

#[test]
fn golden_corpus_parity_matrix_is_explicit() {
    let mut saw_must_match = false;
    let mut saw_may_differ = false;
    let mut categories = HashSet::new();

    for fixture in fixtures() {
        categories.insert(fixture.parity_category);
        match fixture.legacy_parity {
            LegacyParity::MustMatch => saw_must_match = true,
            LegacyParity::MayDiffer { .. } => saw_may_differ = true,
        }
    }

    assert!(
        saw_must_match,
        "expected at least one must-match parity fixture"
    );
    assert!(
        saw_may_differ,
        "expected at least one may-differ parity fixture"
    );
    assert!(
        categories.contains(&ParityCategory::SupportedSubsetDom),
        "expected supported-subset parity coverage"
    );
    assert!(
        categories.contains(&ParityCategory::MalformedMarkupRecovery),
        "expected malformed-recovery parity coverage"
    );
    assert!(
        categories.contains(&ParityCategory::SpecCorrectQuirksBehavior),
        "expected quirks-behavior parity coverage"
    );
}

#[test]
fn golden_corpus_v1_runs_across_deterministic_chunk_plans() {
    let mut failures = Vec::new();
    let mut xfails = Vec::new();
    let mut xfail_invariants: HashMap<Invariant, usize> = HashMap::new();
    let mut xfail_kinds: HashMap<FixtureKind, usize> = HashMap::new();
    for fixture in fixtures() {
        let plans = deterministic_chunk_plans(fixture.input);
        let run = run_golden_fixture(fixture, &plans);
        failures.extend(run.failures);
        for entry in run.xfails {
            *xfail_invariants.entry(entry.invariant).or_insert(0) += 1;
            *xfail_kinds.entry(fixture.kind).or_insert(0) += 1;
            xfails.push(entry.message);
        }
    }
    if !xfails.is_empty() {
        eprintln!("XFAIL summary ({} total):", xfails.len());
        for (inv, count) in xfail_invariants {
            eprintln!("  {inv}: {count}");
        }
        for (kind, count) in xfail_kinds {
            eprintln!("  {:?}: {count}", kind);
        }
    }
    if !failures.is_empty() {
        let report = failures.join("\n");
        panic!("golden corpus failures:\n{report}");
    }
}

#[test]
fn golden_corpus_v1_runs_across_random_chunk_plans() {
    let seeds_per_fixture = fuzz_seed_count();
    let base_seed = fuzz_seed_base();
    let fixture_filter = fuzz_fixture_filter();
    let fuzz_mode = fuzz_mode();
    let verbose = std::env::var("BORROWSER_FUZZ_VERBOSE").is_ok();
    let strict_xpass = std::env::var("BORROWSER_STRICT_XPASS").is_ok();
    let mut failures = Vec::new();
    let mut primary_repro = None;
    let mut matched = 0usize;
    let mut xfail_invariants: HashMap<Invariant, usize> = HashMap::new();

    for (fixture_index, fixture) in fixtures().iter().enumerate() {
        if !fixture_matches_filter(fixture.name, fixture_filter.as_deref()) {
            continue;
        }
        matched += 1;
        let full_dom = run_full(fixture.input);
        let fixture_seed = derive_seed(base_seed, fixture.name, fixture_index as u64);
        for offset in 0..seeds_per_fixture {
            let seed = fixture_seed.wrapping_add(offset as u64);
            let fuzz_plan = random_chunk_plan(fixture.input, seed, fuzz_mode);
            let chunked = run_chunked_with_output(fixture.input, &fuzz_plan.plan);
            let ctx = InvariantCtx::new(fixture, &full_dom, &chunked.document);

            for &invariant in fixture.invariants {
                match check_invariant(&ctx, invariant) {
                    Err(message) => {
                        if let Some(reason) = allowed_failure_reason(fixture, invariant) {
                            *xfail_invariants.entry(invariant).or_insert(0) += 1;
                            if verbose {
                                eprintln!(
                                    "XFAIL: {} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: {message} :: {} ({reason})",
                                    fixture.name,
                                    fixture.kind,
                                    fuzz_plan.plan,
                                    invariant,
                                    fuzz_plan.summary
                                );
                            }
                        } else {
                            let repro = repro_command(fixture.name, seed, fuzz_mode);
                            let (minimized, stats) = minimize_plan_for_failure(
                                fixture,
                                invariant,
                                &full_dom,
                                fixture.input,
                                &fuzz_plan.plan,
                            );
                            if primary_repro.is_none() {
                                primary_repro = Some(repro.clone());
                            }
                            failures.push(format!(
                                "{} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: {message} :: {} :: minimized={} :: shrink(orig_boundaries={} orig_chunks={} min_boundaries={} min_chunks={} checks={} policy_upgraded={} budget_exhausted={}) :: {repro}",
                                fixture.name,
                                fixture.kind,
                                fuzz_plan.plan,
                                invariant,
                                fuzz_plan.summary,
                                minimized,
                                stats.original_boundaries,
                                stats.original_chunks,
                                stats.minimized_boundaries,
                                stats.minimized_chunks,
                                stats.checks,
                                stats.policy_upgraded,
                                stats.budget_exhausted
                            ));
                        }
                    }
                    Ok(()) => {
                        if let Some(reason) = allowed_failure_reason(fixture, invariant) {
                            if strict_xpass {
                                let repro = repro_command(fixture.name, seed, fuzz_mode);
                                if primary_repro.is_none() {
                                    primary_repro = Some(repro.clone());
                                }
                                failures.push(format!(
                                    "{} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: XPASS (allowed to fail: {reason}) :: {} :: {repro}",
                                    fixture.name,
                                    fixture.kind,
                                    fuzz_plan.plan,
                                    invariant,
                                    fuzz_plan.summary
                                ));
                            } else if verbose {
                                eprintln!(
                                    "XPASS: {} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: {reason} :: {}",
                                    fixture.name,
                                    fixture.kind,
                                    fuzz_plan.plan,
                                    invariant,
                                    fuzz_plan.summary
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(filter) = fixture_filter.as_deref()
        && matched == 0
    {
        panic!("no fixtures matched BORROWSER_FUZZ_FIXTURE={filter}");
    }
    if !xfail_invariants.is_empty() {
        eprintln!(
            "XFAIL summary ({} total):",
            xfail_invariants.values().sum::<usize>()
        );
        for (inv, count) in xfail_invariants {
            eprintln!("  {inv}: {count}");
        }
    }
    if !failures.is_empty() {
        let report = failures.join("\n");
        if let Some(repro) = primary_repro {
            panic!("golden corpus random-chunk failures:\n{repro}\n{report}");
        }
        panic!("golden corpus random-chunk failures:\n{report}");
    }
}

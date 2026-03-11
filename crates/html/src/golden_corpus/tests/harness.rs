use super::super::{GoldenFixture, Invariant};
use super::invariants::{InvariantCtx, allowed_failure_reason, check_invariant};
use crate::Node;
use crate::test_harness::{
    ChunkPlan, FuzzMode, ShrinkStats, run_chunked_with_tokens, run_full,
    shrink_chunk_plan_with_stats,
};

pub(super) struct FixtureRun {
    pub(super) failures: Vec<String>,
    pub(super) xfails: Vec<XfailEntry>,
}

pub(super) struct XfailEntry {
    pub(super) invariant: Invariant,
    pub(super) message: String,
}

pub(super) fn run_golden_fixture(fixture: &GoldenFixture, plans: &[ChunkPlan]) -> FixtureRun {
    let mut failures = Vec::new();
    let mut xfails = Vec::new();
    let strict_xpass = std::env::var("BORROWSER_STRICT_XPASS").is_ok();
    let full_dom = run_full(fixture.input);
    let tags_label = format!("[{}]", fixture.tags.join(","));

    for plan in plans {
        let (chunked_dom, chunked_tokens) = run_chunked_with_tokens(fixture.input, plan);
        let ctx = InvariantCtx::new(fixture, &full_dom, &chunked_dom, &chunked_tokens);
        for &invariant in fixture.invariants {
            match check_invariant(&ctx, invariant) {
                Ok(()) => {
                    if let Some(reason) = allowed_failure_reason(fixture, invariant) {
                        if strict_xpass {
                            failures.push(format!(
                                "{} {} :: {} :: {} :: XPASS (allowed to fail: {reason})",
                                fixture.name, tags_label, plan, invariant
                            ));
                        } else {
                            eprintln!(
                                "XPASS: {} {} :: {} :: {} :: {reason}",
                                fixture.name, tags_label, plan, invariant
                            );
                        }
                    }
                }
                Err(message) => {
                    if let Some(reason) = allowed_failure_reason(fixture, invariant) {
                        xfails.push(XfailEntry {
                            invariant,
                            message: format!(
                                "XFAIL: {} {} :: {} :: {} :: {message} ({reason})",
                                fixture.name, tags_label, plan, invariant
                            ),
                        });
                    } else {
                        failures.push(format!(
                            "{} {} :: {} :: {} :: {message}",
                            fixture.name, tags_label, plan, invariant
                        ));
                    }
                }
            }
        }
    }

    FixtureRun { failures, xfails }
}

pub(super) fn minimize_plan_for_failure(
    fixture: &GoldenFixture,
    invariant: Invariant,
    full_dom: &Node,
    input: &str,
    plan: &ChunkPlan,
) -> (ChunkPlan, ShrinkStats) {
    shrink_chunk_plan_with_stats(input, plan, |candidate| {
        let (chunked_dom, chunked_tokens) = run_chunked_with_tokens(input, candidate);
        let ctx = InvariantCtx::new(fixture, full_dom, &chunked_dom, &chunked_tokens);
        check_invariant(&ctx, invariant).is_err()
    })
}

pub(super) fn fuzz_seed_count() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_FUZZ_SEEDS")
        && let Ok(parsed) = value.parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    if std::env::var("CI").is_ok() { 50 } else { 200 }
}

pub(super) fn fuzz_seed_base() -> u64 {
    if let Ok(value) = std::env::var("BORROWSER_FUZZ_SEED") {
        if let Ok(parsed) = u64::from_str_radix(value.trim_start_matches("0x"), 16) {
            return parsed;
        }
        if let Ok(parsed) = value.parse::<u64>() {
            return parsed;
        }
    }
    0x6c8e9cf570932bd5
}

pub(super) fn derive_seed(base: u64, name: &str, salt: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in name.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    base ^ hash ^ salt.wrapping_mul(0x9e3779b97f4a7c15)
}

pub(super) fn fuzz_fixture_filter() -> Option<String> {
    std::env::var("BORROWSER_FUZZ_FIXTURE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn fixture_matches_filter(name: &str, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    if !filter.contains('*') {
        return name == filter || name.starts_with(filter);
    }
    let mut remainder = name;
    for part in filter.split('*').filter(|part| !part.is_empty()) {
        if let Some(index) = remainder.find(part) {
            remainder = &remainder[index + part.len()..];
        } else {
            return false;
        }
    }
    true
}

pub(super) fn fuzz_mode() -> FuzzMode {
    let value = std::env::var("BORROWSER_FUZZ_MODE").unwrap_or_else(|_| "mixed".to_string());
    match value.as_str() {
        "sizes" => FuzzMode::Sizes,
        "boundaries" => FuzzMode::Boundaries,
        "semantic" => FuzzMode::Semantic,
        "mixed" => FuzzMode::Mixed,
        _ => panic!("unknown BORROWSER_FUZZ_MODE={value}"),
    }
}

pub(super) fn fuzz_mode_label(mode: FuzzMode) -> &'static str {
    match mode {
        FuzzMode::Sizes => "sizes",
        FuzzMode::Boundaries => "boundaries",
        FuzzMode::Semantic => "semantic",
        FuzzMode::Mixed => "mixed",
    }
}

pub(super) fn repro_command(fixture_name: &str, seed: u64, mode: FuzzMode) -> String {
    format!(
        "repro: BORROWSER_FUZZ_SEED=0x{seed:016x} BORROWSER_FUZZ_FIXTURE={} BORROWSER_FUZZ_SEEDS=1 BORROWSER_FUZZ_MODE={} cargo test -p html golden_corpus_v1_runs_across_random_chunk_plans -- --nocapture",
        fixture_name,
        fuzz_mode_label(mode)
    )
}

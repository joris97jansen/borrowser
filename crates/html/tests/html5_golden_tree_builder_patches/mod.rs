use html::chunker::{ChunkerConfig, build_chunk_plans};
use html::test_harness::shrink_chunk_plan_with_stats;
use html_test_support::diff_lines;
use std::env;

#[path = "../support/html5_fixture_bands.rs"]
mod fixture_bands;

mod assertions;
mod fixtures;
mod formatting;
mod runner;

use assertions::{batch_partition_summary, enforce_expected, filtered_lines_for_diff, lines_match};
use fixture_bands::{H8_FIXTURE_NAMES, H10_FIXTURE_NAMES};
use fixtures::{FixtureStatus, env_u64, fixture_filter, load_fixtures, update_mode};
use runner::{ExecutionMode, PatchRunResult, run_tree_builder_chunked, run_tree_builder_whole};

#[test]
fn html5_golden_tree_builder_patches_whole_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let update = update_mode();
    let mut ran = 0usize;

    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let actual = run_tree_builder_whole(&fixture);
        enforce_expected(&fixture, &actual, ExecutionMode::WholeInput, None, update);
    }

    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tree_builder_patches_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let update = update_mode();
    if update {
        return;
    }
    let mut fuzz_runs = env_u64("BORROWSER_HTML5_PATCH_FUZZ_RUNS", 4) as usize;
    if env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env_u64("BORROWSER_HTML5_PATCH_FUZZ_SEED", 0xC0FFEE);
    let mut ran = 0usize;

    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;

        let whole = run_tree_builder_whole(&fixture);
        if matches!(fixture.expected.status, FixtureStatus::Active)
            && matches!(whole, PatchRunResult::Err(_))
        {
            panic!(
                "fixture '{}' failed in whole-input mode: {:?}",
                fixture.name, whole
            );
        }

        let plans = build_chunk_plans(&fixture.input, fuzz_runs, fuzz_seed, ChunkerConfig::utf8());
        for plan in plans {
            let actual = run_tree_builder_chunked(&fixture, &plan.plan, &plan.label);
            if let (Some(whole_lines), Some(actual_lines)) = (whole.lines(), actual.lines())
                && !lines_match(ExecutionMode::ChunkedInput, actual_lines, whole_lines)
            {
                let (shrunk, stats) =
                    shrink_chunk_plan_with_stats(&fixture.input, &plan.plan, |candidate| {
                        match run_tree_builder_chunked(&fixture, candidate, "shrinking") {
                            PatchRunResult::Ok(lines) => !lines_match(
                                ExecutionMode::ChunkedInput,
                                lines.as_slice(),
                                whole_lines,
                            ),
                            PatchRunResult::Err(_) => true,
                        }
                    });
                let whole_filtered = filtered_lines_for_diff(whole_lines);
                let actual_filtered = filtered_lines_for_diff(actual_lines);
                let diff = diff_lines(&whole_filtered, &actual_filtered);
                let whole_batches = batch_partition_summary(whole_lines);
                let actual_batches = batch_partition_summary(actual_lines);
                panic!(
                    "chunked patch mismatch in fixture '{}'\nplan: {}\nshrunk: {}\nshrink stats: {:?}\nwhole batches: [{}]\nchunked batches: [{}]\n{}",
                    fixture.name, plan.label, shrunk, stats, whole_batches, actual_batches, diff
                );
            }
            enforce_expected(
                &fixture,
                &actual,
                ExecutionMode::ChunkedInput,
                Some(&plan.label),
                update,
            );
        }
    }

    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn h8_patch_fixture_band_is_auto_discovered() {
    let fixtures = load_fixtures();
    for name in H8_FIXTURE_NAMES {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == *name)
            .unwrap_or_else(|| panic!("missing H8 patch fixture '{name}'"));
        assert_eq!(
            fixture.expected.status,
            FixtureStatus::Active,
            "H8 patch fixture '{name}' must participate in the active golden corpus"
        );
    }
}

#[test]
fn h8_patch_fixture_band_runs_in_whole_and_chunked_modes() {
    if update_mode() {
        return;
    }

    let fixtures = load_fixtures();
    for name in H8_FIXTURE_NAMES {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == *name)
            .unwrap_or_else(|| panic!("missing H8 patch fixture '{name}'"));
        let whole = run_tree_builder_whole(fixture);
        enforce_expected(fixture, &whole, ExecutionMode::WholeInput, None, false);

        let plans = build_chunk_plans(&fixture.input, 1, 0xC0FFEE, ChunkerConfig::utf8());
        for plan in plans {
            let actual = run_tree_builder_chunked(fixture, &plan.plan, &plan.label);
            if let (Some(whole_lines), Some(actual_lines)) = (whole.lines(), actual.lines()) {
                assert!(
                    lines_match(ExecutionMode::ChunkedInput, actual_lines, whole_lines),
                    "H8 patch fixture '{}' diverged under chunk plan '{}'",
                    fixture.name,
                    plan.label
                );
            }
            enforce_expected(
                fixture,
                &actual,
                ExecutionMode::ChunkedInput,
                Some(&plan.label),
                false,
            );
        }
    }
}

#[test]
fn h10_patch_fixture_band_is_auto_discovered() {
    let fixtures = load_fixtures();
    for name in H10_FIXTURE_NAMES {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == *name)
            .unwrap_or_else(|| panic!("missing H10 patch fixture '{name}'"));
        assert_eq!(
            fixture.expected.status,
            FixtureStatus::Active,
            "H10 patch fixture '{name}' must participate in the active golden corpus"
        );
    }
}

#[test]
fn h10_patch_fixture_band_runs_in_whole_and_chunked_modes() {
    if update_mode() {
        return;
    }

    let fixtures = load_fixtures();
    for name in H10_FIXTURE_NAMES {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == *name)
            .unwrap_or_else(|| panic!("missing H10 patch fixture '{name}'"));
        let whole = run_tree_builder_whole(fixture);
        enforce_expected(fixture, &whole, ExecutionMode::WholeInput, None, false);

        let plans = build_chunk_plans(&fixture.input, 1, 0xC0FFEE, ChunkerConfig::utf8());
        for plan in plans {
            let actual = run_tree_builder_chunked(fixture, &plan.plan, &plan.label);
            if let (Some(whole_lines), Some(actual_lines)) = (whole.lines(), actual.lines()) {
                assert!(
                    lines_match(ExecutionMode::ChunkedInput, actual_lines, whole_lines),
                    "H10 patch fixture '{}' diverged under chunk plan '{}'",
                    fixture.name,
                    plan.label
                );
            }
            enforce_expected(
                fixture,
                &actual,
                ExecutionMode::ChunkedInput,
                Some(&plan.label),
                false,
            );
        }
    }
}

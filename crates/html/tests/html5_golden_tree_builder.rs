#![cfg(all(feature = "html5", feature = "dom-snapshot"))]
//! Semantic DOM regression harness.
//!
//! Core-v0 patch contract acceptance is covered by
//! `html5_golden_tree_builder_patches.rs`.

#[path = "html5_golden_tree_builder/assertions.rs"]
mod assertions;

#[path = "html5_golden_tree_builder/fixtures.rs"]
mod fixtures;

#[path = "html5_golden_tree_builder/runner.rs"]
mod runner;

use assertions::enforce_expected;
use fixtures::{FixtureStatus, load_fixtures, normalize_fixture_input};
use html::chunker::{ChunkerConfig, build_chunk_plans};
use html::test_harness::shrink_chunk_plan_with_stats;
use html_test_support::diff_lines;
use runner::{RunOutput, run_tree_builder_chunked, run_tree_builder_whole};
use std::env;

#[test]
fn html5_golden_tree_builder_whole_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let actual = run_tree_builder_whole(&fixture);
        enforce_expected(&fixture, &actual, Mode::WholeInput, None);
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tree_builder_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut fuzz_runs = env_u64("BORROWSER_HTML5_DOM_FUZZ_RUNS", 4) as usize;
    if env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env_u64("BORROWSER_HTML5_DOM_FUZZ_SEED", 0xC0FFEE);
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let whole = run_tree_builder_whole(&fixture);
        if matches!(fixture.expected.status, FixtureStatus::Active)
            && matches!(whole, RunOutput::Err(_))
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
                && actual_lines != whole_lines
            {
                let (shrunk, stats) =
                    shrink_chunk_plan_with_stats(&fixture.input, &plan.plan, |candidate| {
                        match run_tree_builder_chunked(&fixture, candidate, "shrinking") {
                            RunOutput::Ok(lines) => lines.as_slice() != whole_lines,
                            RunOutput::Err(_) => true,
                        }
                    });
                panic!(
                    "chunked output mismatch in fixture '{}'\nplan: {}\nshrunk: {}\nshrink stats: {:?}\n{}",
                    fixture.name,
                    plan.label,
                    shrunk,
                    stats,
                    diff_lines(whole_lines, actual_lines)
                );
            }
            enforce_expected(&fixture, &actual, Mode::ChunkedInput, Some(&plan.label));
        }
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    WholeInput,
    ChunkedInput,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::WholeInput => "whole",
            Mode::ChunkedInput => "chunked",
        }
    }
}

struct FixtureFilter {
    raw: Option<String>,
}

impl FixtureFilter {
    fn matches(&self, name: &str) -> bool {
        let Some(filter) = &self.raw else {
            return true;
        };
        name.contains(filter)
    }
}

fn fixture_filter() -> FixtureFilter {
    FixtureFilter {
        raw: env::var("BORROWSER_HTML5_DOM_FIXTURE").ok(),
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

#[test]
fn fixture_input_normalization_strips_single_terminal_lf() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\n".to_string()),
        "<div>ok</div>"
    );
}

#[test]
fn fixture_input_normalization_strips_single_terminal_crlf() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\r\n".to_string()),
        "<div>ok</div>"
    );
}

#[test]
fn fixture_input_normalization_strips_exactly_one_terminal_line_ending() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\n\n".to_string()),
        "<div>ok</div>\n"
    );
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\r\n\r\n".to_string()),
        "<div>ok</div>\r\n"
    );
}

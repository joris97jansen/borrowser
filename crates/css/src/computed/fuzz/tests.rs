use super::{CssValueFuzzConfig, CssValueFuzzTermination, run_seeded_value_fuzz_case};
use crate::syntax::derive_css_fuzz_seed;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn css_value_fuzz_harness_is_reproducible() {
    let bytes = b"12px";
    let config = CssValueFuzzConfig {
        seed: 0x6262,
        max_property_cases: 4,
        ..CssValueFuzzConfig::default()
    };
    let first = run_seeded_value_fuzz_case(bytes, config.clone())
        .expect("first value fuzz run should succeed");
    let second =
        run_seeded_value_fuzz_case(bytes, config).expect("second value fuzz run should succeed");
    assert_eq!(first, second);
    assert_eq!(first.termination, CssValueFuzzTermination::Completed);
    assert!(first.value_cases_observed > 0);
}

#[test]
fn css_value_fuzz_harness_rejects_input_budget() {
    let summary = run_seeded_value_fuzz_case(
        b"0123456789",
        CssValueFuzzConfig {
            max_input_bytes: 4,
            ..CssValueFuzzConfig::default()
        },
    )
    .expect("oversized value input should be rejected");
    assert_eq!(
        summary.termination,
        CssValueFuzzTermination::RejectedMaxInputBytes
    );
}

#[test]
fn css_value_fuzz_harness_rejects_property_budget() {
    let summary = run_seeded_value_fuzz_case(
        b"red",
        CssValueFuzzConfig {
            max_property_cases: 1,
            ..CssValueFuzzConfig::default()
        },
    )
    .expect("bounded property coverage should stay deterministic");
    assert_eq!(summary.termination, CssValueFuzzTermination::Completed);
    assert_eq!(summary.properties_observed, 1);
}

#[test]
fn css_value_fuzz_harness_counts_specified_and_computed_outcomes() {
    let summary = run_seeded_value_fuzz_case(
        b"bogus",
        CssValueFuzzConfig {
            max_property_cases: 3,
            ..CssValueFuzzConfig::default()
        },
    )
    .expect("value fuzz outcome accounting should stay deterministic");
    assert!(summary.value_cases_observed > 0);
    assert!(
        summary.specified_ok_cases
            + summary.specified_error_cases
            + summary.missing_declaration_value_cases
            > 0
    );
}

#[test]
fn replay_committed_css_values_corpus_deterministically() {
    let corpus = committed_entries(values_corpus_dir(), values_regression_dir());
    assert!(
        !corpus.is_empty(),
        "expected committed css values corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = CssValueFuzzConfig {
            seed: derive_css_fuzz_seed(&bytes),
            max_property_cases: 8,
            ..CssValueFuzzConfig::default()
        };
        let first = run_seeded_value_fuzz_case(&bytes, config.clone())
            .unwrap_or_else(|err| panic!("css values corpus replay failed: {err}"));
        let second = run_seeded_value_fuzz_case(&bytes, config)
            .unwrap_or_else(|err| panic!("css values corpus replay failed: {err}"));
        assert_eq!(first, second);
    }
}

fn values_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/css_values")
}

fn values_regression_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/css_values")
}

fn committed_entries(corpus_dir: PathBuf, regression_dir: PathBuf) -> Vec<PathBuf> {
    let mut entries = entries_in_dir(corpus_dir);
    entries.extend(entries_in_dir(regression_dir));
    entries.sort();
    entries
}

fn entries_in_dir(dir: PathBuf) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut entries = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("failed to read input dir {}: {err}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('.') && !name.ends_with(".md"))
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

use super::{
    SelectorMatchingFuzzConfig, SelectorMatchingFuzzTermination, SelectorParserFuzzConfig,
    SelectorParserFuzzTermination, run_seeded_selector_matching_fuzz_case,
    run_seeded_selector_parser_fuzz_case,
};
use crate::syntax::derive_css_fuzz_seed;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn selector_parser_fuzz_harness_is_reproducible() {
    let bytes = b"section > span.label[data-kind=\"promo\"]";
    let config = SelectorParserFuzzConfig {
        seed: 0x1234,
        ..SelectorParserFuzzConfig::default()
    };
    let first = run_seeded_selector_parser_fuzz_case(bytes, config.clone())
        .expect("first selector parser fuzz run should succeed");
    let second = run_seeded_selector_parser_fuzz_case(bytes, config)
        .expect("second selector parser fuzz run should succeed");
    assert_eq!(first, second);
    assert_eq!(first.termination, SelectorParserFuzzTermination::Completed);
}

#[test]
fn selector_matching_fuzz_harness_is_reproducible() {
    let bytes = b"div#hero.alpha";
    let config = SelectorMatchingFuzzConfig {
        seed: 0x4242,
        ..SelectorMatchingFuzzConfig::default()
    };
    let first = run_seeded_selector_matching_fuzz_case(bytes, config.clone())
        .expect("first selector matching fuzz run should succeed");
    let second = run_seeded_selector_matching_fuzz_case(bytes, config)
        .expect("second selector matching fuzz run should succeed");
    assert_eq!(first, second);
    assert_eq!(
        first.termination,
        SelectorMatchingFuzzTermination::Completed
    );
    assert!(first.elements_observed > 0);
}

#[test]
fn selector_parser_fuzz_harness_rejects_input_budget() {
    let summary = run_seeded_selector_parser_fuzz_case(
        b"0123456789",
        SelectorParserFuzzConfig {
            max_input_bytes: 4,
            ..SelectorParserFuzzConfig::default()
        },
    )
    .expect("oversized selector parser input should be rejected");
    assert_eq!(
        summary.termination,
        SelectorParserFuzzTermination::RejectedMaxInputBytes
    );
}

#[test]
fn selector_matching_fuzz_harness_rejects_element_budget() {
    let summary = run_seeded_selector_matching_fuzz_case(
        b"div",
        SelectorMatchingFuzzConfig {
            max_elements_observed: 2,
            ..SelectorMatchingFuzzConfig::default()
        },
    )
    .expect("selector matching element budget rejection should be deterministic");
    assert_eq!(
        summary.termination,
        SelectorMatchingFuzzTermination::RejectedMaxElementsObserved
    );
}

#[test]
fn replay_committed_selector_parser_corpus_deterministically() {
    let corpus = committed_entries(
        selector_parser_corpus_dir(),
        selector_parser_regression_dir(),
    );
    assert!(
        !corpus.is_empty(),
        "expected committed selector parser corpus entries"
    );
    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = SelectorParserFuzzConfig {
            seed: derive_css_fuzz_seed(&bytes),
            ..SelectorParserFuzzConfig::default()
        };
        let first = run_seeded_selector_parser_fuzz_case(&bytes, config.clone())
            .unwrap_or_else(|err| panic!("selector parser corpus replay failed: {err}"));
        let second = run_seeded_selector_parser_fuzz_case(&bytes, config)
            .unwrap_or_else(|err| panic!("selector parser corpus replay failed: {err}"));
        assert_eq!(first, second);
        assert_eq!(first.termination, SelectorParserFuzzTermination::Completed);
    }
}

#[test]
fn replay_committed_selector_matching_corpus_deterministically() {
    let corpus = committed_entries(
        selector_matching_corpus_dir(),
        selector_matching_regression_dir(),
    );
    assert!(
        !corpus.is_empty(),
        "expected committed selector matching corpus entries"
    );
    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = SelectorMatchingFuzzConfig {
            seed: derive_css_fuzz_seed(&bytes),
            ..SelectorMatchingFuzzConfig::default()
        };
        let first = run_seeded_selector_matching_fuzz_case(&bytes, config.clone())
            .unwrap_or_else(|err| panic!("selector matching corpus replay failed: {err}"));
        let second = run_seeded_selector_matching_fuzz_case(&bytes, config)
            .unwrap_or_else(|err| panic!("selector matching corpus replay failed: {err}"));
        assert_eq!(first, second);
        assert_eq!(
            first.termination,
            SelectorMatchingFuzzTermination::Completed
        );
    }
}

#[test]
fn selector_matching_fuzz_harness_reports_limit_exhaustion_as_typed_termination() {
    let summary = run_seeded_selector_matching_fuzz_case(
        b"body div.alpha",
        SelectorMatchingFuzzConfig {
            matching_limits: crate::selectors::SelectorMatchingLimits {
                max_axis_steps_per_match: 0,
            },
            ..SelectorMatchingFuzzConfig::default()
        },
    )
    .expect("selector matching limit exhaustion should stay recoverable");
    assert_eq!(
        summary.termination,
        SelectorMatchingFuzzTermination::SelectorMatchingLimitExceeded
    );
    assert!(summary.limit_errors_observed > 0);
}

fn committed_entries(corpus_dir: PathBuf, regression_dir: PathBuf) -> Vec<PathBuf> {
    let mut entries = entries_in_dir(corpus_dir);
    entries.extend(entries_in_dir(regression_dir));
    entries.sort();
    entries
}

fn selector_parser_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/css_selector_parser")
}

fn selector_parser_regression_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/css_selector_parser")
}

fn selector_matching_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/css_selector_matching")
}

fn selector_matching_regression_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/css_selector_matching")
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

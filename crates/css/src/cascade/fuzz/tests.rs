use super::{CssCascadeFuzzConfig, CssCascadeFuzzTermination, run_seeded_cascade_fuzz_case};
use crate::cascade::StyleResolutionLimits;
use crate::selectors::SelectorMatchingLimits;
use crate::syntax::derive_css_fuzz_seed;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn css_cascade_fuzz_harness_is_reproducible() {
    let bytes = b"div#hero.alpha { width: 12px; display: block; }";
    let config = CssCascadeFuzzConfig {
        seed: 0x5150,
        ..CssCascadeFuzzConfig::default()
    };
    let first = run_seeded_cascade_fuzz_case(bytes, config.clone())
        .expect("first cascade fuzz run should succeed");
    let second = run_seeded_cascade_fuzz_case(bytes, config)
        .expect("second cascade fuzz run should succeed");
    assert_eq!(first, second);
    assert_eq!(first.termination, CssCascadeFuzzTermination::Completed);
    assert!(first.resolved_elements_observed > 0);
}

#[test]
fn css_cascade_fuzz_harness_rejects_input_budget() {
    let summary = run_seeded_cascade_fuzz_case(
        b"0123456789",
        CssCascadeFuzzConfig {
            max_input_bytes: 4,
            ..CssCascadeFuzzConfig::default()
        },
    )
    .expect("oversized cascade input should be rejected");
    assert_eq!(
        summary.termination,
        CssCascadeFuzzTermination::RejectedMaxInputBytes
    );
}

#[test]
fn css_cascade_fuzz_harness_rejects_resolved_element_budget() {
    let summary = run_seeded_cascade_fuzz_case(
        b"body { color: red; }",
        CssCascadeFuzzConfig {
            max_resolved_elements_observed: 2,
            ..CssCascadeFuzzConfig::default()
        },
    )
    .expect("resolved element budget rejection should be deterministic");
    assert_eq!(
        summary.termination,
        CssCascadeFuzzTermination::RejectedMaxResolvedElementsObserved
    );
}

#[test]
fn css_cascade_fuzz_harness_reports_selector_matching_limit_exhaustion() {
    let summary = run_seeded_cascade_fuzz_case(
        b"body div.alpha { color: red; }",
        CssCascadeFuzzConfig {
            style_resolution_limits: StyleResolutionLimits {
                selector_matching: SelectorMatchingLimits {
                    max_axis_steps_per_match: 0,
                },
                ..StyleResolutionLimits::default()
            },
            ..CssCascadeFuzzConfig::default()
        },
    )
    .expect("selector matching limit exhaustion should stay recoverable in cascade fuzzing");
    assert_eq!(
        summary.termination,
        CssCascadeFuzzTermination::SelectorMatchingLimitExceeded
    );
}

#[test]
fn replay_committed_css_cascade_corpus_deterministically() {
    let corpus = committed_entries(cascade_corpus_dir(), cascade_regression_dir());
    assert!(
        !corpus.is_empty(),
        "expected committed css cascade corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = CssCascadeFuzzConfig {
            seed: derive_css_fuzz_seed(&bytes),
            ..CssCascadeFuzzConfig::default()
        };
        let first = run_seeded_cascade_fuzz_case(&bytes, config.clone()).unwrap_or_else(|err| {
            panic!(
                "committed css cascade corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_cascade_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed css cascade corpus entry {} should replay deterministically: {err}",
                entry.display()
            )
        });
        assert_eq!(first, second);
    }
}

fn cascade_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/css_cascade")
}

fn cascade_regression_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/css_cascade")
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

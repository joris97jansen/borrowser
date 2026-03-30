use super::super::config::{TokenizerFuzzConfig, TokenizerFuzzTermination, derive_fuzz_seed};
use super::super::driver::{
    run_seeded_byte_fuzz_case, run_seeded_rawtext_fuzz_case, run_seeded_script_data_fuzz_case,
    run_seeded_textarea_rcdata_fuzz_case, run_seeded_title_rcdata_fuzz_case,
};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn replay_committed_html5_tokenizer_corpus_deterministically() {
    let corpus = committed_input_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed tokenizer fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = TokenizerFuzzConfig {
            seed: derive_fuzz_seed(&bytes),
            ..TokenizerFuzzConfig::default()
        };
        let first = run_seeded_byte_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_byte_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed corpus entry {} should replay without invariant failure on second run: {err}",
                entry.display()
            )
        });

        assert_eq!(
            first,
            second,
            "committed corpus entry {} should replay deterministically",
            entry.display()
        );
        assert_eq!(
            first.termination,
            TokenizerFuzzTermination::Completed,
            "committed corpus entry {} should complete rather than hitting harness limits",
            entry.display()
        );
    }
}

#[test]
fn replay_single_committed_seed_deterministically() {
    let entry = corpus_entry("invalid-utf8-bytes");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = TokenizerFuzzConfig {
        seed: derive_fuzz_seed(&bytes),
        ..TokenizerFuzzConfig::default()
    };
    let first = run_seeded_byte_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed seed {} should replay without invariant failure: {err}",
            entry.display()
        )
    });
    let second = run_seeded_byte_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed seed {} should replay deterministically: {err}",
            entry.display()
        )
    });
    assert_eq!(first, second);
    assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
}

#[test]
fn replay_committed_html5_script_data_corpus_deterministically() {
    let corpus = committed_script_data_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed script-data tokenizer fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = TokenizerFuzzConfig {
            seed: derive_fuzz_seed(&bytes),
            ..TokenizerFuzzConfig::default()
        };
        let first = run_seeded_script_data_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed script-data corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_script_data_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed script-data corpus entry {} should replay without invariant failure on second run: {err}",
                entry.display()
            )
        });

        assert_eq!(
            first,
            second,
            "committed script-data corpus entry {} should replay deterministically",
            entry.display()
        );
        assert_eq!(
            first.termination,
            TokenizerFuzzTermination::Completed,
            "committed script-data corpus entry {} should complete rather than hitting harness limits",
            entry.display()
        );
    }
}

#[test]
fn replay_single_committed_script_data_seed_deterministically() {
    let entry = script_data_corpus_entry("close-tag-near-miss-storm");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = TokenizerFuzzConfig {
        seed: derive_fuzz_seed(&bytes),
        ..TokenizerFuzzConfig::default()
    };
    let first = run_seeded_script_data_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed script-data seed {} should replay without invariant failure: {err}",
            entry.display()
        )
    });
    let second = run_seeded_script_data_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed script-data seed {} should replay deterministically: {err}",
            entry.display()
        )
    });
    assert_eq!(first, second);
    assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
}

#[test]
fn replay_committed_html5_rawtext_corpus_deterministically() {
    let corpus = committed_rawtext_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed rawtext tokenizer fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = TokenizerFuzzConfig {
            seed: derive_fuzz_seed(&bytes),
            ..TokenizerFuzzConfig::default()
        };
        let first = run_seeded_rawtext_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed rawtext corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_rawtext_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed rawtext corpus entry {} should replay without invariant failure on second run: {err}",
                entry.display()
            )
        });

        assert_eq!(
            first,
            second,
            "committed rawtext corpus entry {} should replay deterministically",
            entry.display()
        );
        assert_eq!(
            first.termination,
            TokenizerFuzzTermination::Completed,
            "committed rawtext corpus entry {} should complete rather than hitting harness limits",
            entry.display()
        );
    }
}

#[test]
fn replay_single_committed_rawtext_seed_deterministically() {
    let entry = rawtext_corpus_entry("style-close-tag-near-miss");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = TokenizerFuzzConfig {
        seed: derive_fuzz_seed(&bytes),
        ..TokenizerFuzzConfig::default()
    };
    let first = run_seeded_rawtext_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed rawtext seed {} should replay without invariant failure: {err}",
            entry.display()
        )
    });
    let second = run_seeded_rawtext_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed rawtext seed {} should replay deterministically: {err}",
            entry.display()
        )
    });
    assert_eq!(first, second);
    assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
}

#[test]
fn replay_committed_html5_rcdata_corpus_deterministically() {
    let corpus = committed_rcdata_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed rcdata tokenizer fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = TokenizerFuzzConfig {
            seed: derive_fuzz_seed(&bytes),
            ..TokenizerFuzzConfig::default()
        };

        let title_first = run_seeded_title_rcdata_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed rcdata corpus entry {} should replay as title without invariant failure: {err}",
                entry.display()
            )
        });
        let title_second =
            run_seeded_title_rcdata_fuzz_case(&bytes, config).unwrap_or_else(|err| {
                panic!(
                    "committed rcdata corpus entry {} should replay as title on second run: {err}",
                    entry.display()
                )
            });
        let textarea_first =
            run_seeded_textarea_rcdata_fuzz_case(&bytes, config).unwrap_or_else(|err| {
                panic!(
                    "committed rcdata corpus entry {} should replay as textarea without invariant failure: {err}",
                    entry.display()
                )
            });
        let textarea_second =
            run_seeded_textarea_rcdata_fuzz_case(&bytes, config).unwrap_or_else(|err| {
                panic!(
                    "committed rcdata corpus entry {} should replay as textarea on second run: {err}",
                    entry.display()
                )
            });

        assert_eq!(title_first, title_second);
        assert_eq!(textarea_first, textarea_second);
        assert_eq!(title_first.termination, TokenizerFuzzTermination::Completed);
        assert_eq!(
            textarea_first.termination,
            TokenizerFuzzTermination::Completed
        );
    }
}

#[test]
fn replay_single_committed_rcdata_seed_deterministically() {
    let entry = rcdata_corpus_entry("dual-close-tag-storm");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = TokenizerFuzzConfig {
        seed: derive_fuzz_seed(&bytes),
        ..TokenizerFuzzConfig::default()
    };
    let title = run_seeded_title_rcdata_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed rcdata seed {} should replay as title without invariant failure: {err}",
            entry.display()
        )
    });
    let textarea = run_seeded_textarea_rcdata_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed rcdata seed {} should replay as textarea without invariant failure: {err}",
            entry.display()
        )
    });
    assert_eq!(title.termination, TokenizerFuzzTermination::Completed);
    assert_eq!(textarea.termination, TokenizerFuzzTermination::Completed);
}

fn corpus_entries() -> Vec<PathBuf> {
    entries_in_dir(corpus_dir())
}

fn regression_entries() -> Vec<PathBuf> {
    entries_in_dir(regressions_dir())
}

fn committed_input_entries() -> Vec<PathBuf> {
    let mut entries = corpus_entries();
    entries.extend(regression_entries());
    entries.sort();
    entries
}

fn script_data_corpus_entries() -> Vec<PathBuf> {
    entries_in_dir(script_data_corpus_dir())
}

fn script_data_regression_entries() -> Vec<PathBuf> {
    entries_in_dir(script_data_regressions_dir())
}

fn committed_script_data_entries() -> Vec<PathBuf> {
    let mut entries = script_data_corpus_entries();
    entries.extend(script_data_regression_entries());
    entries.sort();
    entries
}

fn rawtext_corpus_entries() -> Vec<PathBuf> {
    entries_in_dir(rawtext_corpus_dir())
}

fn rawtext_regression_entries() -> Vec<PathBuf> {
    entries_in_dir(rawtext_regressions_dir())
}

fn committed_rawtext_entries() -> Vec<PathBuf> {
    let mut entries = rawtext_corpus_entries();
    entries.extend(rawtext_regression_entries());
    entries.sort();
    entries
}

fn rcdata_corpus_entries() -> Vec<PathBuf> {
    entries_in_dir(rcdata_corpus_dir())
}

fn rcdata_regression_entries() -> Vec<PathBuf> {
    entries_in_dir(rcdata_regressions_dir())
}

fn committed_rcdata_entries() -> Vec<PathBuf> {
    let mut entries = rcdata_corpus_entries();
    entries.extend(rcdata_regression_entries());
    entries.sort();
    entries
}

fn corpus_entry(name: &str) -> PathBuf {
    corpus_dir().join(name)
}

fn script_data_corpus_entry(name: &str) -> PathBuf {
    script_data_corpus_dir().join(name)
}

fn rawtext_corpus_entry(name: &str) -> PathBuf {
    rawtext_corpus_dir().join(name)
}

fn rcdata_corpus_entry(name: &str) -> PathBuf {
    rcdata_corpus_dir().join(name)
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_tokenizer")
}

fn regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/html5_tokenizer")
}

fn script_data_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_tokenizer_script_data")
}

fn script_data_regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/html5_tokenizer_script_data")
}

fn rawtext_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_tokenizer_rawtext")
}

fn rawtext_regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/html5_tokenizer_rawtext")
}

fn rcdata_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_tokenizer_rcdata")
}

fn rcdata_regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/html5_tokenizer_rcdata")
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

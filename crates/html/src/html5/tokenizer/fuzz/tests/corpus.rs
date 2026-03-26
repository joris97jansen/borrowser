use super::super::config::{TokenizerFuzzConfig, TokenizerFuzzTermination, derive_fuzz_seed};
use super::super::driver::run_seeded_byte_fuzz_case;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn replay_committed_html5_tokenizer_corpus_deterministically() {
    let corpus = corpus_entries();
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

fn corpus_entries() -> Vec<PathBuf> {
    let corpus_dir = corpus_dir();
    let mut entries = fs::read_dir(&corpus_dir)
        .unwrap_or_else(|err| panic!("failed to read corpus dir {}: {err}", corpus_dir.display()))
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

fn corpus_entry(name: &str) -> PathBuf {
    corpus_dir().join(name)
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_tokenizer")
}

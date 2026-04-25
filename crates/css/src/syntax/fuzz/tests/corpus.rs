use super::super::{
    CssParserFuzzConfig, CssParserFuzzTermination, CssTokenizerFuzzConfig,
    CssTokenizerFuzzTermination, derive_css_fuzz_seed, run_seeded_parser_fuzz_case,
    run_seeded_tokenizer_fuzz_case,
};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn replay_committed_css_tokenizer_corpus_deterministically() {
    let corpus = committed_tokenizer_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed css tokenizer fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = CssTokenizerFuzzConfig {
            seed: derive_css_fuzz_seed(&bytes),
            ..CssTokenizerFuzzConfig::default()
        };
        let first = run_seeded_tokenizer_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed css tokenizer corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_tokenizer_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed css tokenizer corpus entry {} should replay deterministically: {err}",
                entry.display()
            )
        });

        assert_eq!(first, second);
        assert_eq!(first.termination, CssTokenizerFuzzTermination::Completed);
    }
}

#[test]
fn replay_single_committed_css_tokenizer_seed_deterministically() {
    let entry = tokenizer_corpus_entry("malformed-comment");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = CssTokenizerFuzzConfig {
        seed: derive_css_fuzz_seed(&bytes),
        ..CssTokenizerFuzzConfig::default()
    };
    let first = run_seeded_tokenizer_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single css tokenizer seed {} should replay without invariant failure: {err}",
            entry.display()
        )
    });
    let second = run_seeded_tokenizer_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single css tokenizer seed {} should replay deterministically: {err}",
            entry.display()
        )
    });
    assert_eq!(first, second);
    assert_eq!(first.termination, CssTokenizerFuzzTermination::Completed);
}

#[test]
fn replay_committed_css_parser_corpus_deterministically() {
    let corpus = committed_parser_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed css parser fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = CssParserFuzzConfig {
            seed: derive_css_fuzz_seed(&bytes),
            ..CssParserFuzzConfig::default()
        };
        let first = run_seeded_parser_fuzz_case(&bytes, config.clone()).unwrap_or_else(|err| {
            panic!(
                "committed css parser corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_parser_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed css parser corpus entry {} should replay deterministically: {err}",
                entry.display()
            )
        });

        assert_eq!(first, second);
        assert_eq!(first.termination, CssParserFuzzTermination::Completed);
    }
}

#[test]
fn replay_single_committed_css_parser_seed_deterministically() {
    let entry = parser_corpus_entry("malformed-recovery");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = CssParserFuzzConfig {
        seed: derive_css_fuzz_seed(&bytes),
        ..CssParserFuzzConfig::default()
    };
    let first = run_seeded_parser_fuzz_case(&bytes, config.clone()).unwrap_or_else(|err| {
        panic!(
            "single css parser seed {} should replay without invariant failure: {err}",
            entry.display()
        )
    });
    let second = run_seeded_parser_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single css parser seed {} should replay deterministically: {err}",
            entry.display()
        )
    });
    assert_eq!(first, second);
    assert_eq!(first.termination, CssParserFuzzTermination::Completed);
}

fn committed_tokenizer_entries() -> Vec<PathBuf> {
    let mut entries = entries_in_dir(tokenizer_corpus_dir());
    entries.extend(entries_in_dir(tokenizer_regressions_dir()));
    entries.sort();
    entries
}

fn committed_parser_entries() -> Vec<PathBuf> {
    let mut entries = entries_in_dir(parser_corpus_dir());
    entries.extend(entries_in_dir(parser_regressions_dir()));
    entries.sort();
    entries
}

fn tokenizer_corpus_entry(name: &str) -> PathBuf {
    tokenizer_corpus_dir().join(name)
}

fn parser_corpus_entry(name: &str) -> PathBuf {
    parser_corpus_dir().join(name)
}

fn tokenizer_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/css_tokenizer")
}

fn tokenizer_regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/css_tokenizer")
}

fn parser_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/css_parser")
}

fn parser_regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/css_parser")
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

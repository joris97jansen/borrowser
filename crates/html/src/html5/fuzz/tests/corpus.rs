use super::super::config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzTermination, derive_html5_pipeline_fuzz_seed,
};
use super::super::driver::run_seeded_html5_pipeline_fuzz_case;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn replay_committed_html5_pipeline_corpus_deterministically() {
    let corpus = committed_input_entries();
    assert!(
        !corpus.is_empty(),
        "expected committed html5 pipeline fuzz corpus entries"
    );

    for entry in corpus {
        let bytes = fs::read(&entry)
            .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
        let config = Html5PipelineFuzzConfig {
            seed: derive_html5_pipeline_fuzz_seed(&bytes),
            ..Html5PipelineFuzzConfig::default()
        };
        let first = run_seeded_html5_pipeline_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed corpus entry {} should replay without invariant failure: {err}",
                entry.display()
            )
        });
        let second = run_seeded_html5_pipeline_fuzz_case(&bytes, config).unwrap_or_else(|err| {
            panic!(
                "committed corpus entry {} should replay deterministically: {err}",
                entry.display()
            )
        });

        assert_eq!(first, second);
        assert_eq!(
            first.termination,
            Html5PipelineFuzzTermination::Completed,
            "committed corpus entry {} should complete rather than hit harness limits",
            entry.display()
        );
    }
}

#[test]
fn replay_single_committed_html5_pipeline_seed_deterministically() {
    let entry = corpus_entry("basic-page");
    let bytes = fs::read(&entry)
        .unwrap_or_else(|err| panic!("failed to read corpus entry {}: {err}", entry.display()));
    let config = Html5PipelineFuzzConfig {
        seed: derive_html5_pipeline_fuzz_seed(&bytes),
        ..Html5PipelineFuzzConfig::default()
    };
    let first = run_seeded_html5_pipeline_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed seed {} should replay without invariant failure: {err}",
            entry.display()
        )
    });
    let second = run_seeded_html5_pipeline_fuzz_case(&bytes, config).unwrap_or_else(|err| {
        panic!(
            "single committed seed {} should replay deterministically: {err}",
            entry.display()
        )
    });
    assert_eq!(first, second);
    assert_eq!(first.termination, Html5PipelineFuzzTermination::Completed);
}

fn committed_input_entries() -> Vec<PathBuf> {
    let mut entries = entries_in_dir(corpus_dir());
    entries.extend(entries_in_dir(regressions_dir()));
    entries.sort();
    entries
}

fn corpus_entry(name: &str) -> PathBuf {
    corpus_dir().join(name)
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_pipeline")
}

fn regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/html5_pipeline")
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

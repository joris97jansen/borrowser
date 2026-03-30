use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn html5_pipeline_regression_snapshots_match_rendered_output() {
    let snapshots = regression_snapshot_entries();
    assert!(
        !snapshots.is_empty(),
        "expected at least one html5 pipeline regression snapshot"
    );

    for snapshot in snapshots {
        let label = snapshot
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_else(|| panic!("snapshot file {} has no UTF-8 stem", snapshot.display()));
        let input = regression_input_entry(label);
        let bytes = fs::read(&input).unwrap_or_else(|err| {
            panic!("failed to read regression input {}: {err}", input.display())
        });
        let rendered = html::html5::render_html5_pipeline_regression_snapshot(&bytes, label)
            .unwrap_or_else(|err| {
                panic!(
                    "failed to render regression snapshot for {} from {}: {err}",
                    label,
                    input.display()
                )
            });
        let expected = fs::read_to_string(&snapshot).unwrap_or_else(|err| {
            panic!(
                "failed to read regression snapshot {}: {err}",
                snapshot.display()
            )
        });

        assert_eq!(
            rendered.trim_end(),
            expected.trim_end(),
            "html5 pipeline regression snapshot {} drifted; regenerate it with the snapshot helper against {}",
            snapshot.display(),
            input.display()
        );
    }
}

fn regression_snapshot_entries() -> Vec<PathBuf> {
    let mut entries = fs::read_dir(regressions_snapshot_dir())
        .unwrap_or_else(|err| {
            panic!(
                "failed to read regression snapshot dir {}: {err}",
                regressions_snapshot_dir().display()
            )
        })
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "snap"))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn regression_input_entry(label: &str) -> PathBuf {
    let regression = fuzz_regressions_dir().join(label);
    if regression.is_file() {
        return regression;
    }
    let corpus = fuzz_corpus_dir().join(label);
    if corpus.is_file() {
        return corpus;
    }
    panic!(
        "no matching pipeline regression input found for snapshot {} in {} or {}",
        label,
        fuzz_regressions_dir().display(),
        fuzz_corpus_dir().display()
    );
}

fn regressions_snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressions/html5_pipeline")
}

fn fuzz_regressions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions/html5_pipeline")
}

fn fuzz_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/html5_pipeline")
}

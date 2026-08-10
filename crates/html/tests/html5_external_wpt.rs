#![cfg(feature = "parser-conformance")]

use html_test_support::external_wpt::{
    ExternalCapability, ExternalCaseClassification, adapt_allowlisted_subset,
};
use html_test_support::parser_fixture::{
    FixtureRepository, FixtureRepositoryPolicy, discover_and_load, run_fixture_corpus,
};
use std::fs;
use std::path::Path;

#[test]
fn pinned_external_records_are_adapter_source_of_truth_and_run_canonically() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("html crate must live under <repository>/crates/html");
    let adapted = adapt_allowlisted_subset(
        &repository_root.join("tests/wpt/external/raw"),
        &repository_root.join("tests/wpt/external/allowlist.toml"),
    )
    .expect("pinned WPT records must adapt deterministically");
    let unsupported = adapted
        .artifacts()
        .iter()
        .filter(|artifact| {
            artifact.classification()
                == &ExternalCaseClassification::Unsupported(ExternalCapability::Scripting)
        })
        .count();
    assert_eq!(unsupported, 2, "default WPT scripting must be explicit");

    let eligible = adapted
        .artifacts()
        .iter()
        .filter(|artifact| artifact.classification() == &ExternalCaseClassification::Eligible)
        .collect::<Vec<_>>();
    assert_eq!(eligible.len(), 1);
    for artifact in &eligible {
        let bundle_root = crate_root
            .join("tests/fixtures/html5/external-wpt")
            .join(artifact.bundle_name());
        for (relative, expected) in artifact.files() {
            let actual = fs::read(bundle_root.join(relative))
                .unwrap_or_else(|error| panic!("generated artifact {relative}: {error}"));
            assert_eq!(&actual, expected, "generated artifact drift: {relative}");
        }
    }

    let fixture_root = crate_root.join("tests/fixtures/html5/external-wpt");
    let repository = FixtureRepository {
        repository_root: repository_root.to_path_buf(),
        fixture_root,
        policy: FixtureRepositoryPolicy::AdaptedOrQuarantine,
    };
    let fixtures = discover_and_load(&repository).expect("generated external fixture validates");
    let reports = run_fixture_corpus(&fixtures)
        .unwrap_or_else(|error| panic!("external canonical corpus failed:\n{error}"));
    assert_eq!(reports.len(), eligible.len());
}

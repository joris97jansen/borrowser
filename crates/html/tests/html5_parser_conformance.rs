#![cfg(feature = "parser-conformance")]

use html::conformance::{ObservationState, ObservedToken};
use html_test_support::parser_fixture::{FixtureRepository, discover_and_load, run_fixture_corpus};
use std::path::Path;

#[test]
fn canonical_parser_conformance_corpus_executes_every_discovered_fixture() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("html crate must live under <repository>/crates/html");
    let fixture_root = crate_root.join("tests/fixtures/html5/conformance");
    let repository = FixtureRepository::native(repository_root, fixture_root);
    let fixtures = discover_and_load(&repository).expect("canonical fixtures must load");

    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.id().as_str() == "tokenizer-character-data"),
        "the AE13a seed fixture must remain discoverable by ID"
    );
    let reports = run_fixture_corpus(&fixtures)
        .unwrap_or_else(|error| panic!("canonical parser fixture corpus failed:\n{error}"));
    assert_eq!(reports.len(), fixtures.len());
    let report = reports
        .iter()
        .find(|report| report.fixture_id().as_str() == "tokenizer-character-data")
        .expect("seed fixture report must exist");
    let result = report
        .result()
        .expect("active fixture has canonical result");
    assert_eq!(
        result.tokens,
        ObservationState::Captured(vec![
            ObservedToken::Character {
                data: "Hello, parser fixtures!\n".to_string(),
            },
            ObservedToken::Eof,
        ])
    );
}

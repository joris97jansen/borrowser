#[cfg(feature = "dom-snapshot")]
use super::fixtures::Fixture;
#[cfg(feature = "dom-snapshot")]
use html::dom_snapshot::DomSnapshotOptions;
#[cfg(feature = "dom-snapshot")]
use html::test_harness::ChunkPlan;
#[cfg(feature = "dom-snapshot")]
use html_test_support::wpt_tree_builder::{run_tree_builder_chunked, run_tree_builder_whole};

/// Tree-builder regression harness for DOM snapshot verification under the
/// rawtext/script regression corpus.

#[cfg(feature = "dom-snapshot")]
pub(crate) fn run_dom_fixture_whole(fixture: &Fixture) -> Vec<String> {
    let options = dom_options(fixture);
    run_tree_builder_whole(&fixture.input, &fixture.name, options).unwrap_or_else(|err| {
        panic!(
            "tree-builder whole-input run failed for rawtext/script regression '{}' [whole]\npath: {}\nguard: {}\n{err}",
            fixture.name,
            fixture.dir.display(),
            fixture.meta.guard
        )
    })
}

#[cfg(feature = "dom-snapshot")]
pub(crate) fn run_dom_fixture_every_boundary(fixture: &Fixture) -> Vec<String> {
    let options = dom_options(fixture);
    let plan = ChunkPlan::boundaries(
        fixture
            .input
            .char_indices()
            .skip(1)
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>(),
    );
    run_tree_builder_chunked(&fixture.input, &fixture.name, &plan, "every-boundary", options)
        .unwrap_or_else(|err| {
            panic!(
                "tree-builder every-boundary run failed for rawtext/script regression '{}'\npath: {}\nguard: {}\n{err}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            )
        })
}

#[cfg(feature = "dom-snapshot")]
fn dom_options(fixture: &Fixture) -> DomSnapshotOptions {
    let expected = fixture
        .expected_dom
        .as_ref()
        .expect("dom_options requires dom expectation");
    DomSnapshotOptions {
        ignore_ids: expected.ignore_ids,
        ignore_empty_style: expected.ignore_empty_style,
    }
}

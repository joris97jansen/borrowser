#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

#[path = "wpt_manifest.rs"]
mod wpt_manifest;

#[path = "support/wpt_tree_builder_suite.rs"]
mod wpt_tree_builder_suite;

#[test]
fn pinned_wpt_template_tree_builder_subset() {
    wpt_tree_builder_suite::run(wpt_tree_builder_suite::TreeBuilderSuiteSpec::templates());
}

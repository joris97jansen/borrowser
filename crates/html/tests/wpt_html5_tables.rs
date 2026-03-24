#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

mod wpt_manifest;
#[path = "support/wpt_tree_builder_suite.rs"]
mod wpt_tree_builder_suite;

#[test]
fn wpt_html5_tables_slice() {
    wpt_tree_builder_suite::run(wpt_tree_builder_suite::TreeBuilderSuiteSpec::tables());
}

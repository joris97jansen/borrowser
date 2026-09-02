use std::path::Path;

#[test]
fn repository_derived_fixture_has_truthful_wpt_lineage_and_ag7_semantics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    rendering_test_support::validate_ag8_rendering_adaptation(root).unwrap();
}

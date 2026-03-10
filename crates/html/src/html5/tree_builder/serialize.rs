#[cfg(feature = "dom-snapshot")]
pub fn serialize_dom_for_test(root: &crate::Node) -> Vec<String> {
    serialize_dom_for_test_with_options(root, crate::dom_snapshot::DomSnapshotOptions::default())
}

#[cfg(feature = "dom-snapshot")]
pub fn serialize_dom_for_test_with_options(
    root: &crate::Node,
    options: crate::dom_snapshot::DomSnapshotOptions,
) -> Vec<String> {
    crate::dom_snapshot::DomSnapshot::new(root, options)
        .as_lines()
        .to_vec()
}

use std::fs;

use conformance_test_support::{
    InventoryDiagnosticKind, InventoryRepository, discover_inventory,
    load_external_lineage_registry, reconcile_external_fixture_lineages,
};

fn build_repository(root: &std::path::Path) {
    let fixture = root.join("tests/conformance/fixtures/derived");
    fs::create_dir_all(fixture.join("rendering")).unwrap();
    fs::write(
        fixture.join("fixture.toml"),
        r#"format = "borrowser-conformance-fixture-v4"
id = "derived-proof"
scope = "static-html-css-no-js"
observation = "paint-operations"
test_path = "rendering/test.html"
[source]
kind = "external-derived"
lineage_id = "lineage-proof-v1"
adapter = "rendering-paired-semantic"
adapter_version = "1"
[reference]
kind = "semantic"
relation = "match"
path = "rendering/reference.html"
[execution_package]
entry_path = "rendering/fixture.toml"
support_paths = []
[metadata]
description = "A synthetic external lineage reconciliation proof."
"#,
    )
    .unwrap();
    fs::write(
        fixture.join("rendering/fixture.toml"),
        b"format = \"synthetic\"\n",
    )
    .unwrap();
    fs::write(fixture.join("rendering/test.html"), b"test\n").unwrap();
    fs::write(fixture.join("rendering/reference.html"), b"reference\n").unwrap();
    let external = root.join("tests/conformance/external");
    fs::create_dir_all(external.join("synthetic")).unwrap();
    fs::write(
        external.join("registries.toml"),
        r#"format = "borrowser-external-registry-index-v1"
registries = ["tests/conformance/external/synthetic/sources.toml"]
"#,
    )
    .unwrap();
    fs::write(
        external.join("synthetic/sources.toml"),
        r#"lineage_registry_format = "borrowser-external-lineage-registry-v1"
[[records]]
id = "source-record"
[[lineages]]
id = "lineage-proof-v1"
source_record = "source-record"
adapter = "rendering-paired-semantic"
adapter_version = "1"
derived_test_id = "derived-proof"
description = "Synthetic lineage."
transformation = "exact-copy-v1"
reference_file = "reference"
test_artifact_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
reference_artifact_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#,
    )
    .unwrap();
}

#[test]
fn every_v4_external_lineage_reconciles_test_and_adapter_exactly() {
    let temp = tempfile::tempdir().unwrap();
    build_repository(temp.path());
    let repository =
        InventoryRepository::new(temp.path(), temp.path().join("tests/conformance/fixtures"));
    let inventory = discover_inventory(&repository).unwrap();
    let registry = load_external_lineage_registry(temp.path()).unwrap();
    reconcile_external_fixture_lineages(&inventory, &registry).unwrap();
}

#[test]
fn well_formed_but_dangling_v4_lineage_fails_repository_discovery() {
    let temp = tempfile::tempdir().unwrap();
    build_repository(temp.path());
    let descriptor = temp
        .path()
        .join("tests/conformance/fixtures/derived/fixture.toml");
    let text = fs::read_to_string(&descriptor)
        .unwrap()
        .replace("lineage-proof-v1", "dangling-lineage-v1");
    fs::write(descriptor, text).unwrap();
    let repository =
        InventoryRepository::new(temp.path(), temp.path().join("tests/conformance/fixtures"));
    let errors = discover_inventory(&repository).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::ExternalLineageReconciliation { .. }
    )));
    let registry = load_external_lineage_registry(temp.path()).unwrap();
    assert!(
        registry
            .get(
                &conformance_test_support::ExternalLineageId::parse("dangling-lineage-v1").unwrap()
            )
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn lineage_index_and_referenced_registry_reject_symlinked_parents() {
    use std::os::unix::fs::symlink;
    for target in ["index", "referenced-registry"] {
        let temp = tempfile::tempdir().unwrap();
        build_repository(temp.path());
        if target == "index" {
            let parent = temp.path().join("tests/conformance/external");
            let real = temp.path().join("tests/conformance/real-external");
            fs::rename(&parent, &real).unwrap();
            symlink(&real, &parent).unwrap();
        } else {
            let parent = temp.path().join("tests/conformance/external/synthetic");
            let real = temp
                .path()
                .join("tests/conformance/external/real-synthetic");
            fs::rename(&parent, &real).unwrap();
            symlink(&real, &parent).unwrap();
        }
        assert!(load_external_lineage_registry(temp.path()).is_err());
    }
}

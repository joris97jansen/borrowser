mod support;

use std::fs;

use conformance_test_support::{
    InventoryRepository, ManifestCheck, ReferenceKind, ReferenceRelation, build_manifest,
    check_manifest, discover_inventory, generate_manifest_bytes, serialize_manifest,
    update_manifest,
};
use support::{TestRepository, descriptor, descriptor_with_reference};

#[test]
fn manifest_generation_is_byte_identical_and_sorted_by_id() {
    let repository = TestRepository::new();
    repository.bundle(
        "first-path",
        &descriptor("zeta-id", "dom-tree", "test.html"),
        &[("test.html", b"zeta")],
    );
    repository.bundle(
        "second-path",
        &descriptor("alpha-id", "html-tokenizer", "test.html"),
        &[("test.html", b"alpha")],
    );
    let first = generate_manifest_bytes(&repository.repository()).expect("first generation");
    let second = generate_manifest_bytes(&repository.repository()).expect("second generation");
    assert_eq!(first, second);
    let text = String::from_utf8(first).expect("UTF-8 manifest");
    assert!(text.find("id = \"alpha-id\"").unwrap() < text.find("id = \"zeta-id\"").unwrap());
    assert!(text.ends_with('\n'));
    assert!(!text.contains('\\'));
    assert!(!text.contains(repository.root().to_string_lossy().as_ref()));
}

#[test]
fn exact_manifest_contract_has_fixed_fields_whitespace_and_reference_metadata() {
    let repository = TestRepository::new();
    repository.bundle(
        "reference",
        &descriptor_with_reference(
            "semantic-reference",
            "paint-operations",
            "test.html",
            "semantic",
            "reference.html",
        ),
        &[("test.html", b"test"), ("reference.html", b"reference")],
    );
    let actual = String::from_utf8(
        generate_manifest_bytes(&repository.repository()).expect("manifest generation"),
    )
    .expect("UTF-8 manifest");
    assert_eq!(
        actual,
        r#"format = "borrowser-conformance-manifest-v3"

[[tests]]
id = "semantic-reference"
fixture_format = "borrowser-conformance-fixture-v1"
fixture_path = "tests/conformance/fixtures/reference"
test_path = "tests/conformance/fixtures/reference/test.html"
metadata_path = "tests/conformance/fixtures/reference/fixture.toml"
scope = "static-html-css-no-js"
observation = "paint-operations"
source_kind = "native"
reference_kind = "semantic"
reference_relation = "match"
reference_path = "tests/conformance/fixtures/reference/reference.html"
"#
    );
}

#[test]
fn structural_reference_is_validated_and_serialized_as_a_declaration() {
    let repository = TestRepository::new();
    repository.bundle(
        "structural-reference",
        &descriptor_with_reference(
            "structural-reference",
            "dom-tree",
            "test.html",
            "structural",
            "reference.html",
        ),
        &[("test.html", b"test"), ("reference.html", b"reference")],
    );
    let inventory = discover_inventory(&repository.repository()).expect("structural inventory");
    assert_eq!(
        inventory.fixtures()[0].reference().unwrap().kind(),
        ReferenceKind::Structural
    );
    assert_eq!(
        inventory.fixtures()[0].reference().unwrap().relation(),
        ReferenceRelation::Match
    );
    let manifest = String::from_utf8(generate_manifest_bytes(&repository.repository()).unwrap())
        .expect("UTF-8 manifest");
    assert!(manifest.contains("reference_kind = \"structural\"\n"));
    assert!(manifest.contains("reference_relation = \"match\"\n"));
    assert!(manifest.contains(
        "reference_path = \"tests/conformance/fixtures/structural-reference/reference.html\"\n"
    ));
}

#[test]
fn checked_in_repository_manifest_is_exactly_current() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root.parent().unwrap().parent().unwrap();
    let repository = InventoryRepository::new(
        repository_root,
        repository_root.join("tests/conformance/fixtures"),
    );
    let actual = generate_manifest_bytes(&repository).expect("repository manifest generation");
    let checked_in = fs::read(repository_root.join("tests/conformance/manifest.toml"))
        .expect("checked-in manifest");
    assert_eq!(actual, checked_in);
}

#[test]
fn check_is_read_only_and_update_replaces_only_after_complete_generation() {
    let repository = TestRepository::new();
    repository.bundle(
        "case",
        &descriptor("update-case", "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    let inventory = discover_inventory(&repository.repository()).expect("valid inventory");
    let manifest = build_manifest(&inventory);
    let output_dir = repository.root().join("tests/conformance");
    let output = output_dir.join("manifest.toml");

    assert_eq!(
        check_manifest(repository.root(), &output, &manifest).expect("check missing manifest"),
        ManifestCheck::Missing
    );
    update_manifest(repository.root(), &output, &manifest).expect("create missing manifest");
    assert_eq!(
        fs::read(&output).expect("created manifest"),
        serialize_manifest(&manifest)
    );

    fs::write(&output, b"sentinel\n").expect("stale manifest");
    assert_eq!(
        check_manifest(repository.root(), &output, &manifest).expect("check stale manifest"),
        ManifestCheck::Stale
    );
    assert_eq!(
        fs::read(&output).expect("unchanged stale manifest"),
        b"sentinel\n"
    );

    update_manifest(repository.root(), &output, &manifest).expect("atomic update");
    assert_eq!(
        fs::read(&output).expect("updated manifest"),
        serialize_manifest(&manifest)
    );
    assert_eq!(
        check_manifest(repository.root(), &output, &manifest).expect("check current manifest"),
        ManifestCheck::Current
    );
}

#[test]
fn failed_update_preserves_existing_manifest_exactly() {
    let repository = TestRepository::new();
    repository.bundle(
        "case",
        &descriptor("preserved-manifest", "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    let inventory = discover_inventory(&repository.repository()).expect("valid inventory");
    let manifest = build_manifest(&inventory);
    let output = repository.root().join("tests/conformance/manifest.toml");
    let previous = b"previous-valid-manifest\r\nexact-bytes\n";
    fs::write(&output, previous).expect("existing manifest");

    let narrower_root = repository.fixture_root();
    assert!(update_manifest(&narrower_root, &output, &manifest).is_err());
    assert_eq!(fs::read(&output).expect("preserved manifest"), previous);
}

#[test]
fn invalid_inventory_cannot_replace_existing_manifest() {
    let repository = TestRepository::new();
    repository.bundle("bad", "malformed = [", &[("test.html", b"input")]);
    let output = repository.root().join("tests/conformance/manifest.toml");
    fs::write(&output, b"previous-valid-manifest\n").expect("existing manifest");
    assert!(discover_inventory(&repository.repository()).is_err());
    assert_eq!(
        fs::read(output).expect("previous manifest remains"),
        b"previous-valid-manifest\n"
    );
}

#[cfg(unix)]
#[test]
fn manifest_output_rejects_symlink_targets_and_parents() {
    use std::os::unix::fs::symlink;

    let repository = TestRepository::new();
    repository.bundle(
        "case",
        &descriptor("symlink-output", "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    let inventory = discover_inventory(&repository.repository()).expect("valid inventory");
    let manifest = build_manifest(&inventory);
    let output = repository.root().join("tests/conformance/manifest.toml");
    let outside = repository.root().join("outside.toml");
    fs::write(&outside, b"outside").expect("outside target");
    symlink(&outside, &output).expect("manifest symlink");
    assert!(update_manifest(repository.root(), &output, &manifest).is_err());
    assert_eq!(fs::read(outside).expect("outside remains"), b"outside");

    fs::remove_file(&output).expect("remove target symlink");
    let linked_parent = repository.root().join("linked-parent");
    symlink(repository.root().join("tests/conformance"), &linked_parent).expect("parent symlink");
    assert!(
        update_manifest(
            repository.root(),
            &linked_parent.join("manifest.toml"),
            &manifest
        )
        .is_err()
    );
}

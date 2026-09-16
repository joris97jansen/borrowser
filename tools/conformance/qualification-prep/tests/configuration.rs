use qualification_prep::{
    canonical,
    configuration::{self, HostRecord, PinRecord},
    distribution::{CandidateDistributionManifest, EntryRecord, FORMAT},
    identity::{AUTHORITY, BrowserIdentity},
};
fn specimens() -> (
    PinRecord,
    HostRecord,
    BrowserIdentity,
    CandidateDistributionManifest,
) {
    // Synthetic unit-test inputs only; never a browser pin or real configuration.
    let hash = "a".repeat(64);
    let m = CandidateDistributionManifest {
        format: FORMAT.into(),
        root_mode: 493,
        directories: vec![],
        entries: vec![EntryRecord::Regular {
            path: "chrome".into(),
            mode: 493,
            executable: true,
            byte_length: 1,
            sha256: canonical::hash(b"x"),
            file_capabilities: "absent".into(),
        }],
    };
    let mh = canonical::hash(&m.canonical_bytes().unwrap());
    let b = BrowserIdentity {
        format: "borrowser-preparation-browser-identity-v1".into(),
        authority: AUTHORITY.into(),
        product_raw: "TestBrowser/1.2".into(),
        browser_product: "TestBrowser".into(),
        browser_version: "1.2".into(),
        revision: Some("@test".into()),
        protocol_version: "test-protocol".into(),
        executable_sha256: canonical::hash(b"x"),
        distribution_manifest_sha256: mh.clone(),
        unshare_sha256: hash.clone(),
        unshare_version: "test-unshare".into(),
    };
    let h = HostRecord {
        format: "borrowser-preparation-host-v1".into(),
        authority: AUTHORITY.into(),
        image_identity: "test-image".into(),
        image_sha256: hash.clone(),
        snapshot_identity: "test-snapshot".into(),
        pretty_name: "Test Linux".into(),
        architecture: "x86_64".into(),
        kernel_release: "test-kernel".into(),
        uid: 1000,
        gid: 1000,
        security_policy_record_sha256: hash.clone(),
        libraries_record_sha256: hash.clone(),
        resources_record_sha256: hash.clone(),
    };
    let p = PinRecord {
        format: "borrowser-preparation-pin-v1".into(),
        authority: AUTHORITY.into(),
        publisher: "test-publisher".into(),
        release: "test-release".into(),
        architecture: "x86_64".into(),
        upstream_artifact: "test-artifact".into(),
        upstream_sha256: hash.clone(),
        provenance_record_sha256: hash.clone(),
        source_build_provenance: "test-source".into(),
        vendor_patches: "test-patches".into(),
        extraction_record_sha256: hash.clone(),
        frozen_tree_identity: "test-tree".into(),
        executable_path: "chrome".into(),
        browser_identity_sha256: canonical::hash(&canonical::json(&b).unwrap()),
        host_record_sha256: canonical::hash(&canonical::json(&h).unwrap()),
        distribution_manifest_sha256: mh,
        review_record_sha256: hash,
    };
    (p, h, b, m)
}
#[test]
fn exact_golden_all_constants_order_and_minimal_argv() {
    let (p, h, b, m) = specimens();
    let actual = configuration::configuration_bytes(&p, &h, &b, &m).unwrap();
    assert_eq!(actual, include_bytes!("configuration.golden.toml"));
    let parsed: toml::Value = toml::from_str(std::str::from_utf8(&actual).unwrap()).unwrap();
    assert_eq!(parsed.as_table().unwrap().len(), 49);
    assert_eq!(parsed["invocation_arguments"].as_array().unwrap().len(), 9);
}
#[test]
fn binding_and_revision_omission() {
    let (mut p, h, mut b, m) = specimens();
    b.revision = None;
    p.browser_identity_sha256 = canonical::hash(&canonical::json(&b).unwrap());
    let bytes = configuration::configuration_bytes(&p, &h, &b, &m).unwrap();
    assert!(
        !String::from_utf8(bytes)
            .unwrap()
            .contains("browser_build_revision")
    );
    b.revision = Some(String::new());
    p.browser_identity_sha256 = canonical::hash(&canonical::json(&b).unwrap());
    assert!(
        !String::from_utf8(configuration::configuration_bytes(&p, &h, &b, &m).unwrap())
            .unwrap()
            .contains("browser_build_revision")
    );
    b.revision = Some(" bad ".into());
    assert!(configuration::configuration_bytes(&p, &h, &b, &m).is_err());
    let (p, h, b, m) = specimens();
    let mut bad = b.clone();
    bad.browser_version = "wrong".into();
    assert!(configuration::configuration_bytes(&p, &h, &bad, &m).is_err());
    let mut bad = b.clone();
    bad.executable_sha256 = "b".repeat(64);
    assert!(configuration::configuration_bytes(&p, &h, &bad, &m).is_err());
    let mut bad = h.clone();
    bad.kernel_release = "changed".into();
    assert!(configuration::configuration_bytes(&p, &bad, &b, &m).is_err());
    let mut bad = p.clone();
    bad.distribution_manifest_sha256 = "b".repeat(64);
    assert!(configuration::configuration_bytes(&bad, &h, &b, &m).is_err());
    let mut bad = m.clone();
    bad.root_mode = 448;
    assert!(configuration::configuration_bytes(&p, &h, &b, &bad).is_err());
}
#[test]
fn records_are_bounded_typed_and_canonical_and_publication_is_exclusive() {
    let (p, _, b, _) = specimens();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("pin");
    canonical::publish_new(&path, &canonical::json(&p).unwrap()).unwrap();
    let _: PinRecord = canonical::record(&path).unwrap();
    assert!(canonical::publish_new(&path, b"bad").is_err());
    let mut v = serde_json::to_value(&p).unwrap();
    v["unknown"] = true.into();
    std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(canonical::record::<PinRecord>(&path).is_err());
    std::fs::write(&path, vec![b' '; 65537]).unwrap();
    assert!(canonical::record::<PinRecord>(&path).is_err());
    let mut v = serde_json::to_value(&b).unwrap();
    v["revision"] = 42.into();
    assert!(serde_json::from_value::<BrowserIdentity>(v).is_err());
    let root = tempfile::tempdir().unwrap();
    let (p, h, b, m) = specimens();
    assert!(configuration::produce(root.path(), &p, &h, &b, &m).is_err());
}
#[test]
fn string_escaping_remains_exact_toml_and_records_have_no_unknown_fields() {
    let (mut p, mut h, b, m) = specimens();
    h.pretty_name = "Test \"Edition\" \\ Linux".into();
    p.host_record_sha256 = canonical::hash(&canonical::json(&h).unwrap());
    let bytes = configuration::configuration_bytes(&p, &h, &b, &m).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("platform_os_version = \"Test \\\"Edition\\\" \\\\ Linux\"\n"));
    let parsed: toml::Value = toml::from_str(text).unwrap();
    assert_eq!(
        parsed["platform_os_version"].as_str().unwrap(),
        h.pretty_name
    );
}

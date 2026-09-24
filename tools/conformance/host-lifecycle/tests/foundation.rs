use borrowser_host_lifecycle::{canonical, deployment::*, model::*};
#[test]
fn exact_root_marker_and_deployment_vectors() {
    let bytes = include_bytes!("fixtures/authority-v2.json");
    let marker: AuthorityRootV2 = canonical::decode(bytes).unwrap();
    marker.validate().unwrap();
    assert_eq!(canonical::encode(&marker).unwrap(), bytes);
    assert_eq!(
        canonical::sha256(bytes),
        include_str!("fixtures/authority-v2.sha256").trim()
    );
    let bytes = include_bytes!("fixtures/deployment-v2.json");
    let deployment: DeploymentV2 = canonical::decode(bytes).unwrap();
    assert_eq!(deployment.marker().unwrap(), marker);
    assert_eq!(
        canonical::sha256(bytes),
        include_str!("fixtures/deployment-v2.sha256").trim()
    );
}
#[test]
fn deployment_rejects_unsupported_generation_and_identity() {
    let bytes = include_bytes!("fixtures/deployment-v2.json");
    for (from, to) in [
        ("schema_version\":2", "schema_version\":1"),
        ("aws-ec2-deployment", "unknown-deployment"),
        ("111111111111", "account"),
        ("eu-central-1", ""),
        ("11111111111111111111111111111111", "machine"),
        ("00000000-0000-0000-0000-000000000001", "filesystem"),
    ] {
        let text = String::from_utf8(bytes.to_vec()).unwrap().replace(from, to);
        assert!(
            canonical::decode::<DeploymentV2>(text.as_bytes())
                .and_then(|d| d.validate())
                .is_err(),
            "{from}"
        );
    }
    for text in [
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .replace("{", "{\"unknown\":0,"),
        String::from_utf8(bytes.to_vec()).unwrap().replace(
            "\"schema_version\":2",
            "\"schema_version\":2,\"schema_version\":2",
        ),
        format!("{} ", std::str::from_utf8(bytes).unwrap()),
    ] {
        assert!(canonical::decode::<DeploymentV2>(text.as_bytes()).is_err());
    }
}
#[test]
fn production_event_schema_has_no_storage_test_or_provider_commands() {
    for kind in [
        "storage-checkpoint",
        "storage-recovery",
        "storage-evidence",
        "allocation-authorized",
        "termination-authorized",
    ] {
        let bytes = format!("{{\"kind\":\"{kind}\"}}\n");
        assert!(canonical::decode::<EventV2>(bytes.as_bytes()).is_err());
    }
    assert!(canonical::decode::<EventV2>(b"{\"kind\":\"authority-initialized\"}\n").is_ok());
}

#[test]
fn canonical_encoding_preserves_control_unicode_and_integer_contract() {
    let input = serde_json::json!({"z": null, "a": "é\n\t\u{0000}\\\"", "n": u64::MAX});
    let expected = concat!(
        r#"{"a":"é\u000a\u0009\u0000\\\"","n":18446744073709551615,"z":null}"#,
        "\n"
    )
    .as_bytes();
    assert_eq!(canonical::encode(&input).unwrap(), expected);
    assert!(canonical::encode(&serde_json::json!(-1)).is_err());
    assert!(canonical::encode(&serde_json::json!(1.5)).is_err());
}
#[test]
fn cli_rejects_removed_commands_before_deployment_or_network() {
    for command in [
        "allocate",
        "cancel",
        "terminate",
        "reconcile",
        "watch",
        "resolve",
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_borrowser-host-lifecycle"))
            .arg(command)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("provider operations unavailable")
        );
    }
}

#[test]
fn region_is_bounded_lexical_identity_not_a_catalogue() {
    use borrowser_host_lifecycle::identity::Region;
    for valid in [
        "eu-central-1",
        "us-gov-west-1",
        "cn-north-1",
        "eusc-de-east-1",
        "a",
        "7",
        "future1-zone2",
        "not-a-real-region",
    ] {
        assert_eq!(valid.parse::<Region>().unwrap().as_str(), valid);
    }
    assert!("a".repeat(32).parse::<Region>().is_ok());
    assert!("a".repeat(33).parse::<Region>().is_err());
    for invalid in [
        "",
        "-a",
        "a-",
        "a--b",
        "EU-central-1",
        " eu-central-1",
        "eu-central-1\n",
        "eu_central_1",
        "eu.central.1",
        "éu-central-1",
        "a/b",
        "a\0b",
    ] {
        assert!(invalid.parse::<Region>().is_err(), "{invalid:?}");
    }
}

#[test]
fn independent_genesis_vector_binds_exact_marker_and_replay_head() {
    let bytes = include_bytes!("fixtures/genesis-v2.json");
    let expected = include_str!("fixtures/genesis-v2.sha256").trim();
    let genesis: EnvelopeV2 = canonical::decode(bytes).unwrap();
    assert_eq!(canonical::encode(&genesis).unwrap(), bytes);
    assert_eq!(canonical::sha256(bytes), expected);
    let marker: AuthorityRootV2 =
        canonical::decode(include_bytes!("fixtures/authority-v2.json")).unwrap();
    assert_eq!(
        genesis.root_sha256.as_str(),
        include_str!("fixtures/authority-v2.sha256").trim()
    );
    assert_eq!(genesis.root_sha256, marker.digest().unwrap());
    let state = AuthorityStateV2::default()
        .apply(&genesis, &marker)
        .unwrap();
    assert_eq!(state.sequence, 1);
    assert_eq!(state.head.unwrap().as_str(), expected);
}

#[test]
fn independent_genesis_rejects_incompatible_facts_and_invalid_provenance() {
    let original: EnvelopeV2 =
        canonical::decode(include_bytes!("fixtures/genesis-v2.json")).unwrap();
    let marker: AuthorityRootV2 =
        canonical::decode(include_bytes!("fixtures/authority-v2.json")).unwrap();
    for field in [
        "authority",
        "format",
        "schema",
        "root",
        "account",
        "authority-id",
        "region",
        "tool-schema",
        "tool-package",
        "dirty",
        "revision",
        "lock",
        "version",
    ] {
        let mut e = original.clone();
        match field {
            "authority" => e.authority = "unknown".into(),
            "format" => e.format = "unknown".into(),
            "schema" => e.schema_version = 1,
            "root" => e.root_sha256 = "f".repeat(64).parse().unwrap(),
            "account" => e.account_id = "222222222222".parse().unwrap(),
            "authority-id" => e.authority_id = "other".parse().unwrap(),
            "region" => e.region = "eusc-de-east-1".parse().unwrap(),
            "tool-schema" => e.tool.schema_version = 1,
            "tool-package" => e.tool.package = "other".into(),
            "dirty" => e.tool.source_clean = false,
            "revision" => e.tool.source_revision = "invalid".into(),
            "lock" => e.tool.cargo_lock_sha256 = "INVALID".into(),
            _ => e.tool.package_version = "".into(),
        }
        assert!(
            AuthorityStateV2::default().apply(&e, &marker).is_err(),
            "{field}"
        );
    }
}

use borrowser_host_lifecycle::{
    canonical, collector_config::*, deployment::*, identity::*, launch::*, trust::*,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> T {
    canonical::decode(bytes).unwrap()
}
fn deployment() -> DeploymentV2 {
    decode(include_bytes!("fixtures/reviewed-deployment-v2.json"))
}
fn approval() -> LaunchApprovalV2 {
    decode(include_bytes!("fixtures/launch-approval-v2.json"))
}
fn trust() -> IdentityTrustV2 {
    decode(include_bytes!("fixtures/identity-trust-v2.json"))
}
fn spec() -> LaunchSpecV2 {
    decode(include_bytes!("fixtures/launch-spec-v2.json"))
}
fn request() -> RunInstancesRequestV2 {
    decode(include_bytes!("fixtures/run-instances-request-v2.json"))
}
fn vector<T: DeserializeOwned + Serialize>(bytes: &[u8], hash: &str) {
    let v: T = decode(bytes);
    assert_eq!(canonical::encode(&v).unwrap(), bytes);
    assert_eq!(canonical::sha256(bytes), hash.trim());
}
#[test]
fn independent_contract_vectors_and_constructors_agree() {
    macro_rules! check {
        ($ty:ty, $name:literal) => {
            vector::<$ty>(
                include_bytes!(concat!("fixtures/", $name, ".json")),
                include_str!(concat!("fixtures/", $name, ".sha256")),
            );
        };
    }
    check!(DeploymentV2, "reviewed-deployment-v2");
    check!(LaunchApprovalV2, "launch-approval-v2");
    check!(LaunchSpecV2, "launch-spec-v2");
    check!(RunInstancesRequestV2, "run-instances-request-v2");
    check!(IdentityTrustV2, "identity-trust-v2");
    check!(CollectorConfigV2, "collector-config-v2");
    let (d, a, t, s) = (deployment(), approval(), trust(), spec());
    assert_eq!(
        d.marker().unwrap(),
        decode::<AuthorityRootV2>(include_bytes!("fixtures/authority-v2.json"))
    );
    assert_eq!(a.deployment_sha256, d.digest().unwrap());
    assert_eq!(a.identity_trust_sha256, t.digest().unwrap());
    a.validate(&d, &t).unwrap();
    assert_eq!(a.launch.policy, LaunchPolicyV2::fixed());
    assert_eq!(
        LaunchSpecV2::new(&d, &a, &t, s.operation_id.clone()).unwrap(),
        s
    );
    assert_eq!(
        s.fingerprint().unwrap().as_str(),
        include_str!("fixtures/launch-spec-v2.sha256").trim()
    );
    let token = ClientToken::derive(&s, &d, &a, &t).unwrap();
    assert_eq!(
        token.as_str(),
        include_str!("fixtures/client-token-v2.txt").trim()
    );
    let binding = include_bytes!("fixtures/client-token-binding-v2.json");
    assert_eq!(canonical::sha256(binding), token.as_str());
    assert_eq!(
        canonical::sha256(binding),
        include_str!("fixtures/client-token-binding-v2.sha256").trim()
    );
    let r = RunInstancesRequestV2::new(&d, &a, &t, s, token).unwrap();
    assert_eq!(r, request());
    assert_eq!(
        r.fingerprint(&d, &a, &t).unwrap().as_str(),
        include_str!("fixtures/run-instances-request-v2.sha256").trim()
    );
    let c: CollectorConfigV2 = decode(include_bytes!("fixtures/collector-config-v2.json"));
    assert_eq!(c.user_data_bytes().unwrap(), a.launch.user_data.as_bytes());
}

#[test]
fn all_documents_reject_unknown_duplicate_fields_and_noncanonical_encoding() {
    fn check<T: Serialize + DeserializeOwned>(bytes: &[u8]) {
        let raw = std::str::from_utf8(bytes).unwrap();
        for altered in [
            raw.replacen('{', "{\"unknown\":null,", 1),
            raw.replacen('{', "{\"schema_version\":2,", 1),
            format!("{raw} "),
        ] {
            assert!(canonical::decode::<T>(altered.as_bytes()).is_err());
        }
    }
    check::<DeploymentV2>(include_bytes!("fixtures/reviewed-deployment-v2.json"));
    check::<LaunchApprovalV2>(include_bytes!("fixtures/launch-approval-v2.json"));
    check::<LaunchSpecV2>(include_bytes!("fixtures/launch-spec-v2.json"));
    check::<RunInstancesRequestV2>(include_bytes!("fixtures/run-instances-request-v2.json"));
    check::<IdentityTrustV2>(include_bytes!("fixtures/identity-trust-v2.json"));
    check::<CollectorConfigV2>(include_bytes!("fixtures/collector-config-v2.json"));
    let raw = std::str::from_utf8(include_bytes!("fixtures/launch-approval-v2.json")).unwrap();
    for (from, to) in [
        ("\"launch\":{", "\"launch\":{\"unknown\":0,"),
        ("\"root_volume\":{", "\"root_volume\":{\"type\":\"gp3\","),
        ("\"policy\":{", "\"policy\":{\"architecture\":\"x86_64\","),
    ] {
        assert!(canonical::decode::<LaunchApprovalV2>(raw.replace(from, to).as_bytes()).is_err());
    }
}
fn rejects_approval(v: &Value) -> bool {
    canonical::decode::<LaunchApprovalV2>(&canonical::encode(v).unwrap())
        .and_then(|a| a.validate(&deployment(), &trust()))
        .is_err()
}
#[test]
fn every_fixed_policy_field_is_enforced() {
    let base = serde_json::to_value(approval()).unwrap();
    for (field, value) in base["launch"]["policy"].as_object().unwrap() {
        let mut changed = base.clone();
        changed["launch"]["policy"][field] = match value {
            Value::Bool(b) => json!(!b),
            Value::Number(n) => json!(n.as_u64().unwrap() + 1),
            _ => json!("unsupported"),
        };
        assert!(rejects_approval(&changed), "{field}");
    }
}
#[test]
fn unsupported_launch_fields_and_missing_capacity_reject() {
    let base = serde_json::to_value(approval()).unwrap();
    for field in [
        "key_name",
        "spot",
        "fleet",
        "auto_scaling",
        "launch_template",
        "network_interfaces",
        "additional_disks",
        "aws_sdk_request",
        "credentials",
        "public_ip",
        "ipv6_addresses",
    ] {
        let mut changed = base.clone();
        changed["launch"][field] = json!({});
        assert!(rejects_approval(&changed), "{field}");
    }
    for field in ["size_gib", "iops", "throughput_mib_s", "kms_key_arn"] {
        let mut changed = base.clone();
        changed["launch"]["root_volume"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(rejects_approval(&changed), "{field}");
    }
    for family in ["gp2", "io2", "standard"] {
        let mut changed = base.clone();
        changed["launch"]["root_volume"]["type"] = json!(family);
        assert!(rejects_approval(&changed));
    }
}
#[test]
fn approval_deployment_and_trust_bindings_reject_changes() {
    let base = serde_json::to_value(approval()).unwrap();
    for (pointer, value) in [
        ("/format", json!("unknown")),
        ("/schema_version", json!(1)),
        ("/deployment_sha256", json!("c".repeat(64))),
        ("/infrastructure_sha256", json!("c".repeat(64))),
        ("/identity_trust_sha256", json!("c".repeat(64))),
        ("/launch/account_id", json!("222222222222")),
        ("/launch/region", json!("us-gov-west-1")),
        ("/launch/availability_zone", json!("eu-central-1b")),
        ("/launch/availability_zone_id", json!("euc1-az2")),
        ("/launch/vpc_id", json!("vpc-00000000000000002")),
        ("/launch/subnet_id", json!("subnet-00000000000000002")),
        (
            "/launch/instance_profile_id",
            json!("AIPA11111111111111111"),
        ),
        ("/launch/role_unique_id", json!("AROA11111111111111111")),
        (
            "/launch/instance_profile_arn",
            json!("arn:aws:iam::111111111111:instance-profile/other"),
        ),
        (
            "/launch/role_arn",
            json!("arn:aws:iam::111111111111:role/other"),
        ),
        (
            "/launch/root_volume/kms_key_arn",
            json!("arn:aws:kms:eu-central-1:111111111111:key/00000000-0000-0000-0000-000000000002"),
        ),
        ("/launch/root_volume/size_gib", json!(129)),
        ("/launch/root_volume/iops", json!(5001)),
        ("/launch/root_volume/throughput_mib_s", json!(501)),
        ("/resource_ceilings/max_instances", json!(2)),
        ("/resource_ceilings/max_memory_mib", json!(0)),
        ("/cost_ceilings/currency", json!("usd")),
        ("/cost_ceilings/max_hourly_microunits", json!(1)),
        ("/review/reviewer", json!("")),
    ] {
        let mut changed = base.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(rejects_approval(&changed), "{pointer}");
    }
    let mut d = deployment();
    d.reviewed_support = None;
    d.validate().unwrap();
    assert!(approval().validate(&d, &trust()).is_err());
}
#[test]
fn deployment_static_identity_and_same_region_requirements() {
    let base = serde_json::to_value(deployment()).unwrap();
    for (pointer, value) in [
        (
            "/reviewed_support/evidence_bucket_region",
            json!("us-east-1"),
        ),
        (
            "/reviewed_support/s3_gateway_endpoint_region",
            json!("us-east-1"),
        ),
        (
            "/reviewed_support/instance_profile_arn",
            json!("arn:aws:iam::222222222222:instance-profile/x"),
        ),
        (
            "/reviewed_support/role_arn",
            json!("arn:aws-cn:iam::111111111111:role/x"),
        ),
        (
            "/reviewed_support/kms_key_arn",
            json!("arn:aws:kms:us-east-1:111111111111:key/00000000-0000-0000-0000-000000000001"),
        ),
    ] {
        let mut changed = base.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            canonical::decode::<DeploymentV2>(&canonical::encode(&changed).unwrap())
                .and_then(|d| d.validate())
                .is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn sorted_unique_bounded_collections() {
    let mut d = deployment();
    for ids in [
        vec![],
        vec!["sg-00000001"; 6],
        vec!["sg-00000001"; 2],
        vec!["sg-00000002", "sg-00000001"],
    ] {
        d.reviewed_support.as_mut().unwrap().security_group_ids =
            ids.iter().map(|s| s.parse().unwrap()).collect();
        assert!(d.validate().is_err());
    }
    for collection in ["instance", "volume", "network_interface"] {
        for tags in [
            json!([]),
            json!([{"key":"x","value":"a"},{"key":"x","value":"b"}]),
            json!([{"key":"z","value":"a"},{"key":"a","value":"b"}]),
            json!([{"key":"aws:reserved","value":"a"}]),
            json!([{"key":"borrowser:operation-id","value":"a"}]),
        ] {
            let mut a = serde_json::to_value(approval()).unwrap();
            a["launch"]["tags"][collection] = tags;
            assert!(rejects_approval(&a));
        }
        let mut a = serde_json::to_value(approval()).unwrap();
        a["launch"]["tags"][collection] = json!(
            (0..17)
                .map(|i| json!({"key":format!("key{i:02}"),"value":"x"}))
                .collect::<Vec<_>>()
        );
        assert!(rejects_approval(&a));
    }
}

/// Rebind an independently edited reviewed deployment, not a production default.
fn rebound(d: &mut DeploymentV2, a: &mut LaunchApprovalV2, t: &IdentityTrustV2) {
    d.reviewed_support.as_mut().unwrap().identity_trust_sha256 = t.digest().unwrap();
    a.deployment_sha256 = d.digest().unwrap();
    a.identity_trust_sha256 = t.digest().unwrap();
    let s = d.support().unwrap();
    a.launch.account_id = d.identity.account_id.clone();
    a.launch.region = d.identity.region.clone();
    a.launch.availability_zone_id = s.availability_zone_id.clone();
    a.launch.availability_zone = s.availability_zone.clone();
    a.launch.role_arn = s.role_arn.clone();
    a.launch.instance_profile_arn = s.instance_profile_arn.clone();
    let RootVolumeV2::Gp3 { kms_key_arn, .. } = &mut a.launch.root_volume;
    *kms_key_arn = s.kms_key_arn.clone();
    let mut c: CollectorConfigV2 = decode(a.launch.user_data.as_bytes());
    c.account_id = a.launch.account_id.clone();
    c.region = a.launch.region.clone();
    a.launch.user_data = String::from_utf8(c.user_data_bytes().unwrap()).unwrap();
}
#[test]
fn client_token_changes_with_each_binding_dimension() {
    let original = request().client_token;
    for change in [
        "authority",
        "operation",
        "account",
        "region",
        "az",
        "instance-type",
        "image",
        "size",
        "iops",
        "throughput",
        "tags",
    ] {
        let (mut d, mut a, mut t) = (deployment(), approval(), trust());
        let mut op: OperationId = "synthetic-operation-1".parse().unwrap();
        match change {
            "authority" => d.identity.authority_id = "other-authority".parse().unwrap(),
            "operation" => op = "other-operation".parse().unwrap(),
            "account" => {
                d.identity.account_id = "222222222222".parse().unwrap();
                let s = d.reviewed_support.as_mut().unwrap();
                s.role_arn = s
                    .role_arn
                    .replace("111111111111", "222222222222")
                    .parse()
                    .unwrap();
                s.instance_profile_arn = s
                    .instance_profile_arn
                    .replace("111111111111", "222222222222")
                    .parse()
                    .unwrap();
                s.kms_key_arn = s
                    .kms_key_arn
                    .replace("111111111111", "222222222222")
                    .parse()
                    .unwrap();
            }
            "region" => {
                d.identity.region = "eusc-de-east-1".parse().unwrap();
                t.region = d.identity.region.clone();
                let s = d.reviewed_support.as_mut().unwrap();
                s.evidence_bucket_region = t.region.clone();
                s.s3_gateway_endpoint_region = t.region.clone();
                s.kms_key_arn = s
                    .kms_key_arn
                    .replace("eu-central-1", "eusc-de-east-1")
                    .parse()
                    .unwrap();
                s.availability_zone = "eusc-de-east-1a".parse().unwrap();
            }
            "az" => {
                d.reviewed_support.as_mut().unwrap().availability_zone_id =
                    "euc1-az2".parse().unwrap()
            }
            "instance-type" => a.launch.instance_type = "synthetic.xlarge".parse().unwrap(),
            "image" => a.launch.ami.image_id = "ami-00000000000000002".parse().unwrap(),
            "size" => {
                let RootVolumeV2::Gp3 { size_gib, .. } = &mut a.launch.root_volume;
                *size_gib += 1;
            }
            "iops" => {
                let RootVolumeV2::Gp3 { iops, .. } = &mut a.launch.root_volume;
                *iops += 1;
            }
            "throughput" => {
                let RootVolumeV2::Gp3 {
                    throughput_mib_s, ..
                } = &mut a.launch.root_volume;
                *throughput_mib_s += 1;
            }
            _ => a.launch.tags.instance[0].value = "different".into(),
        }
        rebound(&mut d, &mut a, &t);
        let s = LaunchSpecV2::new(&d, &a, &t, op).unwrap();
        let token = ClientToken::derive(&s, &d, &a, &t).unwrap();
        assert_ne!(token, original, "{change}");
        assert_eq!(token, ClientToken::derive(&s, &d, &a, &t).unwrap());
        assert_eq!(token.as_str().len(), 64);
        canonical::digest(token.as_str()).unwrap();
        assert!(RunInstancesRequestV2::new(&d, &a, &t, s, original.clone()).is_err());
    }
    let bytes = String::from_utf8(canonical::encode(&spec()).unwrap()).unwrap();
    assert!(!bytes.contains("client_token"));
    for bad in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "z".repeat(64),
    ] {
        assert!(ClientToken::try_from(bad).is_err());
    }
}
#[test]
fn final_request_and_spec_cannot_be_rebound() {
    let base = serde_json::to_value(request()).unwrap();
    for (pointer, value) in [
        ("/format", json!("unknown")),
        ("/schema_version", json!(1)),
        ("/min_count", json!(0)),
        ("/max_count", json!(2)),
        ("/client_token", json!("a".repeat(64))),
        ("/spec_sha256", json!("a".repeat(64))),
        ("/spec/format", json!("unknown")),
        ("/spec/schema_version", json!(3)),
        ("/spec/operation_id", json!("other")),
        ("/spec/launch/instance_type", json!("synthetic.other")),
        ("/spec/launch_approval_sha256", json!("a".repeat(64))),
        ("/spec/launch/tags/instance/0/value", json!("other")),
    ] {
        let mut v = base.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        assert!(
            canonical::decode::<RunInstancesRequestV2>(&canonical::encode(&v).unwrap())
                .and_then(|r| r.validate(&deployment(), &approval(), &trust()))
                .is_err(),
            "{pointer}"
        );
    }
    let mut a = approval();
    a.review.reference = "another-review".parse().unwrap();
    assert!(request().validate(&deployment(), &a, &trust()).is_err());
}
#[test]
fn trust_is_bounded_reviewed_bytes_not_cryptographic_verification() {
    let t = trust();
    t.validate().unwrap();
    assert_eq!(t.certificate_bytes().unwrap(), [0x30, 0]); // Deliberately not a certificate.
    for field in [
        "format",
        "schema_version",
        "certificate_der_hex",
        "certificate_sha256",
        "rsa_modulus_bits",
        "rsa_public_exponent",
    ] {
        let mut v = serde_json::to_value(&t).unwrap();
        v[field] = match field {
            "schema_version" => json!(1),
            "rsa_modulus_bits" => json!(1024),
            "rsa_public_exponent" => json!(3),
            "certificate_sha256" => json!("a".repeat(64)),
            _ => json!("invalid"),
        };
        assert!(
            canonical::decode::<IdentityTrustV2>(&canonical::encode(&v).unwrap())
                .and_then(|t| t.validate())
                .is_err()
        );
    }
    for hex in [
        "".into(),
        "3".into(),
        "GG".into(),
        "AB".into(),
        "00".repeat(16_385),
    ] {
        let mut changed = t.clone();
        changed.certificate_der_hex = hex;
        assert!(changed.validate().is_err());
    }
    let mut changed = t.clone();
    changed.region = "us-east-1".parse().unwrap();
    assert!(approval().validate(&deployment(), &changed).is_err());
    for when in [
        t.review.valid_from_unix_seconds - 1,
        t.review.valid_until_unix_seconds,
    ] {
        assert!(t.review.validate_at(when).is_err());
    }
    t.review
        .validate_at(t.review.valid_from_unix_seconds)
        .unwrap();
    let mut changed = t.clone();
    changed.review.valid_until_unix_seconds = changed.review.valid_from_unix_seconds;
    assert!(changed.validate().is_err());
    let mut changed = t;
    changed.review.reviewed_at_unix_seconds = changed.review.valid_from_unix_seconds + 1;
    assert!(changed.validate().is_err());
}
#[test]
fn collector_has_no_executable_or_bearer_fields_and_bytes_are_bound() {
    let base: Value =
        serde_json::from_slice(include_bytes!("fixtures/collector-config-v2.json")).unwrap();
    for field in [
        "command",
        "shell",
        "cloud_init",
        "executable_path",
        "url",
        "presigned_url",
        "credentials",
        "operation_id",
    ] {
        let mut v = base.clone();
        v[field] = json!("unsupported");
        assert!(canonical::decode::<CollectorConfigV2>(&canonical::encode(&v).unwrap()).is_err());
    }
    let mut a = approval();
    a.launch.user_data = "#!/bin/sh\n".into();
    assert!(a.validate(&deployment(), &trust()).is_err());
    a.launch.user_data = "x".repeat(1025);
    assert!(a.validate(&deployment(), &trust()).is_err());
    for (field, value) in [
        ("format", "other"),
        ("region", "us-east-1"),
        ("account_id", "222222222222"),
        ("role_unique_id", "AROA11111111111111111"),
        ("evidence_bucket", "other-bucket"),
        ("ingress_scheme", "other-prefix"),
    ] {
        let mut v = base.clone();
        v[field] = json!(value);
        a.launch.user_data = String::from_utf8(canonical::encode(&v).unwrap()).unwrap();
        assert!(a.validate(&deployment(), &trust()).is_err(), "{field}");
        let mut s = spec();
        s.launch.user_data = a.launch.user_data.clone();
        assert_ne!(s.fingerprint().unwrap(), spec().fingerprint().unwrap());
    }
}
#[test]
fn lexical_identifiers_do_not_admit_malformed_aws_identities() {
    macro_rules! rejects { ($ty:ty, $($v:expr),+) => { $(assert!($v.parse::<$ty>().is_err(), "{}", $v);)+ }; }
    rejects!(AmiId, "ami-", "ami-ABCDEF00", "i-00000000");
    rejects!(VpcId, "subnet-00000000", "vpc-nope");
    rejects!(SubnetId, "subnet-", "subnet-0000000Z");
    rejects!(SecurityGroupId, "default", "sg-0000000G");
    rejects!(AvailabilityZone, "", "a--b", " A");
    rejects!(AvailabilityZoneId, "", "a_b");
    rejects!(InstanceType, "", "x..large", "m7I.large", "x large");
    rejects!(
        InstanceProfileArn,
        "arn:aws:iam::111111111111:role/x",
        "arn:aws:iam::111111111111:instance-profile/*"
    );
    rejects!(IamRoleId, "", "aroa1", "ROLE-1");
    rejects!(
        KmsKeyArn,
        "arn:aws:kms:eu-central-1:111111111111:alias/x",
        "arn:aws:kms:eu-central-1:111111111111:key/*"
    );
    rejects!(
        EvidenceBucketName,
        "ab",
        "https://bucket",
        "A-bucket",
        "bucket/path",
        "192.168.1.1"
    );
    assert!("synthetic.large".parse::<InstanceType>().is_ok());
    assert!("future-zone".parse::<AvailabilityZone>().is_ok());
}

#[test]
fn structural_review_intervals_and_capacity_ratios_fail_closed() {
    let base = serde_json::to_value(approval()).unwrap();
    for (pointer, value) in [
        ("/review/valid_until_unix_seconds", json!(1800003601u64)),
        ("/review/valid_from_unix_seconds", json!(1799999999u64)),
        ("/launch/root_volume/size_gib", json!(0)),
        ("/launch/root_volume/iops", json!(2999)),
        ("/launch/root_volume/throughput_mib_s", json!(124)),
        (
            "/cost_ceilings/review/valid_until_unix_seconds",
            json!(1800003599u64),
        ),
        (
            "/launch/ami/review/valid_until_unix_seconds",
            json!(1800003599u64),
        ),
    ] {
        let mut v = base.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        assert!(rejects_approval(&v), "{pointer}");
    }
    let mut a = approval();
    a.resource_ceilings.max_iops = 16000;
    a.resource_ceilings.max_throughput_mib_s = 1000;
    let RootVolumeV2::Gp3 {
        size_gib,
        iops,
        throughput_mib_s,
        ..
    } = &mut a.launch.root_volume;
    *size_gib = 1;
    *iops = 4000;
    *throughput_mib_s = 250;
    assert!(a.validate(&deployment(), &trust()).is_err());
    let RootVolumeV2::Gp3 {
        size_gib,
        iops,
        throughput_mib_s,
        ..
    } = &mut a.launch.root_volume;
    *size_gib = 64;
    *iops = 3000;
    *throughput_mib_s = 751;
    assert!(a.validate(&deployment(), &trust()).is_err());
    let mut t = serde_json::to_value(trust()).unwrap();
    for profile in ["raw-rsa2048", "pkcs7-sha1", "rsa2048-cms-sha512"] {
        t["profile"] = json!(profile);
        assert!(canonical::decode::<IdentityTrustV2>(&canonical::encode(&t).unwrap()).is_err());
    }
}

#[test]
fn public_contracts_do_not_extend_the_production_event_or_cli_surface() {
    use borrowser_host_lifecycle::model::EventV2;
    for name in [
        "launch-approved",
        "launch-dispatch",
        "launch-specification",
        "identity-trust-imported",
    ] {
        assert!(
            canonical::decode::<EventV2>(&canonical::encode(&json!({"kind":name})).unwrap())
                .is_err()
        );
    }
    for name in [
        "launch",
        "approve",
        "trust",
        "collector",
        "run-instances",
        "terminate-instances",
    ] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_borrowser-host-lifecycle"))
            .arg(name)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("provider operations unavailable")
        );
    }
    // #1396 deliberately adds only the audited service SDKs. Frozen event/CLI
    // assertions above remain unchanged; provider transmission stays unavailable.
    let manifest = include_str!("../Cargo.toml");
    for service in ["aws-sdk-ec2", "aws-sdk-sts", "aws-sdk-s3"] {
        assert!(manifest.contains(service));
    }
    for dep in [
        "aws-sdk-iam",
        "aws-sdk-kms",
        "aws-config",
        "reqwest",
        "hyper",
        "openssl",
        "x509",
        "cms =",
    ] {
        assert!(
            !manifest.contains(dep),
            "unexpected production dependency {dep}"
        );
    }
}

#[test]
fn gp3_api_bounds_are_not_fixed_qualification_capacity() {
    let mut a = approval();
    a.resource_ceilings.max_root_size_gib = 65_536;
    a.resource_ceilings.max_iops = 80_000;
    a.resource_ceilings.max_throughput_mib_s = 2000;
    let key = deployment().support().unwrap().kms_key_arn.clone();
    a.launch.root_volume = RootVolumeV2::Gp3 {
        size_gib: 65_536,
        iops: 80_000,
        throughput_mib_s: 2000,
        kms_key_arn: key.clone(),
    };
    a.validate(&deployment(), &trust()).unwrap(); // Structural only; never pricing/admission.
    for (size_gib, iops, throughput_mib_s) in [
        (65_537, 80_000, 2000),
        (65_536, 80_001, 2000),
        (65_536, 80_000, 2001),
        (u64::MAX, u64::MAX, u64::MAX),
    ] {
        a.launch.root_volume = RootVolumeV2::Gp3 {
            size_gib,
            iops,
            throughput_mib_s,
            kms_key_arn: key.clone(),
        };
        assert!(a.validate(&deployment(), &trust()).is_err());
    }
}

#[test]
fn planned_cost_uses_checked_currency_times_duration_and_ceil_rounding() {
    for (rate, seconds, expected) in [
        (1, 1, 1),
        (1, 3599, 1),
        (1, 3600, 1),
        (1, 3601, 2),
        (3601, 1, 2),
        (1000, 1800, 500),
        (1, MAX_PLANNED_RUNTIME_SECONDS, 168),
        (u64::MAX, 1, u64::MAX / 3600 + 1),
    ] {
        let mut a = approval();
        set_test_hourly(&mut a, rate);
        a.cost_ceilings.max_hourly_microunits = rate;
        a.cost_ceilings.planned_max_runtime_seconds = seconds;
        a.cost_ceilings.max_operation_microunits = expected;
        assert_eq!(
            a.cost_ceilings.planned_operation_cost_microunits().unwrap(),
            expected
        );
        a.validate(&deployment(), &trust()).unwrap(); // Equality to total is sufficient.
        a.cost_ceilings.max_operation_microunits = expected - 1;
        assert!(a.validate(&deployment(), &trust()).is_err());
    }
    // An hourly rate is not directly comparable with a sub-hour currency total.
    let mut a = approval();
    set_test_hourly(&mut a, 3600);
    a.cost_ceilings.max_hourly_microunits = 7200;
    a.cost_ceilings.planned_max_runtime_seconds = 1;
    a.cost_ceilings.max_operation_microunits = 1;
    a.validate(&deployment(), &trust()).unwrap();
}

#[test]
fn planned_cost_rejects_missing_zero_excessive_duration_overflow_and_rate_violation() {
    for seconds in [0, MAX_PLANNED_RUNTIME_SECONDS + 1, u64::MAX] {
        let mut a = approval();
        a.cost_ceilings.planned_max_runtime_seconds = seconds;
        assert!(a.cost_ceilings.planned_operation_cost_microunits().is_err());
        assert!(a.validate(&deployment(), &trust()).is_err());
    }
    let mut a = approval();
    set_test_hourly(&mut a, u64::MAX);
    a.cost_ceilings.max_hourly_microunits = u64::MAX;
    a.cost_ceilings.max_operation_microunits = u64::MAX;
    a.cost_ceilings.planned_max_runtime_seconds = 2;
    assert!(a.cost_ceilings.planned_operation_cost_microunits().is_err());
    assert!(a.validate(&deployment(), &trust()).is_err());
    a = approval();
    a.cost_ceilings.max_hourly_microunits =
        a.cost_ceilings.reviewed_hourly.total_microunits().unwrap() - 1;
    assert!(a.validate(&deployment(), &trust()).is_err());
    let mut value = serde_json::to_value(approval()).unwrap();
    value["cost_ceilings"]
        .as_object_mut()
        .unwrap()
        .remove("planned_max_runtime_seconds");
    assert!(rejects_approval(&value));
}

#[test]
fn planned_duration_changes_every_approval_binding_without_granting_timer_authority() {
    let (d, t, old_approval) = (deployment(), trust(), approval());
    let old_spec = spec();
    let old_request = request();
    let mut a = old_approval.clone();
    a.cost_ceilings.planned_max_runtime_seconds += 1;
    let s = LaunchSpecV2::new(&d, &a, &t, old_spec.operation_id.clone()).unwrap();
    let token = ClientToken::derive(&s, &d, &a, &t).unwrap();
    let r = RunInstancesRequestV2::new(&d, &a, &t, s.clone(), token.clone()).unwrap();
    assert_ne!(
        a.digest(&d, &t).unwrap(),
        old_approval.digest(&d, &t).unwrap()
    );
    assert_ne!(s.fingerprint().unwrap(), old_spec.fingerprint().unwrap());
    assert_ne!(token, old_request.client_token);
    assert_ne!(
        r.fingerprint(&d, &a, &t).unwrap(),
        old_request.fingerprint(&d, &old_approval, &t).unwrap()
    );
    assert!(old_spec.validate(&d, &a, &t).is_err());
    assert!(old_request.validate(&d, &a, &t).is_err());
    assert!(RunInstancesRequestV2::new(&d, &a, &t, s, old_request.client_token).is_err());
    use borrowser_host_lifecycle::model::EventV2;
    for seconds in [0, 1, MAX_PLANNED_RUNTIME_SECONDS, u64::MAX] {
        let event = json!({"kind":"termination-authorized","planned_max_runtime_seconds":seconds});
        assert!(canonical::decode::<EventV2>(&canonical::encode(&event).unwrap()).is_err());
    }
    for command in ["timeout", "stop", "terminate"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_borrowser-host-lifecycle"))
            .args([command, "604800"])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn opaque_iam_unique_ids_preserve_bytes_without_prefix_history_or_approval() {
    for value in [
        "abcdefghijklmnop",
        "ABCDEFGHIJKLMNOP",
        "0123456789012345",
        "Mixed_case_123456",
        "AIPA00000000000000000",
        "Legacy_Role_00001",
        "Future_Profile02",
    ] {
        assert_eq!(value.parse::<IamRoleId>().unwrap().as_str(), value);
        assert_eq!(value.parse::<InstanceProfileId>().unwrap().as_str(), value);
    }
    assert!("Z".repeat(128).parse::<IamRoleId>().is_ok());
    assert!("Z".repeat(128).parse::<InstanceProfileId>().is_ok());
    for value in [
        "".into(),
        "Z".repeat(15),
        "Z".repeat(129),
        "abcdefghijklmno ".into(),
        "abcdefghijklmno-".into(),
        "abcdefghijklmno:".into(),
        "abcdefghijklmno/".into(),
        "abcdefghijklmnoé".into(),
        "abcdefghijklmno\n".into(),
        "abcdefghijklmno\0".into(),
        "abcdefghijklmno\t".into(),
    ] {
        assert!(value.parse::<IamRoleId>().is_err());
        assert!(value.parse::<InstanceProfileId>().is_err());
    }
    let mut a = approval();
    a.launch.role_unique_id = "SAFEOPAQUEBUTUNREVIEWED".parse().unwrap();
    assert!(a.validate(&deployment(), &trust()).is_err());
    a = approval();
    a.launch.instance_profile_id = "SAFEOPAQUEBUTUNREVIEWED".parse().unwrap();
    assert!(a.validate(&deployment(), &trust()).is_err());
    let mut config: CollectorConfigV2 = decode(approval().launch.user_data.as_bytes());
    config.role_unique_id = "Legacy_Role_00001".parse().unwrap();
    a = approval();
    a.launch.user_data = String::from_utf8(config.user_data_bytes().unwrap()).unwrap();
    assert!(a.validate(&deployment(), &trust()).is_err());
}

#[test]
fn opaque_ec2_resource_suffixes_are_bounded_not_a_catalogue() {
    macro_rules! check {
        ($ty:ty, $prefix:literal) => {{
            for suffix in [
                "a".into(),
                "123".into(),
                "a".repeat(8),
                "b".repeat(17),
                "c".repeat(32),
                "d".repeat(64),
            ] {
                let s = format!("{}{}", $prefix, suffix);
                assert_eq!(s.parse::<$ty>().unwrap().as_str(), s);
            }
            for suffix in [
                "".into(),
                "a".repeat(65),
                "ABC".into(),
                "g".into(),
                "a/b".into(),
                "a-b".into(),
                "a ".into(),
                "é".into(),
            ] {
                assert!(format!("{}{}", $prefix, suffix).parse::<$ty>().is_err());
            }
            assert!("wrong-a".parse::<$ty>().is_err());
        }};
    }
    check!(AmiId, "ami-");
    check!(VpcId, "vpc-");
    check!(SubnetId, "subnet-");
    check!(SecurityGroupId, "sg-");
    check!(VpcEndpointId, "vpce-");
    check!(RouteTableId, "rtb-");
    let mut a = approval();
    a.launch.subnet_id = "subnet-a".parse().unwrap();
    assert!(a.validate(&deployment(), &trust()).is_err());
}

#[test]
fn reboot_migration_is_not_an_authoritative_projection_or_fallback_mutation() {
    let mut a = serde_json::to_value(approval()).unwrap();
    assert_eq!(a["launch"]["policy"]["automatic_recovery"], "disabled");
    assert!(a["launch"]["policy"].get("reboot_migration").is_none());
    a["launch"]["policy"]["reboot_migration"] = json!("disabled");
    assert!(rejects_approval(&a));
    let contract =
        include_str!("../../../../docs/conformance/ag9g0d-run-instances-projection-v2.md");
    for obligation in [
        "AutoRecovery",
        "Pass 4 must inspect",
        "pinned AWS Rust SDK RunInstances input",
        "ModifyInstanceMaintenanceOptions",
        "No qualification conclusion",
    ] {
        assert!(contract.contains(obligation), "{obligation}");
    }
}

fn set_test_hourly(a: &mut LaunchApprovalV2, rate: u64) {
    a.cost_ceilings.reviewed_hourly = ReviewedHourlyCostV2 {
        compute_microunits: rate,
        root_ebs_microunits: 0,
        applicable_other_microunits: 0,
    };
    a.cost_ceilings.fixed_operation_microunits = 0;
}

#[test]
fn reviewed_cost_breakdown_checks_sums_fixed_cost_and_explicit_fields() {
    let mut a = approval();
    a.cost_ceilings.reviewed_hourly = ReviewedHourlyCostV2 {
        compute_microunits: 1,
        root_ebs_microunits: 2,
        applicable_other_microunits: 0,
    };
    a.cost_ceilings.fixed_operation_microunits = 7;
    a.cost_ceilings.planned_max_runtime_seconds = 1;
    a.cost_ceilings.max_hourly_microunits = 3;
    a.cost_ceilings.max_operation_microunits = 8;
    assert_eq!(
        a.cost_ceilings.reviewed_hourly.total_microunits().unwrap(),
        3
    );
    assert_eq!(
        a.cost_ceilings.planned_variable_cost_microunits().unwrap(),
        1
    );
    assert_eq!(
        a.cost_ceilings.planned_operation_cost_microunits().unwrap(),
        8
    );
    a.validate(&deployment(), &trust()).unwrap();
    a.cost_ceilings.max_operation_microunits = 7;
    assert!(a.validate(&deployment(), &trust()).is_err());
    a.cost_ceilings.max_operation_microunits = 8;
    a.cost_ceilings.max_hourly_microunits = 2;
    assert!(a.validate(&deployment(), &trust()).is_err());
    for (compute, ebs, other) in [(u64::MAX, 1, 0), (u64::MAX, 0, 1), (0, u64::MAX, 1)] {
        a.cost_ceilings.reviewed_hourly = ReviewedHourlyCostV2 {
            compute_microunits: compute,
            root_ebs_microunits: ebs,
            applicable_other_microunits: other,
        };
        assert!(a.cost_ceilings.reviewed_hourly.total_microunits().is_err());
        assert!(a.cost_ceilings.planned_operation_cost_microunits().is_err());
        assert!(a.validate(&deployment(), &trust()).is_err());
    }
    set_test_hourly(&mut a, 1);
    a.cost_ceilings.max_hourly_microunits = u64::MAX;
    a.cost_ceilings.max_operation_microunits = u64::MAX;
    a.cost_ceilings.fixed_operation_microunits = u64::MAX;
    assert_eq!(
        a.cost_ceilings.planned_variable_cost_microunits().unwrap(),
        1
    );
    assert!(a.cost_ceilings.planned_operation_cost_microunits().is_err());
    assert!(a.validate(&deployment(), &trust()).is_err());
    set_test_hourly(&mut a, 0);
    assert!(a.validate(&deployment(), &trust()).is_err());
    let base = serde_json::to_value(approval()).unwrap();
    for field in [
        "compute_microunits",
        "root_ebs_microunits",
        "applicable_other_microunits",
    ] {
        let mut v = base.clone();
        v["cost_ceilings"]["reviewed_hourly"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(rejects_approval(&v));
    }
    for field in ["fixed_operation_microunits", "pricing_evidence_sha256"] {
        let mut v = base.clone();
        v["cost_ceilings"].as_object_mut().unwrap().remove(field);
        assert!(rejects_approval(&v));
    }
    let mut v = base.clone();
    v["cost_ceilings"]["reviewed_hourly"]["pricing_formula"] = json!("unsupported");
    assert!(rejects_approval(&v));
    let mut v = base;
    v["cost_ceilings"]["pricing_evidence_sha256"] = json!("not-a-digest");
    assert!(rejects_approval(&v));
}

#[test]
fn every_cost_component_and_pricing_evidence_rebinds_all_artifacts() {
    let (d, t) = (deployment(), trust());
    let old = approval();
    let old_spec = spec();
    let old_request = request();
    for field in ["compute", "ebs", "other", "fixed", "pricing-evidence"] {
        let mut a = old.clone();
        match field {
            "compute" => a.cost_ceilings.reviewed_hourly.compute_microunits += 1,
            "ebs" => a.cost_ceilings.reviewed_hourly.root_ebs_microunits += 1,
            "other" => a.cost_ceilings.reviewed_hourly.applicable_other_microunits += 1,
            "fixed" => a.cost_ceilings.fixed_operation_microunits += 1,
            _ => a.cost_ceilings.pricing_evidence_sha256 = "d".repeat(64).parse().unwrap(),
        }
        assert_ne!(
            a.digest(&d, &t).unwrap(),
            old.digest(&d, &t).unwrap(),
            "{field}"
        );
        let s = LaunchSpecV2::new(&d, &a, &t, old_spec.operation_id.clone()).unwrap();
        let token = ClientToken::derive(&s, &d, &a, &t).unwrap();
        assert_ne!(
            s.fingerprint().unwrap(),
            old_spec.fingerprint().unwrap(),
            "{field}"
        );
        assert_ne!(token, old_request.client_token, "{field}");
        let r = RunInstancesRequestV2::new(&d, &a, &t, s.clone(), token).unwrap();
        assert_ne!(
            r.fingerprint(&d, &a, &t).unwrap(),
            old_request.fingerprint(&d, &old, &t).unwrap(),
            "{field}"
        );
        assert!(old_request.validate(&d, &a, &t).is_err());
        assert!(
            RunInstancesRequestV2::new(&d, &a, &t, s, old_request.client_token.clone()).is_err()
        );
        use borrowser_host_lifecycle::model::EventV2;
        assert!(
            canonical::decode::<EventV2>(&canonical::encode(&a.cost_ceilings).unwrap()).is_err()
        );
    }
}

#[test]
fn reviewed_opaque_role_id_rebinds_collector_and_launch_artifacts() {
    let (mut d, mut a, t) = (deployment(), approval(), trust());
    let id: IamRoleId = "New_role_ID_12345".parse().unwrap();
    d.reviewed_support.as_mut().unwrap().role_unique_id = id.clone();
    assert!(a.validate(&d, &t).is_err());
    a.deployment_sha256 = d.digest().unwrap();
    a.launch.role_unique_id = id.clone();
    assert!(a.validate(&d, &t).is_err()); // Collector still retains the previous ID.
    let mut c: CollectorConfigV2 = decode(a.launch.user_data.as_bytes());
    c.role_unique_id = id;
    a.launch.user_data = String::from_utf8(c.user_data_bytes().unwrap()).unwrap();
    assert_ne!(a.launch.user_data, approval().launch.user_data);
    a.validate(&d, &t).unwrap();
    let s = LaunchSpecV2::new(&d, &a, &t, spec().operation_id).unwrap();
    let token = ClientToken::derive(&s, &d, &a, &t).unwrap();
    assert_ne!(s.fingerprint().unwrap(), spec().fingerprint().unwrap());
    assert_ne!(token, request().client_token);
    let r = RunInstancesRequestV2::new(&d, &a, &t, s, token).unwrap();
    assert_ne!(
        r.fingerprint(&d, &a, &t).unwrap(),
        request()
            .fingerprint(&deployment(), &approval(), &t)
            .unwrap()
    );
}

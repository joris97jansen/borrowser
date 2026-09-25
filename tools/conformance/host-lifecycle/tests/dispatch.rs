use borrowser_host_lifecycle::{
    canonical, deployment::AuthorityRootV2, dispatch::*, model::*, scheduling::*,
};
fn root() -> AuthorityRootV2 {
    canonical::decode(include_bytes!("fixtures/authority-v2.json")).unwrap()
}
fn vector(name: &str) -> EnvelopeV2 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dispatch-v2");
    canonical::decode(&std::fs::read(path.join(format!("{name}.json"))).unwrap()).unwrap()
}
fn genesis() -> EnvelopeV2 {
    canonical::decode(include_bytes!("fixtures/genesis-v2.json")).unwrap()
}
fn documents() -> PreparedLaunchV2 {
    PreparedLaunchV2 {
        deployment: canonical::decode(include_bytes!("fixtures/reviewed-deployment-v2.json"))
            .unwrap(),
        approval: canonical::decode(include_bytes!("fixtures/launch-approval-v2.json")).unwrap(),
        trust: canonical::decode(include_bytes!("fixtures/identity-trust-v2.json")).unwrap(),
        specification: canonical::decode(include_bytes!("fixtures/launch-spec-v2.json")).unwrap(),
        request: canonical::decode(include_bytes!("fixtures/run-instances-request-v2.json"))
            .unwrap(),
        authorization: canonical::decode(include_bytes!("fixtures/dispatch-v2/authorization.json"))
            .unwrap(),
    }
}
fn state(stage: usize) -> AuthorityStateV2 {
    let mut state = AuthorityStateV2::default()
        .apply(&genesis(), &root())
        .unwrap();
    for name in [
        "prepared",
        "dispatch-intent",
        "attempt-intent",
        "definitely-not-transmitted",
    ]
    .iter()
    .take(stage)
    {
        state = state
            .apply_retained(&vector(name), &root(), Some(&documents()))
            .unwrap();
    }
    state
}
fn apply(
    state: &AuthorityStateV2,
    event: EventV2,
    time: TimeSample,
) -> borrowser_host_lifecycle::Result<AuthorityStateV2> {
    let mut e = genesis();
    e.sequence = state.sequence;
    e.previous_sha256 = state.head.clone();
    e.event = event;
    e.time = time;
    state.apply_retained(&e, &root(), Some(&documents()))
}
fn time(seconds: u64) -> TimeSample {
    let mut t = vector("prepared").time;
    t.boottime_ns += seconds * 1_000_000_000;
    t
}
#[test]
fn independent_authorization_and_event_vectors() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dispatch-v2");
    for name in [
        "authorization",
        "prepared",
        "dispatch-intent",
        "attempt-intent",
        "definitely-not-transmitted",
        "transmission-uncertain",
        "parameter-conflict",
        "access-blocked",
        "throttled-held",
        "response-unresolved",
    ] {
        let bytes = std::fs::read(path.join(format!("{name}.json"))).unwrap();
        assert_eq!(
            canonical::sha256(&bytes),
            std::fs::read_to_string(path.join(format!("{name}.sha256")))
                .unwrap()
                .trim()
        );
        if name == "authorization" {
            let auth: LaunchAuthorizationV2 = canonical::decode(&bytes).unwrap();
            assert_eq!(auth.digest().unwrap().as_str(), canonical::sha256(&bytes));
            let p = documents();
            assert_eq!(auth, p.authorization);
            assert_eq!(p.binding().unwrap(), auth.binding);
        } else {
            let e: EnvelopeV2 = canonical::decode(&bytes).unwrap();
            assert_eq!(canonical::encode(&e).unwrap(), bytes);
            let next = state(e.sequence as usize - 1)
                .apply_retained(&e, &root(), Some(&documents()))
                .unwrap();
            assert_eq!(next.head.unwrap().as_str(), canonical::sha256(&bytes));
        }
    }
    assert_eq!(
        state(2).operation.unwrap().next_attempt(&time(0)).unwrap(),
        match vector("attempt-intent").event {
            EventV2::LaunchAttemptIntent(i) => i,
            _ => panic!(),
        }
    );
}
#[test]
fn authorization_rejects_rebinding_stale_head_and_second_operation_transactionally() {
    let original = state(0);
    for field in [
        "head",
        "operation",
        "request",
        "token",
        "spec",
        "account",
        "region",
        "az",
        "approval",
        "trust",
        "deployment",
        "root",
        "reviewer",
        "future-audit",
        "generation",
        "before-review",
    ] {
        let mut e = vector("prepared");
        let mut p = documents();
        match field {
            "head" => p.authorization.expected_head = "f".repeat(64).parse().unwrap(),
            "operation" => p.authorization.binding.operation_id = "different".parse().unwrap(),
            "request" => p.request.max_count = 2,
            "token" => {
                p.request.client_token =
                    borrowser_host_lifecycle::launch::ClientToken::try_from("f".repeat(64)).unwrap()
            }
            "spec" => p.specification.launch.instance_type = "other.large".parse().unwrap(),
            "account" => p.authorization.binding.account_id = "222222222222".parse().unwrap(),
            "region" => p.authorization.binding.region = "eu-west-1".parse().unwrap(),
            "az" => p.authorization.binding.availability_zone_id = "euc1-az2".parse().unwrap(),
            "approval" => p.authorization.binding.approval_sha256 = "f".repeat(64).parse().unwrap(),
            "trust" => p.authorization.binding.trust_sha256 = "f".repeat(64).parse().unwrap(),
            "deployment" => {
                p.authorization.binding.deployment_sha256 = "f".repeat(64).parse().unwrap()
            }
            "root" => p.deployment.identity.controller_machine_id = "f".repeat(32),
            "reviewer" => {
                assert!(
                    "".parse::<borrowser_host_lifecycle::identity::ReviewText>()
                        .is_err()
                );
                continue;
            }
            "future-audit" => p.authorization.authorized_at_unix_seconds = 1800000001,
            "generation" => p.authorization.schema_version = 3,
            _ => p.authorization.authorized_at_unix_seconds = 1,
        }
        if let Ok(preparation) = p.preparation() {
            e.event = EventV2::LaunchPrepared(preparation);
            assert!(
                original.apply_retained(&e, &root(), Some(&p)).is_err(),
                "{field}"
            );
        } else {
            assert!(
                p.validate(&root(), original.head.as_ref().unwrap(), &e.time)
                    .is_err()
            );
        }
        assert_eq!(original, state(0));
    }
    let s = state(1);
    assert!(apply(&s, vector("prepared").event, time(1)).is_err());
    assert!(apply(&state(0), vector("attempt-intent").event, time(0)).is_err());
    assert!(apply(&state(1), vector("attempt-intent").event, time(0)).is_err());
}
#[test]
fn ordering_and_exact_receipt_binding() {
    assert!(apply(&state(0), vector("dispatch-intent").event, time(0)).is_err());
    assert!(apply(&state(2), vector("dispatch-intent").event, time(0)).is_err());
    assert!(
        apply(
            &state(2),
            vector("definitely-not-transmitted").event,
            time(0)
        )
        .is_err()
    );
    assert!(
        apply(
            &state(4),
            vector("definitely-not-transmitted").event,
            time(0)
        )
        .is_err()
    );
    for field in ["head", "sequence", "token", "time", "number"] {
        let mut e = vector("attempt-intent");
        let EventV2::LaunchAttemptIntent(ref mut i) = e.event else {
            panic!()
        };
        match field {
            "head" => i.dispatch.receipt.head = "f".repeat(64).parse().unwrap(),
            "sequence" => i.dispatch.receipt.sequence += 1,
            "token" => {
                i.dispatch.intent.binding.client_token =
                    borrowser_host_lifecycle::launch::ClientToken::try_from("f".repeat(64)).unwrap()
            }
            "time" => i.dispatch.time.boottime_ns += 1,
            _ => i.number = 2.try_into().unwrap(),
        }
        assert!(state(2).apply(&e, &root()).is_err(), "{field}");
    }
}
#[test]
fn pending_and_blocking_outcomes_never_grant_retry() {
    assert!(state(3).operation.unwrap().next_attempt(&time(10)).is_err());
    for name in [
        "transmission-uncertain",
        "parameter-conflict",
        "access-blocked",
        "throttled-held",
        "response-unresolved",
    ] {
        let s = state(3).apply(&vector(name), &root()).unwrap();
        for seconds in [0, 2, 8, 119, 120, 1000] {
            assert!(
                s.operation
                    .as_ref()
                    .unwrap()
                    .next_attempt(&time(seconds))
                    .is_err(),
                "{name}"
            );
        }
        assert!(apply(&s, vector("prepared").event, time(10)).is_err());
    }
}
#[test]
fn three_attempt_budget_and_two_eight_second_delays() {
    let mut s = state(4);
    let mut identities = vec![s.operation.as_ref().unwrap().attempts[0].identity.clone()];
    for (elapsed, too_early) in [(2, 1), (10, 9)] {
        assert!(
            s.operation
                .as_ref()
                .unwrap()
                .next_attempt(&time(too_early))
                .is_err()
        );
        let intent = s
            .operation
            .as_ref()
            .unwrap()
            .next_attempt(&time(elapsed))
            .unwrap();
        s = apply(&s, EventV2::LaunchAttemptIntent(intent), time(elapsed)).unwrap();
        let attempt = s
            .operation
            .as_ref()
            .unwrap()
            .attempts
            .last()
            .unwrap()
            .identity
            .clone();
        assert!(!identities.contains(&attempt));
        identities.push(attempt.clone());
        assert!(
            s.operation
                .as_ref()
                .unwrap()
                .next_attempt(&time(elapsed + 10))
                .is_err()
        );
        s = apply(
            &s,
            EventV2::LaunchAttemptOutcome(AttemptOutcomeV2 {
                attempt,
                outcome: LaunchAttemptOutcome::DefinitelyNotTransmitted,
            }),
            time(elapsed),
        )
        .unwrap();
    }
    assert_eq!(s.operation.as_ref().unwrap().attempts.len(), 3);
    assert!(s.operation.unwrap().next_attempt(&time(119)).is_err());
    assert!(LaunchAttemptNumber::try_from(4).is_err());
    assert!(LaunchAttemptNumber::try_from(0).is_err());
}
#[test]
fn deadline_reboot_namespace_realtime_and_overflow() {
    let s = state(4);
    let op = s.operation.unwrap();
    assert!(op.next_attempt(&time(119)).is_ok());
    assert!(op.next_attempt(&time(120)).is_err());
    for field in ["boot", "namespace", "regression"] {
        let mut now = time(10);
        match field {
            "boot" => now.boot_id = "00000000-0000-0000-0000-000000000002".into(),
            "namespace" => now.time_namespace = "time:[2]".into(),
            _ => now.boottime_ns = 0,
        }
        assert!(op.next_attempt(&now).is_err());
    }
    let mut now = time(10);
    now.realtime_ns = 0;
    assert!(op.next_attempt(&now).is_ok());
    now.boottime_ns = u64::MAX;
    assert!(LaunchDispatchDeadline::after(&now).is_err());
}
#[test]
fn outcome_can_be_retained_after_reboot_but_cannot_restore_retry() {
    let mut e = vector("definitely-not-transmitted");
    e.time.boot_id = "00000000-0000-0000-0000-000000000002".into();
    let s = state(3).apply(&e, &root()).unwrap();
    assert!(
        s.operation
            .as_ref()
            .unwrap()
            .next_attempt(&time(10))
            .is_err()
    );
    let mut now = time(10);
    now.boot_id = e.time.boot_id;
    assert!(s.operation.unwrap().next_attempt(&now).is_err());
}

#[test]
fn outcome_delay_starts_at_durable_observation_not_attempt_intent() {
    let mut e = vector("definitely-not-transmitted");
    e.time = time(10);
    let s = state(3).apply(&e, &root()).unwrap();
    let op = s.operation.unwrap();
    assert!(op.next_attempt(&time(11)).is_err());
    assert!(op.next_attempt(&time(12)).is_ok());
}
#[test]
fn dispatch_and_attempt_times_cannot_precede_retained_preparation() {
    let mut prep = vector("prepared");
    prep.time = time(5);
    let s = state(0)
        .apply_retained(&prep, &root(), Some(&documents()))
        .unwrap();
    let op = s.operation.as_ref().unwrap();
    let intent = DispatchIntentV2 {
        binding: op.prepared.authorization.binding.clone(),
        authorization_sha256: op.prepared.authorization.digest().unwrap(),
        preparation: op.preparation.clone(),
        prepared_at: op.preparation_time.clone(),
        deadline: op.deadline.clone(),
    };
    assert!(apply(&s, EventV2::LaunchDispatchIntent(intent.clone()), time(4)).is_err());
    let s = apply(&s, EventV2::LaunchDispatchIntent(intent), time(6)).unwrap();
    assert!(
        s.operation
            .as_ref()
            .unwrap()
            .next_attempt(&time(5))
            .is_err()
    );
    assert!(s.operation.as_ref().unwrap().next_attempt(&time(6)).is_ok());
}
#[test]
fn oversized_preparation_and_unknown_event_fields_fail_closed() {
    let e = vector("prepared");
    let mut p = documents();
    p.approval.launch.user_data = "x".repeat(65536);
    assert!(canonical::encode(&p.approval).is_err());
    assert!(state(0).apply_retained(&e, &root(), Some(&p)).is_err());
    assert!(state(0).apply(&e, &root()).is_err());
    let bytes = canonical::encode(&vector("attempt-intent")).unwrap();
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace("\"number\":1", "\"number\":1,\"can_retry\":true");
    assert!(canonical::decode::<EnvelopeV2>(text.as_bytes()).is_err());
}

#[test]
fn a_fresh_other_operation_cannot_replace_any_unresolved_state_even_after_expiry() {
    use borrowser_host_lifecycle::launch::{ClientToken, LaunchSpecV2, RunInstancesRequestV2};
    for stage in 1..=4 {
        let s = state(stage);
        let mut p = documents();
        p.specification = LaunchSpecV2::new(
            &p.deployment,
            &p.approval,
            &p.trust,
            "new-operation".parse().unwrap(),
        )
        .unwrap();
        let token =
            ClientToken::derive(&p.specification, &p.deployment, &p.approval, &p.trust).unwrap();
        p.request = RunInstancesRequestV2::new(
            &p.deployment,
            &p.approval,
            &p.trust,
            p.specification.clone(),
            token,
        )
        .unwrap();
        p.authorization.binding = p.binding().unwrap();
        p.authorization.expected_head = s.head.clone().unwrap();

        p.validate(&root(), s.head.as_ref().unwrap(), &time(121))
            .unwrap();
        assert_eq!(
            apply(
                &s,
                EventV2::LaunchPrepared(p.preparation().unwrap()),
                time(121)
            )
            .unwrap_err(),
            borrowser_host_lifecycle::Error("unresolved acquisition exists")
        );
    }
}

#[test]
fn human_artifact_has_no_operational_clock_or_deadline_authority() {
    let original = documents();
    for field in ["deadline", "authorized_at", "retry_not_before"] {
        let mut value = serde_json::to_value(&original.authorization).unwrap();
        value[field] = serde_json::json!({"boottime_ns": u64::MAX});
        assert!(
            canonical::decode::<LaunchAuthorizationV2>(&canonical::encode(&value).unwrap())
                .is_err()
        );
    }
    for audit_seconds in [1800000000, 1800000001] {
        let mut p = original.clone();
        p.authorization.authorized_at_unix_seconds = audit_seconds;
        let mut e = vector("prepared");
        e.time = time(10);
        e.time.realtime_ns += 1_000_000_000;
        e.event = EventV2::LaunchPrepared(p.preparation().unwrap());
        let s = state(0).apply_retained(&e, &root(), Some(&p)).unwrap();
        let op = s.operation.unwrap();
        assert_eq!(
            op.deadline.boottime_ns,
            e.time.boottime_ns + 120_000_000_000
        );
        assert_eq!(op.preparation_time, e.time);
    }
    let s = state(1);
    for delta in [1, 120_000_000_000] {
        let mut e = vector("dispatch-intent");
        let EventV2::LaunchDispatchIntent(ref mut d) = e.event else {
            panic!()
        };
        d.deadline.boottime_ns += delta;
        assert!(s.apply(&e, &root()).is_err());
    }
    let mut e = vector("dispatch-intent");
    let EventV2::LaunchDispatchIntent(ref mut d) = e.event else {
        panic!()
    };
    d.prepared_at.boottime_ns += 1;
    assert!(s.apply(&e, &root()).is_err());
}
#[test]
fn retry_times_are_fixed_from_exact_outcome_receipts() {
    let mut s = state(4);
    for (due, before) in [(2, 1), (10, 9)] {
        let mut early = time(before);
        early.boottime_ns += 999_999_999;
        assert!(s.operation.as_ref().unwrap().next_attempt(&early).is_err());
        let intent = s
            .operation
            .as_ref()
            .unwrap()
            .next_attempt(&time(due))
            .unwrap();
        let mut value = serde_json::to_value(&intent).unwrap();
        for timestamp in [0, u64::MAX] {
            value["retry_not_before"] = timestamp.into();
            assert!(
                canonical::decode::<AttemptIntentV2>(&canonical::encode(&value).unwrap()).is_err()
            );
        }
        s = apply(&s, EventV2::LaunchAttemptIntent(intent), time(due)).unwrap();
        let attempt = s
            .operation
            .as_ref()
            .unwrap()
            .attempts
            .last()
            .unwrap()
            .identity
            .clone();
        s = apply(
            &s,
            EventV2::LaunchAttemptOutcome(AttemptOutcomeV2 {
                attempt,
                outcome: LaunchAttemptOutcome::DefinitelyNotTransmitted,
            }),
            time(due),
        )
        .unwrap();
    }
}

/// Maximal lexical identities, maximally escaped bounded text, full tag/SG
/// collections and largest supported trust byte representation. These are valid
/// Pass-2 contract values, not AWS existence or cryptographic trust assertions.
fn maximal_documents() -> PreparedLaunchV2 {
    use borrowser_host_lifecycle::{collector_config::CollectorConfigV2, launch::*};
    let mut p = documents();
    let text: borrowser_host_lifecycle::identity::ReviewText = "\"".repeat(256).parse().unwrap();
    let region = "r".repeat(32);
    p.deployment.identity.authority_id = "a".repeat(128).parse().unwrap();
    p.deployment.identity.region = region.parse().unwrap();
    p.trust.region = region.parse().unwrap();
    p.trust.certificate_der_hex = "aa".repeat(16384);
    p.trust.certificate_sha256 = canonical::sha256(&vec![0xaa; 16384]).parse().unwrap();
    p.trust.source_reference = text.clone();
    p.trust.replaces = Some("f".repeat(64).parse().unwrap());
    p.trust.review.reviewer = text.clone();
    p.trust.review.reference = text.clone();
    let trust_digest = p.trust.digest().unwrap();
    let support = p.deployment.reviewed_support.as_mut().unwrap();
    support.infrastructure_reference = text.clone();
    support.review.reviewer = text.clone();
    support.review.reference = text.clone();
    support.availability_zone = "z".repeat(32).parse().unwrap();
    support.availability_zone_id = "i".repeat(32).parse().unwrap();
    support.vpc_id = format!("vpc-{}", "a".repeat(64)).parse().unwrap();
    support.subnet_id = format!("subnet-{}", "a".repeat(64)).parse().unwrap();
    support.security_group_ids = (0..5)
        .map(|i| format!("sg-{}{i}", "a".repeat(63)).parse().unwrap())
        .collect();
    let profile = "arn:aws:iam::111111111111:instance-profile/";
    support.instance_profile_arn = format!("{profile}{}", "p".repeat(512 - profile.len()))
        .parse()
        .unwrap();
    let role = "arn:aws:iam::111111111111:role/";
    support.role_arn = format!("{role}{}", "r".repeat(512 - role.len()))
        .parse()
        .unwrap();
    support.role_unique_id = "r".repeat(128).parse().unwrap();
    support.instance_profile_id = "p".repeat(128).parse().unwrap();
    support.kms_key_arn =
        format!("arn:aws:kms:{region}:111111111111:key/00000000-0000-0000-0000-000000000001")
            .parse()
            .unwrap();
    support.evidence_bucket = "b".repeat(63).parse().unwrap();
    support.evidence_bucket_region = region.parse().unwrap();
    support.s3_gateway_endpoint_region = region.parse().unwrap();
    support.s3_gateway_endpoint_id = format!("vpce-{}", "a".repeat(64)).parse().unwrap();
    support.subnet_route_table_id = format!("rtb-{}", "a".repeat(64)).parse().unwrap();
    support.identity_trust_sha256 = trust_digest;
    let launch = &mut p.approval.launch;
    launch.region = region.parse().unwrap();
    launch.availability_zone = support.availability_zone.clone();
    launch.availability_zone_id = support.availability_zone_id.clone();
    launch.vpc_id = support.vpc_id.clone();
    launch.subnet_id = support.subnet_id.clone();
    launch.security_group_ids = support.security_group_ids.clone();
    launch.instance_profile_arn = support.instance_profile_arn.clone();
    launch.instance_profile_id = support.instance_profile_id.clone();
    launch.role_arn = support.role_arn.clone();
    launch.role_unique_id = support.role_unique_id.clone();
    let RootVolumeV2::Gp3 {
        ref mut kms_key_arn,
        ..
    } = launch.root_volume;
    *kms_key_arn = support.kms_key_arn.clone();
    launch.instance_type = "i".repeat(128).parse().unwrap();
    launch.ami.image_id = format!("ami-{}", "a".repeat(64)).parse().unwrap();
    launch.ami.root_device_name = format!("/dev/{}", "a".repeat(32));
    launch.ami.provenance_reference = text.clone();
    launch.ami.review.reviewer = text.clone();
    launch.ami.review.reference = text.clone();
    for tags in [
        &mut launch.tags.instance,
        &mut launch.tags.volume,
        &mut launch.tags.network_interface,
    ] {
        *tags = (0..16)
            .map(|i| TagV2 {
                key: format!("{}{:03}", "\"".repeat(125), i),
                value: "\"".repeat(256),
            })
            .collect();
    }
    let mut config: CollectorConfigV2 = canonical::decode(launch.user_data.as_bytes()).unwrap();
    config.region = region.parse().unwrap();
    config.role_unique_id = support.role_unique_id.clone();
    config.evidence_bucket = support.evidence_bucket.clone();
    launch.user_data = String::from_utf8(config.user_data_bytes().unwrap()).unwrap();
    p.approval.identity_trust_sha256 = support.identity_trust_sha256.clone();
    p.approval.deployment_sha256 = p.deployment.digest().unwrap();
    p.approval.review.reviewer = text.clone();
    p.approval.review.reference = text.clone();
    p.approval.cost_ceilings.review.reviewer = text.clone();
    p.approval.cost_ceilings.review.reference = text.clone();
    p.specification = LaunchSpecV2::new(
        &p.deployment,
        &p.approval,
        &p.trust,
        "o".repeat(128).parse().unwrap(),
    )
    .unwrap();
    let token =
        ClientToken::derive(&p.specification, &p.deployment, &p.approval, &p.trust).unwrap();
    p.request = RunInstancesRequestV2::new(
        &p.deployment,
        &p.approval,
        &p.trust,
        p.specification.clone(),
        token,
    )
    .unwrap();
    p.authorization.binding = p.binding().unwrap();
    p.authorization.reviewer = text.clone();
    p.authorization.reference = text.clone();
    p.authorization.rationale = text;
    p
}
#[test]
fn worst_case_valid_documents_and_all_production_event_shapes_have_structural_headroom() {
    let mut p = maximal_documents();
    let root = p.deployment.marker().unwrap();
    let mut g = genesis();
    g.authority_id = root.identity.authority_id.clone();
    g.region = root.identity.region.clone();
    g.root_sha256 = root.digest().unwrap();
    g.tool.package_version = "\"".repeat(64);
    g.time.time_namespace = "\"".repeat(128);
    let mut s = AuthorityStateV2::default().apply(&g, &root).unwrap();
    p.authorization.expected_head = s.head.clone().unwrap();
    let mut t = time(0);
    t.time_namespace = g.time.time_namespace.clone();
    t.boottime_ns = u64::MAX - 120_000_000_000;
    p.validate(&root, s.head.as_ref().unwrap(), &t).unwrap();
    // Individual artifacts remain valid and bounded, while embedding the aggregate
    // cannot fit. Preparation never serializes this aggregate.
    assert!(canonical::encode(&p).is_err());
    let refs = p.references().unwrap();
    println!(
        "maximum-value artifact sizes: deployment={}, approval={}, trust={}, spec={}, request={}, authorization={}",
        refs.deployment.bytes,
        refs.approval.bytes,
        refs.trust.bytes,
        refs.specification.bytes,
        refs.request.bytes,
        refs.authorization.bytes
    );
    let mut e = g.clone();
    e.sequence = s.sequence;
    e.previous_sha256 = s.head.clone();
    e.time = t.clone();
    e.event = EventV2::LaunchPrepared(p.preparation().unwrap());
    fn bound(e: &EnvelopeV2) {
        let bytes = canonical::encode(e).unwrap();
        assert_eq!(canonical::decode::<EnvelopeV2>(&bytes).unwrap(), *e);
        assert!(bytes.len() < 8192, "{}", bytes.len());
        // Conservative schema-level numeric upper bound, even for widths which
        // cannot occur simultaneously in a reachable journal (e.g. sequence).
        // Keep schema and attempt number fixed; every other integer gets 20 digits.
        fn widen(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::Object(fields) => {
                    for (key, value) in fields {
                        if key != "schema_version" && key != "number" {
                            widen(value);
                        }
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        widen(value);
                    }
                }
                serde_json::Value::Number(_) => *value = u64::MAX.into(),
                _ => (),
            }
        }
        let mut upper = serde_json::to_value(e).unwrap();
        widen(&mut upper);
        let upper = canonical::encode(&upper).unwrap().len();
        assert!(upper < 8192);
        println!(
            "event {:?}: valid={} numeric-upper={upper}",
            std::mem::discriminant(&e.event),
            bytes.len()
        );
    }
    bound(&e);
    s = s.apply_retained(&e, &root, Some(&p)).unwrap();
    let op = s.operation.as_ref().unwrap();
    e.event = EventV2::LaunchDispatchIntent(DispatchIntentV2 {
        binding: p.authorization.binding.clone(),
        authorization_sha256: p.authorization.digest().unwrap(),
        preparation: op.preparation.clone(),
        prepared_at: op.preparation_time.clone(),
        deadline: op.deadline.clone(),
    });
    e.sequence = s.sequence;
    e.previous_sha256 = s.head.clone();
    bound(&e);
    s = s.apply(&e, &root).unwrap();
    e.event = EventV2::LaunchAttemptIntent(s.operation.as_ref().unwrap().next_attempt(&t).unwrap());
    e.sequence = s.sequence;
    e.previous_sha256 = s.head.clone();
    bound(&e);
    s = s.apply(&e, &root).unwrap();
    let attempt = s.operation.as_ref().unwrap().attempts[0].identity.clone();
    for outcome in [
        LaunchAttemptOutcome::DefinitelyNotTransmitted,
        LaunchAttemptOutcome::TransmissionUncertain,
        LaunchAttemptOutcome::ParameterConflict,
        LaunchAttemptOutcome::AccessBlocked,
        LaunchAttemptOutcome::ThrottledHeld,
        LaunchAttemptOutcome::ResponseUnresolved,
    ] {
        e.event = EventV2::LaunchAttemptOutcome(AttemptOutcomeV2 {
            attempt: attempt.clone(),
            outcome,
        });
        e.sequence = s.sequence;
        e.previous_sha256 = s.head.clone();
        bound(&e);
        s.apply(&e, &root).unwrap();
    }
}

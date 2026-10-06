use super::*;

#[path = "operation_discovery_parent_tests.rs"]
mod positional_parent_tests;

const FOREIGN_TAGS: &str = "<tagSet><item><key>borrowser:authority-id</key><value>synthetic-authority</value></item><item><key>borrowser:operation-id</key><value>foreign-operation</value></item></tagSet>";
const PROFILE: &str = "<iamInstanceProfile><arn>arn:aws:iam::111111111111:instance-profile/foreign</arn><id>AIPABCDEFGHIJKLMNOPQ</id></iamInstanceProfile>";

#[derive(Debug)]
struct ForeignComponent(DiscoveryScript);
impl HttpConnector for ForeignComponent {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let text = std::str::from_utf8(request.body().bytes().unwrap_or_default()).unwrap();
        let fields: BTreeMap<_, _> = text.split('&').filter_map(|p| p.split_once('=')).collect();
        let get = |key| fields.get(key).copied().unwrap_or("");
        let action = get("Action");
        let authority = get("Filter.1.Name").contains("authority-id");
        let attached = get("Filter.1.Value.1") == "i-aaaaaaaa";
        let body = match action {
            "DescribeInstances" if authority || !get("InstanceId.1").is_empty() => Some(xml(
                ReadOperationV1::DescribeInstances,
                &format!(
                    "<item><ownerId>111111111111</ownerId><instancesSet><item><instanceId>i-aaaaaaaa</instanceId><clientToken>foreign-token</clientToken>{FOREIGN_TAGS}{PROFILE}<networkInterfaceSet><item><networkInterfaceId>eni-aaaaaaaa</networkInterfaceId><ownerId>111111111111</ownerId><attachment><attachmentId>eni-attach-aaaaaaaa</attachmentId></attachment></item></networkInterfaceSet><blockDeviceMapping><item><deviceName>/dev/sda1</deviceName><ebs><volumeId>vol-aaaaaaaa</volumeId><volumeOwnerId>111111111111</volumeOwnerId></ebs></item></blockDeviceMapping><secondaryInterfaceSet/></item></instancesSet></item>"
                ),
            )),
            "DescribeNetworkInterfaces"
                if authority || attached || !get("NetworkInterfaceId.1").is_empty() =>
            {
                Some(xml(
                    ReadOperationV1::DescribeNetworkInterfaces,
                    &format!(
                        "<item><networkInterfaceId>eni-aaaaaaaa</networkInterfaceId><ownerId>111111111111</ownerId>{FOREIGN_TAGS}<attachment><attachmentId>eni-attach-aaaaaaaa</attachmentId><instanceId>i-aaaaaaaa</instanceId><instanceOwnerId>111111111111</instanceOwnerId></attachment></item>"
                    ),
                ))
            }
            "DescribeVolumes" if authority || attached || !get("VolumeId.1").is_empty() => {
                Some(xml(
                    ReadOperationV1::DescribeVolumes,
                    &format!(
                        "<item><volumeId>vol-aaaaaaaa</volumeId>{FOREIGN_TAGS}<attachmentSet><item><volumeId>vol-aaaaaaaa</volumeId><instanceId>i-aaaaaaaa</instanceId><device>/dev/sda1</device></item></attachmentSet></item>"
                    ),
                ))
            }
            "DescribeIamInstanceProfileAssociations"
                if attached || !get("AssociationId.1").is_empty() =>
            {
                Some(format!(
                    "<DescribeIamInstanceProfileAssociationsResponse><iamInstanceProfileAssociationSet><item><associationId>iip-assoc-aaaaaaaa</associationId><instanceId>i-aaaaaaaa</instanceId>{PROFILE}</item></iamInstanceProfileAssociationSet></DescribeIamInstanceProfileAssociationsResponse>"
                ))
            }
            _ => None,
        };
        if let Some(body) = body {
            self.0.requests.lock().unwrap().push(
                fields
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect(),
            );
            HttpConnectorFuture::new(async move { Ok(response(200, &body).try_into().unwrap()) })
        } else {
            self.0.call(request)
        }
    }
}

#[tokio::test]
async fn foreign_component_requires_source_specific_relationship_agreement() {
    use crate::provider::{
        allocation_value_v5::MemberV5 as M,
        ec2_allocation_observation_v5::ObservationDataV5 as D,
        management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2 as U},
    };
    use relationships::{AllocationIdentity as I, Attribution as A};
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let connector = SharedHttpConnector::new(ForeignComponent(DiscoveryScript {
        requests: Arc::new(Mutex::new(Vec::new())),
        populate: false,
        foreign: true,
    }));
    let http = http_client_fn(move |_, _| connector.clone());
    let mut session = ObservationSession::bounded(
        &p.deployment,
        MANIFEST,
        secret(),
        BoundedHttp::new(http, ObservationRound::test()),
    )
    .unwrap();
    let baseline = crate::aws::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    assert!(baseline.report.complete, "{:?}", baseline.report);
    let component = [
        I::Instance("i-aaaaaaaa".parse().unwrap()),
        I::NetworkInterface("eni-aaaaaaaa".parse().unwrap()),
        I::Volume("vol-aaaaaaaa".parse().unwrap()),
        I::ProfileAssociation("iip-assoc-aaaaaaaa".parse().unwrap()),
    ];
    for id in &component {
        assert_eq!(
            baseline.report.attribution(id),
            Some(A::AffirmativelyUnrelated)
        );
    }
    let opaque_operator = || {
        V::Present(crate::provider::ec2_allocation_observation_v5::OperatorV5 {
            managed: V::Present(false),
            principal: M::Present("opaque-principal".to_owned().try_into().unwrap()),
            hidden_by_default: V::Unavailable(U::NotReturned),
        })
    };
    for case in 0..21 {
        let mut changed = baseline.evidence.evidence().clone();
        let mut mutated = false;
        for entry in &mut changed.observations.records {
            let ObservationEntryV5::V5(record) = entry else {
                continue;
            };
            if mutated {
                break;
            }
            match (&mut record.data, case) {
                (D::InstanceEniAttachment(v), 0) => {
                    v.attachment = V::Unavailable(U::NotReturned);
                }
                (D::InstanceEniAttachment(v), 1) => {
                    let V::Present(a) = &mut v.attachment else {
                        panic!()
                    };
                    a.id = M::Present("eni-attach-bbbbbbbb".parse().unwrap());
                }
                (D::StandaloneEniAttachment(v), 2) => {
                    let V::Present(a) = &mut v.attachment else {
                        panic!()
                    };
                    a.instance_owner = M::NotReturned;
                }
                (D::StandaloneEniAttachment(v), 3) => {
                    let V::Present(a) = &mut v.attachment else {
                        panic!()
                    };
                    a.instance_owner = M::Present("222222222222".parse().unwrap());
                }
                (D::NetworkInterface(v), 4)
                    if matches!(
                        record.source.path,
                        SourcePathV5::InstanceNetworkInterface { .. }
                    ) =>
                {
                    v.owner = M::Present("222222222222".parse().unwrap());
                }
                (D::NetworkInterface(v), 5)
                    if matches!(
                        record.source.path,
                        SourcePathV5::InstanceNetworkInterface { .. }
                    ) =>
                {
                    v.owner = M::NotReturned;
                }
                (D::InstanceEbsMapping(v), 6) => {
                    let V::Present(a) = &mut v.ebs else { panic!() };
                    a.volume_owner = M::NotReturned;
                }
                (D::InstanceEbsMapping(v), 7) => {
                    let V::Present(a) = &mut v.ebs else { panic!() };
                    a.volume_owner = M::Present("222222222222".parse().unwrap());
                }
                (D::VolumeAttachment(v), 8) => {
                    v.device = M::Present("/dev/sdb".to_owned().try_into().unwrap());
                }
                (D::ProfileAssociation(v), 9) => {
                    let V::Present(a) = &mut v.profile else {
                        panic!()
                    };
                    a.id = M::Present("AIPZZZZZZZZZZZZZZZZZ".parse().unwrap());
                }
                (D::ProfileAssociation(v), 10) => {
                    v.profile = V::Unavailable(U::NotReturned);
                }
                (D::VolumeAttachment(v), 11) => {
                    v.associated_resource =
                        M::Present("opaque-managed-reference".to_owned().try_into().unwrap());
                }
                (D::StandaloneEniAttachment(v), 12) => {
                    v.attachment = V::Unavailable(U::NotReturned);
                }
                (D::Reservation(v), 13) => {
                    v.owner = M::Present("222222222222".parse().unwrap());
                }
                (D::NetworkInterface(v), 14) => {
                    v.requester.identity =
                        M::Present("opaque-requester".to_owned().try_into().unwrap());
                }
                (D::NetworkInterface(v), 15) => {
                    v.operator = opaque_operator();
                }
                (D::Volume(v), 16) => {
                    v.operator = opaque_operator();
                }
                (D::InstanceEbsMapping(v), 17) => {
                    let V::Present(a) = &mut v.ebs else { panic!() };
                    a.associated_resource =
                        M::Present("opaque-associated-resource".to_owned().try_into().unwrap());
                }
                (D::VolumeAttachment(v), 18) => {
                    v.instance_owning_service =
                        M::Present("opaque-service".to_owned().try_into().unwrap());
                }
                (D::Instance(v), 19) => {
                    v.operator = opaque_operator();
                }
                (D::InstanceEbsMapping(v), 20) => {
                    let V::Present(a) = &mut v.ebs else { panic!() };
                    a.operator = opaque_operator();
                }
                _ => continue,
            }
            mutated = true;
        }
        assert!(mutated, "case {case}");
        let before = &baseline.evidence.evidence().observations;
        assert_eq!(before.coverage, changed.observations.coverage);
        let bytes = |records: &Vec<ObservationEntryV5>| {
            records
                .iter()
                .map(|r| r.canonical_bytes().unwrap().len())
                .sum::<usize>()
        };
        let mut execution = baseline.execution.clone();
        execution.accounting.normalized_bytes +=
            bytes(&changed.observations.records).saturating_sub(bytes(&before.records)) as u64;
        let valid = ingest(&inputs, changed).unwrap();
        let report = derive_offline(&inputs, &valid, &execution).unwrap();
        if matches!(case, 1 | 3 | 4 | 7 | 8 | 9 | 13) {
            assert!(
                report
                    .relationships
                    .resources
                    .values()
                    .any(|r| r.agreement.contradictory && r.attribution == A::ContradictoryLinkage)
            );
        }
        if case == 11 || case >= 14 {
            assert!(report.relationships.gaps.iter().any(|g| g.kind
                == relationships::RelationshipGapKind::OpaqueManagedReference
                && g.observation.is_some()));
            assert!(!report.complete);
        }
        for id in &component {
            assert_ne!(
                report.attribution(id),
                Some(A::AffirmativelyUnrelated),
                "case {case}: {id:?}"
            );
        }
        assert_eq!(
            valid.evidence().observations.records.len(),
            before.records.len()
        );
    }
}

#[tokio::test]
async fn selected_dns_value_must_agree_with_live_adapter_coverage() {
    use crate::aws::ec2_infrastructure_reads::InfrastructureRead;
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for (attribute, selected, sibling) in [
        (
            VpcAttribute::EnableDnsSupport,
            "enableDnsSupport",
            "enableDnsHostnames",
        ),
        (
            VpcAttribute::EnableDnsHostnames,
            "enableDnsHostnames",
            "enableDnsSupport",
        ),
    ] {
        for (fields, valid) in [
            (String::new(), false),
            (format!("<{selected}/>"), false),
            (format!("<{sibling}><value>true</value></{sibling}>"), false),
            (
                format!("<{selected}><value>false</value></{selected}>"),
                true,
            ),
            (
                format!("<{selected}><value>true</value></{selected}>"),
                true,
            ),
        ] {
            let body = format!(
                "<DescribeVpcAttributeResponse><vpcId>vpc-00000000000000001</vpcId>{fields}</DescribeVpcAttributeResponse>"
            );
            let (mut session, _, round) = reader(vec![response(200, &body)]);
            let result = session
                .infrastructure(InfrastructureRead::VpcAttribute(attribute), true)
                .await
                .unwrap();
            assert_eq!(
                result.coverage.status,
                if valid {
                    CoverageStatus::Complete
                } else {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                }
            );
            assert_eq!(result.coverage.records, 1);
            let truthful = bundle(c.identity().unwrap(), result);
            let evidence = ingest(&inputs, truthful.clone()).unwrap();
            let report = derive_offline(&inputs, &evidence, &facts(&round)).unwrap();
            assert!(report.representation.values().all(|a| a.complete()));
            let mut inconsistent = truthful;
            inconsistent.observations.coverage[0].status = CoverageStatus::Complete;
            let evidence = ingest(&inputs, inconsistent).unwrap();
            assert_eq!(
                derive_offline(&inputs, &evidence, &facts(&round)).is_ok(),
                valid,
                "{attribute:?}: {fields}"
            );
        }
    }
}

#[tokio::test]
async fn all_seed_stages_precede_relationship_requests() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let (mut session, requests) = scripted_session(&p, ObservationRound::test(), true);
    let result = crate::aws::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    assert!(result.report.complete);
    let requests = requests.lock().unwrap();
    let last_tag = requests
        .iter()
        .rposition(|r| {
            r.get("Filter.1.Name")
                .is_some_and(|s| s.contains("borrowser"))
        })
        .unwrap();
    let first_followup = requests
        .iter()
        .position(|r| {
            r.contains_key("InstanceId.1")
                || r.contains_key("NetworkInterfaceId.1")
                || r.contains_key("VolumeId.1")
                || r.contains_key("InstanceId")
        })
        .unwrap();
    assert!(
        last_tag < first_followup,
        "an allocation follow-up preceded an independent seed"
    );
    assert_eq!(
        requests[0].get("Filter.1.Name").map(String::as_str),
        Some("client-token")
    );
    // This association is first returned by a depth-one attachment query, so
    // its exact read must follow every depth-one query (despite its lexical key).
    assert!(requests.last().unwrap().contains_key("AssociationId.1"));
    let association = result.report.required.reads.values().find(|r|matches!(&r.query,
        crate::provider::reviewed_subnet_routes_v1::DiscoveryQuery::Existing(q) if q.operation==ReadOperationV1::DescribeIamInstanceProfileAssociations && matches!(q.scope,QueryScopeV1::Exact{..}))).unwrap();
    assert_eq!(
        association.stage,
        scheduling::WorkStage::Relationships { depth: 2 }
    );
    assert_eq!(
        result.report,
        derive_offline(&inputs, &result.evidence, &result.execution).unwrap()
    );
}

#[tokio::test]
async fn request_exhaustion_keeps_the_seed_order_without_resetting_the_round() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let round = ObservationRound::test();
    for _ in 0..121 {
        round.test_request(0).unwrap();
    }
    let (mut session, requests) = scripted_session(&p, round.clone(), true);
    let result = crate::aws::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 7);
    assert_eq!(round.snapshot().requests, 128);
    assert_eq!(
        round.snapshot().failure,
        Some(crate::provider::limits::LimitKind::Requests)
    );
    let names: Vec<_> = requests
        .iter()
        .map(|r| r.get("Filter.1.Name").unwrap().as_str())
        .collect();
    assert_eq!(names[0], "client-token");
    assert!(names[1..4].iter().all(|s| s.contains("authority-id")));
    assert!(names[4..7].iter().all(|s| s.contains("operation-id")));
    assert!(!result.report.complete);
    assert_eq!(
        result.report,
        derive_offline(&inputs, &result.evidence, &result.execution).unwrap()
    );
}

#[test]
fn combined_report_budget_exhausts_before_any_former_individual_ceiling() {
    use crate::provider::context_v3::*;
    use relationships::AllocationIdentity as I;
    let (p, c) = retained();
    let state: crate::identity::ProviderStateDigest = "a".repeat(64).parse().unwrap();
    let mut fields = c.fields().clone();
    fields.prior_provider = Some(PriorProviderIdentityV3 {
        bound_instance: None,
        sticky_conflict: false,
        state: state.clone(),
        evidence: Vec::new(),
    });
    let c = ReconciliationContextV3::from_fields(fields).unwrap();
    let mut prior = PriorDiscoveryFacts {
        state,
        resources: (0..12)
            .map(|i| I::Instance(format!("i-{i:017x}").parse().unwrap()))
            .collect(),
        relationships: Vec::new(),
        fully_supplied: true,
    };
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, Some(&prior)).unwrap();
    let raw = DiscoveryEvidence {
        observations: ProviderObservationV5 {
            context: c.identity().unwrap(),
            records: Vec::new(),
            coverage: Vec::new(),
        },
        reviewed_subnet_routes: None,
    };
    let valid = ingest(&inputs, raw.clone()).unwrap();
    let execution = facts(&ObservationRound::test());
    let baseline = derive_offline(&inputs, &valid, &execution).unwrap();
    assert!(!baseline.metadata_exhausted);
    // These are distinct retained source occurrences, even when endpoints
    // repeat. Query deduplication must not delete their report cost/evidence.
    prior.relationships = (0..100)
        .map(|n| {
            (
                prior.resources[n % 12].clone(),
                prior.resources[(n + 1) % 12].clone(),
            )
        })
        .collect();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, Some(&prior)).unwrap();
    let report = derive_offline(&inputs, &valid, &execution).unwrap();
    // Independently specified: 27 retained seeds and eight reads for each of
    // twelve prior instances fit 123 query slots. No query occurrence triggers.
    // The graph and keys individually fit the old 32/64 KiB caps, but their
    // combined keys + owned queries + statuses + graph must fit one 64 KiB cap.
    assert_eq!(report.relationships.resources.len(), 12);
    assert!(report.required.reads.len() <= 27 + 8 * 12);
    assert!(report.required.reads.len() < 128);
    assert!(report.required.reads.keys().map(Vec::len).sum::<usize>() < 64 << 10);
    assert!(
        report
            .required
            .reads
            .values()
            .all(|r| r.observations.is_empty())
    );
    assert_eq!(report.relationships.relationships.len(), 100);
    assert!(
        prior.resources.len() * (2 * (16 + 19) + 64)
            + prior.relationships.len() * (2 * (16 + 19) + 24)
            < 32 << 10
    );
    assert!(report.metadata_exhausted, "{} bytes", report.metadata_bytes);
    assert!(report.metadata_bytes <= 65_536);
    assert!(!report.complete);
    prior.resources.reverse();
    prior.relationships.reverse();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, Some(&prior)).unwrap();
    let valid = ingest(&inputs, raw).unwrap();
    assert_eq!(report, derive_offline(&inputs, &valid, &execution).unwrap());
}

#[tokio::test]
async fn frozen_v5_already_rejects_complete_coverage_with_missing_selected_attributes() {
    for attribute in [
        InstanceAttribute::UserData,
        InstanceAttribute::InstanceInitiatedShutdownBehavior,
        InstanceAttribute::DisableApiTermination,
        InstanceAttribute::DisableApiStop,
    ] {
        let (p, c) = retained();
        let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
        let (mut session, _, round) = reader(vec![response(
            200,
            "<DescribeInstanceAttributeResponse><instanceId>i-aaaaaaaa</instanceId></DescribeInstanceAttributeResponse>",
        )]);
        let read = AllocationRead::try_from((
            ReadOperationV1::DescribeInstanceAttribute,
            QueryScopeV1::InstanceAttribute {
                instance: "i-aaaaaaaa".parse().unwrap(),
                attribute,
            },
        ))
        .unwrap();
        let result = session.allocation(read, true).await.unwrap();
        assert_eq!(
            result.coverage.status,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed)
        );
        let mut evidence = bundle(c.identity().unwrap(), result);
        let valid = ingest(&inputs, evidence.clone()).unwrap();
        assert!(
            !derive_offline(&inputs, &valid, &facts(&round))
                .unwrap()
                .complete
        );
        evidence.observations.coverage[0].status = CoverageStatus::Complete;
        assert!(evidence.observations.validate().is_err());
        assert!(ingest(&inputs, evidence).is_err());
    }
}

use super::{
    ec2_allocation_reads::AllocationRead,
    ec2_infrastructure_reads_tests::reader,
    observation_session::ObservationSession,
    response_limits::{BoundedHttp, ObservationRound},
    tests::secret,
};
use crate::provider::discovery::test_support::{MANIFEST, retained};
use crate::provider::{coverage::*, discovery::*, observation_v5::*, source_occurrence_v5::*};
use aws_smithy_runtime_api::client::http::{
    HttpConnector, HttpConnectorFuture, SharedHttpConnector, http_client_fn,
};
use aws_smithy_runtime_api::client::orchestrator::HttpRequest;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

fn query(op: ReadOperationV1) -> AllocationRead {
    AllocationRead::try_from((
        op,
        QueryScopeV1::OperationTag {
            operation: "synthetic-operation-1".parse().unwrap(),
        },
    ))
    .unwrap()
}
fn eni(id: &str, instance: &str) -> String {
    format!(
        "<item><networkInterfaceId>{id}</networkInterfaceId><attachment><instanceId>{instance}</instanceId></attachment></item>"
    )
}
fn volume(id: &str, instance: &str) -> String {
    format!(
        "<item><volumeId>{id}</volumeId><attachmentSet><item><volumeId>{id}</volumeId><instanceId>{instance}</instanceId></item></attachmentSet></item>"
    )
}
fn reservation(id: &str) -> String {
    format!(
        "<item><instancesSet><item><instanceId>{id}</instanceId><networkInterfaceSet/><blockDeviceMapping/><secondaryInterfaceSet/></item></instancesSet></item>"
    )
}
fn xml(op: ReadOperationV1, roots: &str) -> String {
    let set = match op {
        ReadOperationV1::DescribeNetworkInterfaces => "networkInterfaceSet",
        ReadOperationV1::DescribeVolumes => "volumeSet",
        _ => "reservationSet",
    };
    format!("<{op:?}Response><{set}>{roots}</{set}></{op:?}Response>")
}
fn bundle(
    context: crate::identity::ReconciliationContextDigest,
    r: super::query_execution::QueryResult,
) -> DiscoveryEvidence {
    DiscoveryEvidence {
        observations: ProviderObservationV5 {
            context,
            records: r.records,
            coverage: vec![r.coverage],
        },
        reviewed_subnet_routes: None,
    }
}
fn root(entry: &ObservationEntryV5) -> u64 {
    let ObservationEntryV5::V5(v) = entry else {
        panic!()
    };
    match v.source.path {
        SourcePathV5::NetworkInterface { interface } => interface.into(),
        SourcePathV5::Volume { volume } | SourcePathV5::VolumeAttachment { volume, .. } => {
            volume.into()
        }
        SourcePathV5::Reservation { reservation } | SourcePathV5::Instance { reservation, .. } => {
            reservation.into()
        }
        _ => panic!("unexpected test projection"),
    }
}
fn facts(round: &ObservationRound) -> DiscoveryExecutionFacts {
    DiscoveryExecutionFacts {
        accounting: round.snapshot(),
        rejected_before_admission: Vec::new(),
        stop: CoordinatorStop::Quiescent,
        final_check: FinalRoundCheck::Passed,
    }
}

fn response(status: u16, body: &str) -> http::Response<aws_smithy_types::body::SdkBody> {
    super::tests::response(status, body, None)
}

#[derive(Debug)]
struct DiscoveryScript {
    requests: RecordedRequests,
    populate: bool,
    foreign: bool,
}
impl HttpConnector for DiscoveryScript {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let mut params = BTreeMap::new();
        let body = std::str::from_utf8(request.body().bytes().unwrap_or_default()).unwrap();
        for pair in body.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.insert(k.to_owned(), v.to_owned());
            }
        }
        let target = request.headers().get("x-amz-target").map(str::to_owned);
        let action = params
            .get("Action")
            .map(String::as_str)
            .unwrap_or(if target.is_some() {
                "DescribeKey"
            } else {
                "HeadBucket"
            });
        let get = |k: &str| params.get(k).map(String::as_str).unwrap_or("");
        let mut status = 200;
        let body=match action {
            "GetCallerIdentity"=>"<GetCallerIdentityResponse><GetCallerIdentityResult><Account>111111111111</Account><Arn>arn:aws:iam::111111111111:role/test</Arn><UserId>test</UserId></GetCallerIdentityResult></GetCallerIdentityResponse>".into(),
            "HeadBucket"=>String::new(),"DescribeKey"=>"{}".into(),
            "GetInstanceProfile"=>"<GetInstanceProfileResponse><GetInstanceProfileResult/></GetInstanceProfileResponse>".into(),
            "DescribeVpcAttribute"=> {
                let attribute=get("Attribute");
                format!("<DescribeVpcAttributeResponse><vpcId>vpc-00000000000000001</vpcId><{attribute}><value>true</value></{attribute}></DescribeVpcAttributeResponse>")
            }
            "DescribeInstances"=> {
                let id=if self.foreign {
                    if !get("InstanceId.1").is_empty(){get("InstanceId.1")}
                    else if get("Filter.1.Name").contains("authority-id"){"i-aaaaaaaa"}else{""}
                }else if !self.populate {""}else if !get("InstanceId.1").is_empty(){get("InstanceId.1")}
                    else if get("Filter.1.Name")=="client-token" {"i-aaaaaaaa"}
                    else if get("Filter.1.Name").contains("operation-id"){"i-bbbbbbbb"}else{"i-dddddddd"};
                let mut roots=if id.is_empty(){String::new()}else{reservation(id)};
                if self.foreign && !id.is_empty() {
                    roots=roots.replace("<instancesSet>","<ownerId>111111111111</ownerId><instancesSet>")
                        .replace("<networkInterfaceSet/>","<clientToken>foreign-token</clientToken><tagSet><item><key>borrowser:authority-id</key><value>synthetic-authority</value></item><item><key>borrowser:operation-id</key><value>foreign-operation</value></item></tagSet><networkInterfaceSet/>");
                }
                if self.populate && id=="i-aaaaaaaa" {
                    roots=roots.replace("<networkInterfaceSet/>","<networkInterfaceSet><item><networkInterfaceId>eni-aaaaaaaa</networkInterfaceId><attachment><attachmentId>eni-attach-aaaaaaaa</attachmentId></attachment></item></networkInterfaceSet>")
                        .replace("<blockDeviceMapping/>","<blockDeviceMapping><item><deviceName>/dev/sda1</deviceName><ebs><volumeId>vol-aaaaaaaa</volumeId></ebs></item></blockDeviceMapping>");
                }
                xml(ReadOperationV1::DescribeInstances,&roots)
            }
            "DescribeNetworkInterfaces"=> {
                let id=if !self.populate || get("Filter.1.Name")=="attachment.instance-id" {""}
                    else if !get("NetworkInterfaceId.1").is_empty(){get("NetworkInterfaceId.1")}
                    else if get("Filter.1.Name").contains("operation-id"){"eni-aaaaaaaa"}else{"eni-bbbbbbbb"};
                let instance=if id=="eni-aaaaaaaa"{"i-cccccccc"}else{"i-eeeeeeee"};
                xml(ReadOperationV1::DescribeNetworkInterfaces,&if id.is_empty(){String::new()}else{eni(id,instance)})
            }
            "DescribeVolumes"=> {
                let id=if !self.populate || get("Filter.1.Name")=="attachment.instance-id" {""}
                    else if !get("VolumeId.1").is_empty(){get("VolumeId.1")}
                    else if get("Filter.1.Name").contains("operation-id"){"vol-aaaaaaaa"}else{"vol-bbbbbbbb"};
                let instance=if id=="vol-aaaaaaaa"{"i-ffffffff"}else{"i-11111111"};
                xml(ReadOperationV1::DescribeVolumes,&if id.is_empty(){String::new()}else{volume(id,instance)})
            }
            "DescribeInstanceAttribute"=> {
                let attribute=get("Attribute");let value=if attribute=="userData"{"Cg=="}else if attribute=="instanceInitiatedShutdownBehavior"{"stop"}else{"false"};
                format!("<DescribeInstanceAttributeResponse><instanceId>{}</instanceId><{attribute}><value>{value}</value></{attribute}></DescribeInstanceAttributeResponse>",get("InstanceId"))
            }
            "DescribeIamInstanceProfileAssociations"=> {
                let item=if self.populate && (get("Filter.1.Value.1")=="i-aaaaaaaa" || !get("AssociationId.1").is_empty()) {
                    "<item><associationId>iip-assoc-aaaaaaaa</associationId><instanceId>i-bbbbbbbb</instanceId></item>"
                }else{""};
                format!("<DescribeIamInstanceProfileAssociationsResponse><iamInstanceProfileAssociationSet>{item}</iamInstanceProfileAssociationSet></DescribeIamInstanceProfileAssociationsResponse>")
            },
            "DescribeRegions"|"DescribeAvailabilityZones"|"DescribeSubnets"|"DescribeVpcs"|"DescribeSecurityGroups"|
            "DescribeRouteTables"|"DescribeVpcEndpoints"|"DescribePrefixLists"|"DescribeDhcpOptions"|"DescribeNetworkAcls"|
            "DescribeImages"|"DescribeInstanceTypes"|"DescribeInstanceTypeOfferings"=> {
                let set=match action {
                    "DescribeRegions"=>"regionInfo","DescribeAvailabilityZones"=>"availabilityZoneInfo",
                    "DescribeSubnets"=>"subnetSet","DescribeVpcs"=>"vpcSet","DescribeSecurityGroups"=>"securityGroupInfo",
                    "DescribeRouteTables"=>"routeTableSet","DescribeVpcEndpoints"=>"vpcEndpointSet",
                    "DescribePrefixLists"=>"prefixListSet","DescribeDhcpOptions"=>"dhcpOptionsSet",
                    "DescribeNetworkAcls"=>"networkAclSet","DescribeImages"=>"imagesSet",
                    "DescribeInstanceTypes"=>"instanceTypeSet",_=>"instanceTypeOfferingSet",
                };
                format!("<{action}Response><{set}/></{action}Response>")
            },
            _=>{status=400;"<Response><Errors><Error><Code>UnauthorizedOperation</Code></Error></Errors></Response>".into()},
        };
        self.requests.lock().unwrap().push(params);
        let response = super::tests::response(status, &body, Some("eu-central-1"))
            .try_into()
            .unwrap();
        HttpConnectorFuture::new(async move { Ok(response) })
    }
}
fn scripted_session(
    p: &crate::dispatch::PreparedLaunchV2,
    round: ObservationRound,
    populate: bool,
) -> (ObservationSession, RecordedRequests) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let connector = SharedHttpConnector::new(DiscoveryScript {
        requests: requests.clone(),
        populate,
        foreign: false,
    });
    let http = http_client_fn(move |_, _| connector.clone());
    (
        ObservationSession::bounded(
            &p.deployment,
            MANIFEST,
            secret(),
            BoundedHttp::new(http, round),
        )
        .unwrap(),
        requests,
    )
}
#[tokio::test]
async fn every_independent_path_survives_coordinator_closure_and_offline_recomputation() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for populate in [false, true] {
        let (mut session, requests) = scripted_session(&p, ObservationRound::test(), populate);
        let result = super::operation_discovery::observe(&mut session, &inputs)
            .await
            .unwrap();
        assert!(
            result.report.complete,
            "execution={:?} representation={:?} unresolved={} exhausted={} stop={:?}",
            result
                .report
                .execution
                .iter()
                .filter(|(_, s)| **s != WorkStatus::Complete)
                .collect::<Vec<_>>(),
            result
                .report
                .representation
                .iter()
                .filter(|(_, a)| !a.complete())
                .collect::<Vec<_>>(),
            result.report.relationships.unresolved_references,
            result.report.metadata_exhausted,
            result.execution.stop
        );
        let ids: Vec<_> = result
            .report
            .plausible_instances()
            .map(|id| id.as_str())
            .collect();
        if populate {
            assert_eq!(
                ids,
                [
                    "i-11111111",
                    "i-aaaaaaaa",
                    "i-bbbbbbbb",
                    "i-cccccccc",
                    "i-dddddddd",
                    "i-eeeeeeee",
                    "i-ffffffff"
                ]
            );
        } else {
            assert!(ids.is_empty());
        }
        if populate {
            use relationships::{AllocationIdentity as I, Attribution};
            assert_eq!(
                result
                    .report
                    .attribution(&I::NetworkInterface("eni-aaaaaaaa".parse().unwrap())),
                Some(Attribution::ContradictoryLinkage)
            );
            assert!(
                result
                    .report
                    .relationships
                    .relationships
                    .iter()
                    .any(|e| e.from == I::Instance("i-aaaaaaaa".parse().unwrap())
                        && e.to == I::Volume("vol-aaaaaaaa".parse().unwrap()))
            );
            assert!(
                result
                    .report
                    .relationships
                    .resources
                    .contains_key(&I::ProfileAssociation(
                        "iip-assoc-aaaaaaaa".parse().unwrap()
                    ))
            );
        }
        let offline = derive_offline(&inputs, &result.evidence, &result.execution).unwrap();
        assert_eq!(result.report, offline);
        let mut permutation = result.evidence.evidence().clone();
        permutation.observations.records.reverse();
        permutation.observations.coverage.reverse();
        let reordered = ingest(&inputs, permutation).unwrap();
        assert_eq!(
            offline,
            derive_offline(&inputs, &reordered, &result.execution).unwrap()
        );
        let requests = requests.lock().unwrap();
        for operation in [
            "DescribeInstances",
            "DescribeNetworkInterfaces",
            "DescribeVolumes",
        ] {
            for tag in ["authority-id", "operation-id"] {
                let matches: Vec<_> = requests
                    .iter()
                    .filter(|r| {
                        r.get("Action").is_some_and(|v| v == operation)
                            && r.get("Filter.1.Name").is_some_and(|v| v.contains(tag))
                    })
                    .collect();
                assert_eq!(matches.len(), 1);
                assert!(!matches[0].contains_key("Filter.2.Name"));
            }
        }
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.get("Filter.1.Name").is_some_and(|v| v == "client-token"))
                .count(),
            1
        );
        assert!(requests.iter().all(|r| {
            !r.values()
                .any(|v| v == "instance-state-name" || v == "image-id" || v == "vpc-id")
        }));
    }
}

#[derive(Debug)]
struct FinalClock {
    calls: std::sync::atomic::AtomicU64,
    expire: u64,
    session: bool,
}
impl super::response_limits::ObservationClock for FinalClock {
    fn sample(&self) -> crate::Result<crate::scheduling::TimeSample> {
        use std::sync::atomic::Ordering;
        let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut sample = crate::test_support::genesis().time;
        sample.boottime_ns = if !self.session && count >= self.expire {
            crate::provider::limits::OBSERVATION_NS
        } else {
            0
        };
        Ok(sample)
    }
    fn realtime(&self) -> std::time::SystemTime {
        use std::{
            sync::atomic::Ordering,
            time::{Duration, SystemTime, UNIX_EPOCH},
        };
        if self.session && self.calls.load(Ordering::SeqCst) >= self.expire {
            UNIX_EPOCH + Duration::from_secs(4102444800)
        } else {
            SystemTime::now()
        }
    }
}
#[tokio::test]
async fn expiry_after_last_request_is_checked_after_report_preparation() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let clock = Arc::new(FinalClock {
        calls: 0.into(),
        expire: u64::MAX,
        session: false,
    });
    let round = ObservationRound::test_with_clock(clock.clone());
    let (mut session, requests) = scripted_session(&p, round, false);
    let baseline = super::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    assert!(baseline.report.complete);
    let final_sample = clock.calls.load(std::sync::atomic::Ordering::SeqCst);
    let request_count = requests.lock().unwrap().len();
    for session_expiry in [false, true] {
        let clock = Arc::new(FinalClock {
            calls: 0.into(),
            expire: final_sample,
            session: session_expiry,
        });
        let round = ObservationRound::test_with_clock(clock);
        let (mut session, requests) = scripted_session(&p, round, false);
        let result = super::operation_discovery::observe(&mut session, &inputs)
            .await
            .unwrap();
        assert_eq!(requests.lock().unwrap().len(), request_count);
        assert!(
            result
                .report
                .execution
                .values()
                .all(|s| *s == WorkStatus::Complete)
        );
        assert!(result.report.representation.values().all(|a| a.complete()));
        assert!(!result.report.complete);
        assert_eq!(
            result.report.final_check,
            FinalRoundCheck::Failed(if session_expiry {
                crate::provider::limits::LimitKind::Session
            } else {
                crate::provider::limits::LimitKind::Elapsed
            })
        );
        assert_eq!(
            result.report,
            derive_offline(&inputs, &result.evidence, &result.execution).unwrap()
        );
    }
}

#[tokio::test]
async fn affirmative_unrelatedness_requires_all_prerequisites_and_never_erases_evidence() {
    use crate::provider::{
        allocation_value_v5::MemberV5 as M,
        discovery::relationships::*,
        ec2_allocation_observation_v5::ObservationDataV5 as D,
        management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2 as U},
    };
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let connector = SharedHttpConnector::new(DiscoveryScript {
        requests: Arc::new(Mutex::new(Vec::new())),
        populate: false,
        foreign: true,
    });
    let http = http_client_fn(move |_, _| connector.clone());
    let round = ObservationRound::test();
    let mut session = ObservationSession::bounded(
        &p.deployment,
        MANIFEST,
        secret(),
        BoundedHttp::new(http, round),
    )
    .unwrap();
    let complete = super::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    let id = AllocationIdentity::Instance("i-aaaaaaaa".parse().unwrap());
    assert!(complete.report.complete);
    assert_eq!(
        complete.report.attribution(&id),
        Some(Attribution::AffirmativelyUnrelated)
    );
    assert!(!complete.evidence.evidence().observations.records.is_empty());
    let original = complete.evidence.evidence();
    // Each modified fact is otherwise structurally valid. No mutation is an
    // admission check; these are the evidence prerequisites for exclusion itself.
    for case in 0..7 {
        let mut changed = original.clone();
        for entry in &mut changed.observations.records {
            let ObservationEntryV5::V5(v) = entry else {
                continue;
            };
            if let D::Instance(instance) = &mut v.data {
                match case {
                    0 => instance.token = M::NotReturned,
                    1 => {
                        instance.token = M::Present(
                            p.request
                                .client_token
                                .as_str()
                                .to_owned()
                                .try_into()
                                .unwrap(),
                        )
                    }
                    2 => instance.reservation_owner = M::NotReturned,
                    3 => instance.reservation_owner = M::Present("222222222222".parse().unwrap()),
                    4 => instance.tags = V::Unavailable(U::NotReturned),
                    5 => {
                        if let V::Present(tags) = &instance.tags {
                            let mut tags = tags.as_slice().to_vec();
                            tags[1].value =
                                M::Present("synthetic-operation-1".to_owned().try_into().unwrap());
                            instance.tags = V::Present(tags.try_into().unwrap());
                        }
                    }
                    _ => (),
                }
            }
        }
        if case == 6 {
            changed
                .observations
                .coverage
                .iter_mut()
                .find(|c| c.query.operation == ReadOperationV1::DescribeInstances)
                .unwrap()
                .status = CoverageStatus::Incomplete(ReadFailureV1::AccessDenied);
        }
        let old_bytes: usize = original
            .observations
            .records
            .iter()
            .map(|v| v.canonical_bytes().unwrap().len())
            .sum();
        let new_bytes: usize = changed
            .observations
            .records
            .iter()
            .map(|v| v.canonical_bytes().unwrap().len())
            .sum();
        let mut execution = complete.execution.clone();
        execution.accounting.normalized_bytes += new_bytes.saturating_sub(old_bytes) as u64;
        let validated = ingest(&inputs, changed).unwrap();
        let result = derive_offline(&inputs, &validated, &execution).unwrap();
        assert_ne!(
            result.attribution(&id),
            Some(Attribution::AffirmativelyUnrelated),
            "case {case}"
        );
        assert!(
            result
                .plausible_instances()
                .any(|v| v.as_str() == "i-aaaaaaaa")
        );
        assert_eq!(
            validated.evidence().observations.records.len(),
            original.observations.records.len()
        );
    }
    let mut expired = complete.execution.clone();
    expired.accounting.failure = Some(crate::provider::limits::LimitKind::Session);
    expired.final_check = FinalRoundCheck::Failed(crate::provider::limits::LimitKind::Session);
    let report = derive_offline(&inputs, &complete.evidence, &expired).unwrap();
    assert_ne!(
        report.attribution(&id),
        Some(Attribution::AffirmativelyUnrelated)
    );
}

type RecordedRequests = Arc<Mutex<Vec<BTreeMap<String, String>>>>;

#[tokio::test]
async fn retained_requirements_cover_all_facts_independently_of_caller_required_flags() {
    use AdmissionFactId::*;
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let (mut session, _) = scripted_session(&p, ObservationRound::test(), true);
    let result = super::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    for fact in [
        A01, A02, A03, A04, A05, A06, A07, A08, A09, A10, A11, A12, A13, A14, A15, A16, A17, A18,
        A19, A20, A21, A22, A23, A24, A25, A26, A27, A28, A29, A30, A31, A32, A33, A34, A35, A36,
        A37, A38,
    ] {
        assert!(
            result
                .report
                .required
                .reads
                .values()
                .any(|r| r.facts.contains(fact)),
            "{fact:?}"
        );
    }
    let mut evidence = result.evidence.evidence().clone();
    for c in &mut evidence.observations.coverage {
        c.required = false;
    }
    evidence
        .reviewed_subnet_routes
        .as_mut()
        .unwrap()
        .coverage
        .required = false;
    let mut execution = result.execution.clone();
    // The literal `false` adds one byte to each conservative coverage reservation.
    execution.accounting.normalized_bytes += evidence.observations.coverage.len() as u64 + 1;
    let validated = ingest(&inputs, evidence).unwrap();
    let report = derive_offline(&inputs, &validated, &execution).unwrap();
    assert_eq!(report.required, result.report.required);
    assert!(report.complete);
}

#[path = "operation_discovery_boundary_tests.rs"]
mod operation_discovery_boundary_tests;

#[path = "reviewed_subnet_route_reads_tests.rs"]
mod reviewed_subnet_route_reads_tests;

#[path = "operation_discovery_review_tests.rs"]
mod operation_discovery_review_tests;

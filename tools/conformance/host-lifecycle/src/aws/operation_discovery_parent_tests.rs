use super::*;
use crate::provider::{
    allocation_value_v5::MemberV5 as M, ec2_allocation_observation_v5::ObservationDataV5 as D,
    inventory::ResourceIdentity as R,
};
use relationships::{
    AllocationIdentity as I, Attribution as A, ParentIdentityAgreement as Agreement,
    RelationshipGapKind as Gap, RelationshipKind as Kind,
};

const PARENT: &str = "i-bbbbbbbb";
const ENCLOSING: &str = "i-aaaaaaaa";

/// The foreign component is unchanged. An exact read of the additional parent
/// has a genuine terminal empty response; it must not supply an accidental edge
/// from requested A to returned B and thereby mask the positional-link defect.
#[derive(Debug)]
struct ParentProbeTransport(ForeignComponent);
impl HttpConnector for ParentProbeTransport {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let text = std::str::from_utf8(request.body().bytes().unwrap_or_default()).unwrap();
        if text.split('&').any(|p| p == "InstanceId.1=i-bbbbbbbb") {
            let body = xml(ReadOperationV1::DescribeInstances, "");
            HttpConnectorFuture::new(async move { Ok(response(200, &body).try_into().unwrap()) })
        } else {
            self.0.call(request)
        }
    }
}

async fn foreign_baseline(
    inputs: &DiscoveryInputs<'_>,
) -> (
    crate::aws::operation_discovery::ObservedDiscovery,
    ObservationSession,
) {
    let connector =
        SharedHttpConnector::new(ParentProbeTransport(ForeignComponent(DiscoveryScript {
            requests: Arc::new(Mutex::new(Vec::new())),
            populate: false,
            foreign: true,
        })));
    let http = http_client_fn(move |_, _| connector.clone());
    let mut session = ObservationSession::bounded(
        &inputs.prepared.deployment,
        MANIFEST,
        secret(),
        BoundedHttp::new(http, ObservationRound::test()),
    )
    .unwrap();
    let baseline = crate::aws::operation_discovery::observe(&mut session, inputs)
        .await
        .unwrap();
    assert!(baseline.report.complete);
    for id in [
        I::Instance(ENCLOSING.parse().unwrap()),
        I::NetworkInterface("eni-aaaaaaaa".parse().unwrap()),
        I::Volume("vol-aaaaaaaa".parse().unwrap()),
        I::ProfileAssociation("iip-assoc-aaaaaaaa".parse().unwrap()),
    ] {
        assert_eq!(
            baseline.report.attribution(&id),
            Some(A::AffirmativelyUnrelated)
        );
    }
    assert!(!baseline.report.relationships.positional_parents.is_empty());
    assert!(
        baseline
            .report
            .relationships
            .positional_parents
            .iter()
            .all(|p| p.agreement == Agreement::Consistent)
    );
    (baseline, session)
}

fn record_bytes(raw: &DiscoveryEvidence) -> u64 {
    raw.observations
        .records
        .iter()
        .map(|r| r.canonical_bytes().unwrap().len() as u64)
        .sum()
}

fn replace_authority_parent(raw: &mut DiscoveryEvidence, token: &str) {
    for entry in &mut raw.observations.records {
        let ObservationEntryV5::V5(r) = entry else {
            continue;
        };
        if r.query.operation != ReadOperationV1::DescribeInstances
            || !matches!(r.query.scope, QueryScopeV1::AuthorityTag { .. })
        {
            continue;
        }
        match &mut r.data {
            D::Instance(v) => {
                v.id = M::Present(PARENT.parse().unwrap());
                v.token = M::Present(token.to_owned().try_into().unwrap());
            }
            D::InstanceOptions(v) => v.id = M::Present(PARENT.parse().unwrap()),
            D::ExcludedFeatures(v) => {
                v.resource_id = M::Present(PARENT.to_owned().try_into().unwrap())
            }
            _ => (),
        }
    }
}

async fn read_additional_parent(session: &mut ObservationSession, raw: &mut DiscoveryEvidence) {
    let instance = PARENT.parse::<crate::identity::InstanceId>().unwrap();
    let mut reads = vec![(
        ReadOperationV1::DescribeInstances,
        QueryScopeV1::Exact {
            identities: vec![R::Instance(instance.clone())],
        },
    )];
    for operation in [
        ReadOperationV1::DescribeNetworkInterfaces,
        ReadOperationV1::DescribeVolumes,
        ReadOperationV1::DescribeIamInstanceProfileAssociations,
    ] {
        reads.push((
            operation,
            QueryScopeV1::AttachedTo {
                instance: instance.clone(),
            },
        ));
    }
    for attribute in [
        InstanceAttribute::UserData,
        InstanceAttribute::InstanceInitiatedShutdownBehavior,
        InstanceAttribute::DisableApiTermination,
        InstanceAttribute::DisableApiStop,
    ] {
        reads.push((
            ReadOperationV1::DescribeInstanceAttribute,
            QueryScopeV1::InstanceAttribute {
                instance: instance.clone(),
                attribute,
            },
        ));
    }
    let before = session.round().snapshot();
    for read in reads {
        let result = session
            .allocation(AllocationRead::try_from(read).unwrap(), true)
            .await
            .unwrap();
        assert_eq!(result.coverage.status, CoverageStatus::Complete);
        raw.observations.records.extend(result.records);
        raw.observations.coverage.push(result.coverage);
    }
    let after = session.round().snapshot();
    // Independently specified: four empty list reads, four singleton attributes.
    assert_eq!(after.requests - before.requests, 8);
    assert_eq!(after.source_occurrences - before.source_occurrences, 4);
    assert_eq!(after.retained_outputs - before.retained_outputs, 4);
}

#[tokio::test]
async fn positional_parent_positive_linkage_cannot_exclude_nested_foreign_component() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let (baseline, mut session) = foreign_baseline(&inputs).await;
    let mut raw = baseline.evidence.evidence().clone();
    let before = record_bytes(&raw);
    replace_authority_parent(&mut raw, c.fields().binding.client_token.as_str());
    let extra_bytes = record_bytes(&raw).saturating_sub(before);
    read_additional_parent(&mut session, &mut raw).await;
    let mut execution = baseline.execution.clone();
    execution.accounting = session.round().snapshot();
    execution.accounting.normalized_bytes += extra_bytes;
    let valid = ingest(&inputs, raw).unwrap();
    valid.evidence().observations.validate().unwrap();
    let report = derive_offline(&inputs, &valid, &execution).unwrap();
    assert!(report.complete, "{report:?}");
    assert!(!report.accounting_gap);
    assert!(report.representation.values().all(|a| a.complete()));
    assert!(
        report
            .execution
            .values()
            .all(|s| *s == WorkStatus::Complete)
    );
    let ids = [
        I::Instance(PARENT.parse().unwrap()),
        I::Instance(ENCLOSING.parse().unwrap()),
        I::NetworkInterface("eni-aaaaaaaa".parse().unwrap()),
        I::Volume("vol-aaaaaaaa".parse().unwrap()),
    ];
    eprintln!(
        "complete={} attributions={:?}",
        report.complete,
        ids.iter()
            .map(|id| report.attribution(id))
            .collect::<Vec<_>>()
    );
    for id in &ids {
        assert_ne!(
            report.attribution(id),
            Some(A::AffirmativelyUnrelated),
            "{id:?}"
        );
    }
    assert!(
        report
            .plausible_instances()
            .any(|id| id.as_str() == ENCLOSING)
    );
    for id in &ids {
        assert!(report.relationships.resources[id].positive_linkage);
        assert_eq!(report.attribution(id), Some(A::ContradictoryLinkage));
    }
    assert!(
        report
            .relationships
            .positional_parents
            .iter()
            .any(|p| p.agreement == Agreement::Contradictory)
    );
    assert_positional_provenance(&report, &valid);
    assert_permutations(&inputs, valid.evidence(), &execution, &report);
}

fn assert_positional_provenance(report: &DiscoveryReport, evidence: &ValidatedDiscoveryEvidence) {
    for fact in &report.relationships.positional_parents {
        let Some(parent) = fact.parent else { continue };
        let ObservationEntryV5::V5(p) = &evidence.evidence().observations.records[parent] else {
            panic!()
        };
        let ObservationEntryV5::V5(c) = &evidence.evidence().observations.records[fact.child]
        else {
            panic!()
        };
        assert_eq!(p.query, c.query);
        assert_eq!(p.source.page, c.source.page);
        assert_eq!(
            c.source.path.containing_list().unwrap().0,
            Some(p.source.path)
        );
        assert!(matches!(p.data, D::Instance(_) | D::Volume(_)));
    }
    for edge in &report.relationships.relationships {
        let Kind::PositionalParent { parent } = edge.kind else {
            continue;
        };
        let child = edge.observation.unwrap();
        assert!(
            report
                .relationships
                .positional_parents
                .iter()
                .any(|f| f.parent == Some(parent) && f.child == child)
        );
        for id in [&edge.from, &edge.to] {
            let read = report.required.reads.values().find(|r| matches!(&r.query,
                crate::provider::reviewed_subnet_routes_v1::DiscoveryQuery::Existing(q)
                if matches!(&q.scope, QueryScopeV1::Exact { identities } if identities == &[id.resource()]))).unwrap();
            assert!(read.observations.contains(&parent));
            assert!(read.observations.contains(&child));
            assert!(read.facts.contains(AdmissionFactId::A23));
        }
    }
}

fn assert_permutations(
    inputs: &DiscoveryInputs<'_>,
    raw: &DiscoveryEvidence,
    execution: &DiscoveryExecutionFacts,
    expected: &DiscoveryReport,
) {
    let mut raw = raw.clone();
    for reverse in [true, false] {
        if reverse {
            raw.observations.records.reverse();
            raw.observations.coverage.reverse();
        } else {
            raw.observations.records.rotate_left(1);
        }
        let valid = ingest(inputs, raw.clone()).unwrap();
        assert_eq!(
            &derive_offline(inputs, &valid, execution).unwrap(),
            expected
        );
    }
}

fn nested_instance(id: &str, token: &str, eni_count: usize, ebs: bool) -> String {
    let interfaces = "<item><networkInterfaceId>eni-aaaaaaaa</networkInterfaceId><ownerId>111111111111</ownerId><attachment><attachmentId>eni-attach-aaaaaaaa</attachmentId></attachment></item>".repeat(eni_count);
    let mapping = if ebs {
        "<item><deviceName>/dev/sda1</deviceName><ebs><volumeId>vol-aaaaaaaa</volumeId><volumeOwnerId>111111111111</volumeOwnerId></ebs></item>"
    } else {
        ""
    };
    format!(
        "<item><ownerId>111111111111</ownerId><instancesSet><item><instanceId>{id}</instanceId><clientToken>{token}</clientToken><networkInterfaceSet>{interfaces}</networkInterfaceSet><blockDeviceMapping>{mapping}</blockDeviceMapping><secondaryInterfaceSet/></item></instancesSet></item>"
    )
}

fn authority_read(inputs: &DiscoveryInputs<'_>, operation: ReadOperationV1) -> AllocationRead {
    AllocationRead::try_from((
        operation,
        QueryScopeV1::AuthorityTag {
            authority: inputs.context.fields().binding.authority_id.clone(),
        },
    ))
    .unwrap()
}

async fn supplied(
    inputs: &DiscoveryInputs<'_>,
    operation: ReadOperationV1,
    roots: &str,
) -> (DiscoveryEvidence, DiscoveryExecutionFacts) {
    let (mut session, _, round) = reader(vec![response(200, &xml(operation, roots))]);
    let result = session
        .allocation(authority_read(inputs, operation), true)
        .await
        .unwrap();
    assert_eq!(result.coverage.status, CoverageStatus::Complete);
    (
        bundle(inputs.context_identity().unwrap(), result),
        facts(&round),
    )
}

fn change_enclosing(raw: &mut DiscoveryEvidence) {
    for entry in &mut raw.observations.records {
        if let ObservationEntryV5::V5(r) = entry {
            match &mut r.data {
                D::NetworkInterface(v) => {
                    v.enclosing_instance = M::Present(ENCLOSING.parse().unwrap())
                }
                D::InstanceEniAttachment(v) => {
                    v.enclosing_instance = M::Present(ENCLOSING.parse().unwrap())
                }
                D::InstanceEbsMapping(v) => {
                    v.enclosing_instance = M::Present(ENCLOSING.parse().unwrap())
                }
                D::UnsupportedSecondaryInterface(v) => {
                    v.enclosing_instance = M::Present(ENCLOSING.parse().unwrap())
                }
                _ => (),
            }
        }
    }
}

#[tokio::test]
async fn positional_eni_and_ebs_links_preserve_actual_and_enclosing_perspectives() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for ebs in [false, true] {
        let roots = nested_instance(
            PARENT,
            c.fields().binding.client_token.as_str(),
            usize::from(!ebs),
            ebs,
        );
        let (mut raw, mut execution) =
            supplied(&inputs, ReadOperationV1::DescribeInstances, &roots).await;
        assert_eq!(raw.observations.coverage[0].records, 3); // reservation, instance, one extracted child
        let before = record_bytes(&raw);
        change_enclosing(&mut raw);
        execution.accounting.normalized_bytes += record_bytes(&raw).saturating_sub(before);
        let valid = ingest(&inputs, raw).unwrap();
        let report = derive_offline(&inputs, &valid, &execution).unwrap();
        assert!(report.representation.values().all(|a| a.complete()));
        let from = I::Instance(PARENT.parse().unwrap());
        let enclosing = I::Instance(ENCLOSING.parse().unwrap());
        let child = if ebs {
            I::Volume("vol-aaaaaaaa".parse().unwrap())
        } else {
            I::NetworkInterface("eni-aaaaaaaa".parse().unwrap())
        };
        for to in [&enclosing, &child] {
            assert!(
                report
                    .relationships
                    .relationships
                    .iter()
                    .any(|e| e.from == from
                        && &e.to == to
                        && matches!(e.kind, Kind::PositionalParent { .. }))
            );
            assert!(report.relationships.resources[to].positive_linkage);
            assert_eq!(
                report.relationships.resources[to].attribution,
                A::ContradictoryLinkage
            );
            assert_eq!(report.relationships.resources[to].discovery_depth, Some(1));
        }
        assert_eq!(
            report.relationships.positional_parents.len(),
            if ebs { 1 } else { 2 }
        );
        assert!(
            report
                .relationships
                .positional_parents
                .iter()
                .all(|f| f.agreement == Agreement::Contradictory)
        );
        assert_positional_provenance(&report, &valid);
        assert_permutations(&inputs, valid.evidence(), &execution, &report);
    }
}

#[tokio::test]
async fn positional_volume_parent_keeps_enclosing_and_independent_volume_ids() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let roots = volume("vol-aaaaaaaa", ENCLOSING).replace("<attachmentSet>", &format!("<tagSet><item><key>borrowser:operation-id</key><value>{}</value></item></tagSet><attachmentSet>", c.fields().binding.operation_id.as_str()));
    let (mut raw, mut execution) =
        supplied(&inputs, ReadOperationV1::DescribeVolumes, &roots).await;
    assert_eq!(raw.observations.coverage[0].records, 3); // volume, tag, attachment
    let before = record_bytes(&raw);
    for e in &mut raw.observations.records {
        if let ObservationEntryV5::V5(r) = e
            && let D::VolumeAttachment(v) = &mut r.data
        {
            v.enclosing_volume = M::Present("vol-bbbbbbbb".parse().unwrap());
            v.volume = M::Present("vol-cccccccc".parse().unwrap());
        }
    }
    execution.accounting.normalized_bytes += record_bytes(&raw).saturating_sub(before);
    let valid = ingest(&inputs, raw).unwrap();
    let report = derive_offline(&inputs, &valid, &execution).unwrap();
    let actual = I::Volume("vol-aaaaaaaa".parse().unwrap());
    for id in [
        I::Volume("vol-bbbbbbbb".parse().unwrap()),
        I::Volume("vol-cccccccc".parse().unwrap()),
        I::Instance(ENCLOSING.parse().unwrap()),
    ] {
        assert!(
            report
                .relationships
                .relationships
                .iter()
                .any(|e| e.from == actual
                    && e.to == id
                    && matches!(e.kind, Kind::PositionalParent { .. }))
        );
        assert!(report.relationships.resources[&id].positive_linkage);
        if matches!(id, I::Volume(_)) {
            assert_eq!(report.attribution(&id), Some(A::ContradictoryLinkage));
        }
    }
    assert_eq!(
        report.relationships.positional_parents[0].agreement,
        Agreement::Contradictory
    );
    assert_positional_provenance(&report, &valid);
    assert_permutations(&inputs, valid.evidence(), &execution, &report);
}

#[tokio::test]
async fn positional_lookup_isolates_queries_pages_and_sibling_instances() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let op = ReadOperationV1::DescribeInstances;
    let unrelated = "<item><instanceId>i-cccccccc</instanceId><networkInterfaceSet/><blockDeviceMapping/><secondaryInterfaceSet/></item>";
    let first = nested_instance(PARENT, c.fields().binding.client_token.as_str(), 1, false)
        .replace("</instancesSet>", &format!("{unrelated}</instancesSet>"));
    let second =
        nested_instance("i-dddddddd", "foreign", 1, false).replace("eni-aaaaaaaa", "eni-dddddddd");
    let other_query =
        nested_instance("i-eeeeeeee", "foreign", 1, false).replace("eni-aaaaaaaa", "eni-eeeeeeee");
    let first_page = xml(op, &first).replace(
        "</DescribeInstancesResponse>",
        "<nextToken>next</nextToken></DescribeInstancesResponse>",
    );
    let (mut session, _, round) = reader(vec![
        response(200, &first_page),
        response(200, &xml(op, &second)),
        response(200, &xml(op, &other_query)),
    ]);
    let a = session
        .allocation(authority_read(&inputs, op), true)
        .await
        .unwrap();
    let b = session.allocation(query(op), true).await.unwrap();
    assert_eq!(
        (a.coverage.pages, a.coverage.records, a.records.len()),
        (2, 7, 15)
    );
    assert_eq!(
        (b.coverage.pages, b.coverage.records, b.records.len()),
        (1, 3, 6)
    );
    let mut raw = bundle(inputs.context_identity().unwrap(), a);
    raw.observations.records.extend(b.records);
    raw.observations.coverage.push(b.coverage);
    let execution = facts(&round);
    let valid = ingest(&inputs, raw.clone()).unwrap();
    let report = derive_offline(&inputs, &valid, &execution).unwrap();
    assert_eq!(report.relationships.positional_parents.len(), 6);
    assert!(
        report
            .relationships
            .positional_parents
            .iter()
            .all(|f| f.agreement == Agreement::Consistent)
    );
    assert_positional_provenance(&report, &valid);
    for (instance, eni) in [
        (PARENT, "eni-aaaaaaaa"),
        ("i-dddddddd", "eni-dddddddd"),
        ("i-eeeeeeee", "eni-eeeeeeee"),
    ] {
        let from = I::Instance(instance.parse().unwrap());
        let to = I::NetworkInterface(eni.parse().unwrap());
        let edges: Vec<_> = report
            .relationships
            .relationships
            .iter()
            .filter(|e| matches!(e.kind, Kind::PositionalParent { .. }) && e.from == from)
            .collect();
        assert_eq!(edges.len(), 2);
        assert!(edges.iter().all(|e| e.to == to));
    }
    let sibling = I::Instance("i-cccccccc".parse().unwrap());
    assert!(!report.relationships.resources[&sibling].positive_linkage);
    assert_eq!(report.attribution(&sibling), Some(A::DiscoveryHit));
    assert!(
        report
            .relationships
            .relationships
            .iter()
            .all(|e| e.from != sibling && e.to != sibling)
    );
    assert_permutations(&inputs, valid.evidence(), &execution, &report);

    // Other queries/pages and same-source siblings must not fill the missing
    // actual Instance projection at authority/page 1/reservation 0/instance 0.
    raw.observations.records.retain(|e| !matches!(e, ObservationEntryV5::V5(r)
        if matches!(r.query.scope, QueryScopeV1::AuthorityTag { .. }) && u64::from(r.source.page) == 1
        && matches!(&r.data, D::Instance(v) if matches!(&v.id, M::Present(id) if id.as_str() == PARENT))));
    let valid = ingest(&inputs, raw).unwrap();
    let partial = derive_offline(&inputs, &valid, &execution).unwrap();
    assert!(!partial.complete);
    assert_eq!(
        partial
            .relationships
            .positional_parents
            .iter()
            .filter(|f| f.parent.is_none())
            .count(),
        2
    );
    assert_eq!(
        partial
            .relationships
            .gaps
            .iter()
            .filter(|g| g.kind == Gap::ParentProjectionUnavailable)
            .count(),
        2
    );
    assert_permutations(&inputs, valid.evidence(), &execution, &partial);
}

#[tokio::test]
async fn positional_missing_projection_and_unavailable_identities_remain_explicit() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for is_volume in [false, true] {
        let (op, roots) = if is_volume {
            (
                ReadOperationV1::DescribeVolumes,
                volume("vol-aaaaaaaa", PARENT),
            )
        } else {
            (
                ReadOperationV1::DescribeInstances,
                nested_instance(PARENT, c.fields().binding.client_token.as_str(), 1, false),
            )
        };
        let (raw, execution) = supplied(&inputs, op, &roots).await;
        for case in 0..3 {
            let mut partial = raw.clone();
            let before = record_bytes(&partial);
            if case == 0 {
                partial.observations.records.retain(|e| !matches!(e, ObservationEntryV5::V5(r) if matches!(r.data, D::Instance(_) | D::Volume(_))));
            } else {
                for e in &mut partial.observations.records {
                    let ObservationEntryV5::V5(r) = e else {
                        continue;
                    };
                    match (&mut r.data, case) {
                        (D::Instance(v), 1) => v.id = M::NotReturned,
                        (D::Volume(v), 1) => v.id = M::NotReturned,
                        (D::NetworkInterface(v), 2) => v.enclosing_instance = M::NotReturned,
                        (D::InstanceEniAttachment(v), 2) => v.enclosing_instance = M::NotReturned,
                        (D::VolumeAttachment(v), 2) => v.enclosing_volume = M::NotReturned,
                        _ => (),
                    }
                }
            }
            let mut execution = execution.clone();
            execution.accounting.normalized_bytes += record_bytes(&partial).saturating_sub(before);
            let valid = ingest(&inputs, partial).unwrap();
            valid.evidence().observations.validate().unwrap();
            let report = derive_offline(&inputs, &valid, &execution).unwrap();
            assert!(!report.complete);
            assert!(
                report
                    .relationships
                    .positional_parents
                    .iter()
                    .all(|f| f.agreement == Agreement::Unavailable)
            );
            let expected = [
                Gap::ParentProjectionUnavailable,
                Gap::ParentIdentityUnavailable,
                Gap::EnclosingIdentityUnavailable,
            ][case];
            assert!(
                report
                    .relationships
                    .gaps
                    .iter()
                    .any(|g| g.kind == expected && g.observation.is_some())
            );
            assert_eq!(
                valid.evidence().observations.coverage,
                raw.observations.coverage
            );
            assert!(
                report
                    .relationships
                    .resources
                    .keys()
                    .all(|id| report.attribution(id) != Some(A::AffirmativelyUnrelated))
            );
            assert_permutations(&inputs, valid.evidence(), &execution, &report);
        }
    }
}

#[tokio::test]
async fn positional_links_keep_same_source_disagreement_and_unsupported_secondary_evidence() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let roots = nested_instance(PARENT, c.fields().binding.client_token.as_str(), 0, false)
        .replace("<secondaryInterfaceSet/>", "<secondaryInterfaceSet><item><secondaryInterfaceId>opaque-secondary</secondaryInterfaceId></item></secondaryInterfaceSet>");
    let op = ReadOperationV1::DescribeInstances;
    let (mut session, _, round) = reader(vec![response(200, &xml(op, &roots))]);
    let result = session
        .allocation(authority_read(&inputs, op), true)
        .await
        .unwrap();
    assert_eq!(
        result.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Unsupported)
    );
    let mut raw = bundle(inputs.context_identity().unwrap(), result);
    let mut execution = facts(&round);
    let before = record_bytes(&raw);
    change_enclosing(&mut raw);
    for e in &mut raw.observations.records {
        if let ObservationEntryV5::V5(r) = e
            && let D::InstanceOptions(v) = &mut r.data
        {
            v.id = M::Present("i-cccccccc".parse().unwrap());
        }
    }
    execution.accounting.normalized_bytes += record_bytes(&raw).saturating_sub(before);
    let valid = ingest(&inputs, raw).unwrap();
    let report = derive_offline(&inputs, &valid, &execution).unwrap();
    assert!(!report.complete); // unsupported expansion, not the identity disagreement
    assert_eq!(report.relationships.resources.len(), 3); // no invented secondary identity
    assert_eq!(report.relationships.positional_parents.len(), 1);
    assert_eq!(
        report.relationships.positional_parents[0].agreement,
        Agreement::Contradictory
    );
    assert!(
        report
            .relationships
            .gaps
            .iter()
            .any(|g| g.kind == Gap::UnsupportedReference)
    );
    let actual = I::Instance(PARENT.parse().unwrap());
    let sibling = I::Instance("i-cccccccc".parse().unwrap());
    assert!(
        report
            .relationships
            .relationships
            .iter()
            .any(|e| e.kind == Kind::SameSource
                && ((e.from == actual && e.to == sibling)
                    || (e.to == actual && e.from == sibling)))
    );
    for node in report.relationships.resources.values() {
        assert!(node.positive_linkage);
        assert_eq!(node.attribution, A::ContradictoryLinkage);
    }
    assert_positional_provenance(&report, &valid);
    assert_permutations(&inputs, valid.evidence(), &execution, &report);
}

#[tokio::test]
async fn positional_fact_and_link_exhaustion_preserves_evidence_and_disables_exclusion() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    // Forty ENI occurrences, two projections each, all retained by the real
    // adapter. The original graph fits; adding positional facts/links exhausts it.
    let roots = nested_instance(PARENT, c.fields().binding.client_token.as_str(), 40, false);
    let (mut raw, mut execution) =
        supplied(&inputs, ReadOperationV1::DescribeInstances, &roots).await;
    assert_eq!(
        (
            raw.observations.coverage[0].records,
            raw.observations.records.len()
        ),
        (42, 84)
    );
    let before = record_bytes(&raw);
    change_enclosing(&mut raw);
    execution.accounting.normalized_bytes += record_bytes(&raw).saturating_sub(before);
    let valid = ingest(&inputs, raw.clone()).unwrap();
    let report = derive_offline(&inputs, &valid, &execution).unwrap();
    assert!(report.relationships.exhausted);
    assert!(report.metadata_exhausted);
    assert!(report.metadata_bytes <= 65_536);
    assert!(!report.complete);
    assert!(!report.relationships.positional_parents.is_empty());
    assert!(report.relationships.positional_parents.len() < 80);
    assert!(
        report
            .relationships
            .relationships
            .iter()
            .any(|e| matches!(e.kind, Kind::PositionalParent { .. }))
    );
    assert_eq!(valid.evidence().observations.records.len(), 84);
    assert_eq!(
        valid.evidence().observations.coverage,
        raw.observations.coverage
    );
    assert!(report.representation.values().all(|a| a.complete()));
    assert!(
        report
            .relationships
            .resources
            .keys()
            .all(|id| report.attribution(id) != Some(A::AffirmativelyUnrelated))
    );
    assert_permutations(&inputs, valid.evidence(), &execution, &report);
}

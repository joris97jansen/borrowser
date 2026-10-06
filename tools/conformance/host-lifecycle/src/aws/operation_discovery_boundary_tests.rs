use super::*;

#[tokio::test]
async fn whole_resource_omissions_pass_frozen_validation_but_never_complete_representation() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for op in [
        ReadOperationV1::DescribeNetworkInterfaces,
        ReadOperationV1::DescribeVolumes,
        ReadOperationV1::DescribeInstances,
    ] {
        let items = match op {
            ReadOperationV1::DescribeNetworkInterfaces => {
                eni("eni-aaaaaaaa", "i-aaaaaaaa")
                    + &eni("eni-bbbbbbbb", "i-cccccccc")
                    + &eni("eni-cccccccc", "i-bbbbbbbb")
            }
            ReadOperationV1::DescribeVolumes => {
                volume("vol-aaaaaaaa", "i-aaaaaaaa")
                    + &volume("vol-bbbbbbbb", "i-cccccccc")
                    + &volume("vol-cccccccc", "i-bbbbbbbb")
            }
            _ => {
                reservation("i-aaaaaaaa") + &reservation("i-cccccccc") + &reservation("i-bbbbbbbb")
            }
        };
        let (mut session, _, round) = reader(vec![response(200, &xml(op, &items))]);
        let result = session.allocation(query(op), true).await.unwrap();
        // Independently authored source accounting: 3 ENIs; 3 volumes+3 attachments;
        // or 3 reservations+3 instances. Output projection counts are independent.
        let (sources, outputs) = if op == ReadOperationV1::DescribeNetworkInterfaces {
            (3, 9)
        } else if op == ReadOperationV1::DescribeVolumes {
            (6, 9)
        } else {
            (6, 12)
        };
        assert_eq!(result.coverage.status, CoverageStatus::Complete);
        assert_eq!(result.coverage.records, sources);
        assert_eq!(result.records.len(), outputs);
        let complete = bundle(c.identity().unwrap(), result);
        let supplied = facts(&round);
        let valid = ingest(&inputs, complete.clone()).unwrap();
        let baseline = derive_offline(&inputs, &valid, &supplied).unwrap();
        assert!(baseline.representation.values().all(|a| a.complete()));
        assert!(
            baseline
                .plausible_instances()
                .any(|id| id.as_str() == "i-cccccccc")
        );
        for omitted in 0..3 {
            let mut partial = complete.clone();
            partial.observations.records.retain(|r| root(r) != omitted);
            partial
                .observations
                .records
                .sort_by_cached_key(|r| r.canonical_bytes().unwrap());
            partial.observations.validate().unwrap(); // The intentional historical partial-evidence contract.
            let valid = ingest(&inputs, partial.clone()).unwrap();
            let report = derive_offline(&inputs, &valid, &supplied).unwrap();
            assert!(!report.complete);
            assert!(report.accounting_gap);
            assert!(report.representation.values().any(|a| !a.complete()));
            assert_eq!(
                valid.evidence().observations.coverage[0].status,
                CoverageStatus::Complete
            );
            if omitted == 1 {
                assert!(
                    !report
                        .plausible_instances()
                        .any(|id| id.as_str() == "i-cccccccc")
                );
            }
            partial.observations.records.reverse();
            let permuted = ingest(&inputs, partial).unwrap();
            assert_eq!(
                report,
                derive_offline(&inputs, &permuted, &supplied).unwrap()
            );
        }
    }
}

#[tokio::test]
async fn whole_nonempty_page_and_whole_query_omission_are_detected_but_empty_pages_are_valid() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let op = ReadOperationV1::DescribeNetworkInterfaces;
    let page = |roots: &str, token: bool| {
        xml(op, roots).replace(
            "</DescribeNetworkInterfacesResponse>",
            if token {
                "<nextToken>next</nextToken></DescribeNetworkInterfacesResponse>"
            } else {
                "</DescribeNetworkInterfacesResponse>"
            },
        )
    };
    for first_empty in [false, true] {
        let roots = eni("eni-aaaaaaaa", "i-cccccccc");
        let (mut session, _, round) = reader(vec![
            response(200, &page(if first_empty { "" } else { &roots }, true)),
            response(200, &page(if first_empty { &roots } else { "" }, false)),
        ]);
        let result = session.allocation(query(op), true).await.unwrap();
        assert_eq!((result.coverage.records, result.coverage.pages), (1, 2));
        let complete = bundle(c.identity().unwrap(), result);
        let supplied = facts(&round);
        let valid = ingest(&inputs, complete.clone()).unwrap();
        assert!(
            derive_offline(&inputs, &valid, &supplied)
                .unwrap()
                .representation
                .values()
                .all(|a| a.complete())
        );
        let mut omitted = complete;
        omitted.observations.records.clear();
        omitted.observations.validate().unwrap();
        let valid = ingest(&inputs, omitted).unwrap();
        let report = derive_offline(&inputs, &valid, &supplied).unwrap();
        assert!(!report.complete);
        assert!(report.representation.values().any(|a| !a.complete()));
    }
    let (mut session, _, round) = reader(vec![response(200, &xml(op, ""))]);
    let result = session.allocation(query(op), true).await.unwrap();
    assert_eq!(result.coverage.records, 0);
    let valid = ingest(&inputs, bundle(c.identity().unwrap(), result)).unwrap();
    assert!(
        derive_offline(&inputs, &valid, &facts(&round))
            .unwrap()
            .representation
            .values()
            .all(|a| a.complete())
    );
}

#[tokio::test]
async fn omitted_parent_child_or_sibling_remains_partial() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let op = ReadOperationV1::DescribeInstances;
    let (mut session, _, round) = reader(vec![response(200, &xml(op, &reservation("i-aaaaaaaa")))]);
    let result = session.allocation(query(op), true).await.unwrap();
    let complete = bundle(c.identity().unwrap(), result);
    let supplied = facts(&round);
    for removed in 0..complete.observations.records.len() {
        let mut partial = complete.clone();
        partial.observations.records.remove(removed);
        let valid = ingest(&inputs, partial).unwrap();
        let report = derive_offline(&inputs, &valid, &supplied).unwrap();
        assert!(report.representation.values().any(|a| !a.complete()));
    }
}

#[tokio::test]
async fn omitted_or_rejected_queries_and_inconsistent_accounting_cannot_manufacture_completeness() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let (mut session, _) = scripted_session(&p, ObservationRound::test(), false);
    let complete = crate::aws::operation_discovery::observe(&mut session, &inputs)
        .await
        .unwrap();
    assert!(complete.report.complete);
    let mut supplied = complete.evidence.evidence().clone();
    let removed = supplied
        .observations
        .coverage
        .iter()
        .position(|c| c.query.operation == ReadOperationV1::DescribeImages)
        .unwrap();
    let query = supplied.observations.coverage.remove(removed).query;
    let partial = ingest(&inputs, supplied).unwrap();
    let missing = derive_offline(&inputs, &partial, &complete.execution).unwrap();
    assert!(!missing.complete);
    assert!(missing.accounting_gap);
    assert!(
        missing
            .execution
            .values()
            .any(|s| *s == WorkStatus::Missing)
    );
    let mut facts = complete.execution.clone();
    facts.rejected_before_admission.push(RejectedRead {
        query: crate::provider::reviewed_subnet_routes_v1::DiscoveryQuery::Existing(query),
        reason: ReadFailureV1::Unsupported,
    });
    let rejected = derive_offline(&inputs, &partial, &facts).unwrap();
    assert!(
        rejected
            .execution
            .values()
            .any(|s| matches!(s, WorkStatus::RejectedBeforeAdmission(_)))
    );
    assert!(derive_offline(&inputs, &complete.evidence, &facts).is_err());
    for field in 0..5 {
        let mut invalid = complete.execution.clone();
        match field {
            0 => invalid.accounting.requests = 0,
            1 => invalid.accounting.source_occurrences = 0,
            2 => invalid.accounting.retained_outputs = 0,
            3 => invalid.accounting.normalized_bytes = 0,
            _ => invalid.accounting.failure = Some(crate::provider::limits::LimitKind::Elapsed),
        }
        assert!(derive_offline(&inputs, &complete.evidence, &invalid).is_err());
    }
    // Conservative byte reservations are deliberately larger than retained encodings.
    let mut surplus = complete.execution.clone();
    surplus.accounting.normalized_bytes += 10;
    assert!(
        derive_offline(&inputs, &complete.evidence, &surplus)
            .unwrap()
            .complete
    );
    let mut partial_account = complete.execution.clone();
    partial_account.accounting.retained_outputs += 1;
    assert!(
        !derive_offline(&inputs, &complete.evidence, &partial_account)
            .unwrap()
            .complete
    );
}

#[test]
fn prior_identity_cycles_and_bounded_frontiers_are_order_independent() {
    use crate::provider::context_v3::*;
    use crate::provider::discovery::relationships::AllocationIdentity as I;
    let (p, c) = retained();
    for count in [2, 100, 1000] {
        let state: crate::identity::ProviderStateDigest = "a".repeat(64).parse().unwrap();
        let mut fields = c.fields().clone();
        fields.prior_provider = Some(PriorProviderIdentityV3 {
            bound_instance: Some("i-aaaaaaaa".parse().unwrap()),
            sticky_conflict: true,
            state: state.clone(),
            evidence: Vec::new(),
        });
        let c = ReconciliationContextV3::from_fields(fields).unwrap();
        let resources: Vec<_> = (0..count)
            .map(|n| I::Instance(format!("i-{n:08x}").parse().unwrap()))
            .collect();
        let mut prior = PriorDiscoveryFacts {
            state,
            relationships: vec![
                (resources[0].clone(), resources[1].clone()),
                (resources[1].clone(), resources[0].clone()),
            ],
            resources,
            fully_supplied: true,
        };
        let evidence = DiscoveryEvidence {
            observations: ProviderObservationV5 {
                context: c.identity().unwrap(),
                records: Vec::new(),
                coverage: Vec::new(),
            },
            reviewed_subnet_routes: None,
        };
        let facts = DiscoveryExecutionFacts {
            accounting: AccountingSnapshot {
                requests: 0,
                source_occurrences: 0,
                retained_outputs: 0,
                response_bytes: 0,
                normalized_bytes: 0,
                failure: None,
            },
            rejected_before_admission: Vec::new(),
            stop: CoordinatorStop::Quiescent,
            final_check: FinalRoundCheck::Passed,
        };
        let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, Some(&prior)).unwrap();
        let valid = ingest(&inputs, evidence.clone()).unwrap();
        let report = derive_offline(&inputs, &valid, &facts).unwrap();
        assert!(!report.complete);
        assert_eq!(report.metadata_exhausted, count > 2);
        assert!(report.required.reads.len() <= 128);
        prior.resources.reverse();
        prior.relationships.reverse();
        let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, Some(&prior)).unwrap();
        let valid = ingest(&inputs, evidence).unwrap();
        assert_eq!(report, derive_offline(&inputs, &valid, &facts).unwrap());
    }
}

#[tokio::test]
async fn combined_families_obey_request_and_byte_limits_before_indexing() {
    use crate::provider::{limits::*, reviewed_subnet_routes_v1::*};
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let body = crate::aws::ec2_infrastructure_reads_tests::fixture(
        crate::aws::ec2_infrastructure_reads::InfrastructureRead::RouteTables,
    );
    let (mut session, _, _) = reader(vec![response(200, &body)]);
    let route = session.reviewed_subnet_routes().await.unwrap();
    let routes = ReviewedSubnetRouteEvidenceV1 {
        context: c.identity().unwrap(),
        records: route.records,
        coverage: route.coverage,
    };
    let coverage = (0..128)
        .map(|n| ReadCoverageV1 {
            query: QueryIdentityV1 {
                operation: ReadOperationV1::DescribeInstances,
                account: "111111111111".parse().unwrap(),
                region: "eu-central-1".parse().unwrap(),
                scope: QueryScopeV1::ClientToken {
                    token: format!("{n:064x}").try_into().unwrap(),
                },
            },
            required: true,
            requests: 1,
            pages: 1,
            records: 0,
            terminal_page: true,
            status: CoverageStatus::Complete,
        })
        .collect();
    let old = ProviderObservationV5 {
        context: c.identity().unwrap(),
        records: Vec::new(),
        coverage,
    };
    old.validate().unwrap();
    assert!(
        ingest(
            &inputs,
            DiscoveryEvidence {
                observations: old,
                reviewed_subnet_routes: Some(routes.clone())
            }
        )
        .is_err()
    );
    // Each carrier independently fits. Their shared byte ledger does not.
    let body = crate::aws::ec2_infrastructure_reads_tests::fixture(
        crate::aws::ec2_infrastructure_reads::InfrastructureRead::Regions,
    );
    let (mut session, _, _) = reader(vec![response(200, &body)]);
    let mut region = session
        .infrastructure(
            crate::aws::ec2_infrastructure_reads::InfrastructureRead::Regions,
            true,
        )
        .await
        .unwrap();
    let ObservationEntryV5::V4(record) = &mut region.records[0] else {
        panic!()
    };
    if let crate::provider::ec2_observation_v4::ObservationDataV4::Region {
        opt_in_status, ..
    } = &mut record.data
    {
        *opt_in_status = crate::provider::ec2_observation_v4::Ec2MemberV4::Present(
            "x".repeat(2000).try_into().unwrap(),
        );
    }
    let bytes = region.records[0].canonical_bytes().unwrap().len();
    let copies = (NORMALIZED_BYTES as usize - 2048) / bytes;
    region.records = vec![region.records[0].clone(); copies];
    region.coverage.records = copies as u64;
    let old = ProviderObservationV5 {
        context: c.identity().unwrap(),
        records: region.records,
        coverage: vec![region.coverage],
    };
    old.validate().unwrap();
    assert!(
        ingest(
            &inputs,
            DiscoveryEvidence {
                observations: old.clone(),
                reviewed_subnet_routes: None
            }
        )
        .is_ok()
    );
    let mut routes = routes;
    let source_count = routes.coverage.records;
    routes.records = (0..8).flat_map(|_| routes.records.clone()).collect();
    routes.coverage.records = source_count * 8;
    let route_only = DiscoveryEvidence {
        observations: ProviderObservationV5 {
            context: c.identity().unwrap(),
            records: Vec::new(),
            coverage: Vec::new(),
        },
        reviewed_subnet_routes: Some(routes.clone()),
    };
    assert!(ingest(&inputs, route_only).is_ok());
    assert!(
        ingest(
            &inputs,
            DiscoveryEvidence {
                observations: old,
                reviewed_subnet_routes: Some(routes)
            }
        )
        .is_err()
    );
}

#[tokio::test]
async fn representation_scratch_exhaustion_is_explicit_and_permutation_stable() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let roots = "<item><instancesSet/></item>".repeat(400);
    let (mut session, _, round) = reader(vec![response(
        200,
        &xml(ReadOperationV1::DescribeInstances, &roots),
    )]);
    let result = session
        .allocation(query(ReadOperationV1::DescribeInstances), true)
        .await
        .unwrap();
    assert_eq!(result.coverage.records, 400);
    assert_eq!(result.records.len(), 400);
    assert_eq!(result.coverage.status, CoverageStatus::Complete);
    let mut supplied = bundle(c.identity().unwrap(), result);
    let valid = ingest(&inputs, supplied.clone()).unwrap();
    let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
    assert!(report.metadata_exhausted);
    assert!(!report.complete);
    assert!(report.representation.values().any(|a| {
        a.gaps
            .contains(&representation::RepresentationGap::AuditBudgetExhausted)
    }));
    assert_eq!(valid.evidence().observations.records.len(), 400);
    supplied.observations.records.reverse();
    let permuted = ingest(&inputs, supplied).unwrap();
    assert_eq!(
        report,
        derive_offline(&inputs, &permuted, &facts(&round)).unwrap()
    );
}

#[tokio::test]
async fn endpoint_representation_uses_scanner_work_and_legacy_duplicates_are_not_sets() {
    use crate::aws::ec2_infrastructure_reads::InfrastructureRead as I;
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for (policy, expected, verifiable) in [
        ("{}", 2, true),
        (r#"{"Version":"2012-10-17"}"#, 4, true),
        (
            r#"{"Statement":{"Effect":"Allow","Action":["s3:GetObject"],"Resource":"*","Principal":{"AWS":["x"]},"Condition":{"Bool":{"aws:SecureTransport":true}}}}"#,
            22,
            true,
        ),
        (r#"{"Unknown":{"x":[1,2]}}"#, 8, false),
    ] {
        let body = format!(
            "<DescribeVpcEndpointsResponse><vpcEndpointSet><item><policyDocument>{policy}</policyDocument></item></vpcEndpointSet></DescribeVpcEndpointsResponse>"
        );
        let (mut session, _, round) = reader(vec![response(200, &body)]);
        let result = session.infrastructure(I::VpcEndpoints, true).await.unwrap();
        assert_eq!(result.coverage.records, expected);
        let valid = ingest(&inputs, bundle(c.identity().unwrap(), result)).unwrap();
        let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
        assert_eq!(
            report.representation.values().all(|a| a.complete()),
            verifiable
        );
        if !verifiable {
            assert!(
                report.representation.values().any(|a| a.gaps.contains(
                    &representation::RepresentationGap::UnverifiableSourceRepresentation
                ))
            );
        }
    }
    let body = "<DescribeRegionsResponse><regionInfo><item><regionName>eu-central-1</regionName></item><item><regionName>eu-central-1</regionName></item></regionInfo></DescribeRegionsResponse>";
    let (mut session, _, round) = reader(vec![response(200, body)]);
    let result = session.infrastructure(I::Regions, true).await.unwrap();
    assert_eq!(result.coverage.records, 2);
    assert_eq!(result.records.len(), 2);
    let mut supplied = bundle(c.identity().unwrap(), result);
    let valid = ingest(&inputs, supplied.clone()).unwrap();
    assert!(
        derive_offline(&inputs, &valid, &facts(&round))
            .unwrap()
            .representation
            .values()
            .all(|a| a.complete())
    );
    supplied.observations.records.pop();
    supplied.observations.validate().unwrap();
    let partial = ingest(&inputs, supplied).unwrap();
    assert!(
        derive_offline(&inputs, &partial, &facts(&round))
            .unwrap()
            .representation
            .values()
            .any(|a| !a.complete())
    );
}

#[tokio::test]
async fn unavailable_extracted_collection_does_not_imply_empty_relationships() {
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let body = xml(
        ReadOperationV1::DescribeInstances,
        "<item><reservationId>r-a</reservationId></item>",
    );
    let (mut session, _, round) = reader(vec![response(200, &body)]);
    let result = session
        .allocation(query(ReadOperationV1::DescribeInstances), true)
        .await
        .unwrap();
    assert_eq!(
        result.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed)
    );
    assert_eq!(result.coverage.records, 1);
    let valid = ingest(&inputs, bundle(c.identity().unwrap(), result)).unwrap();
    let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
    assert!(report.representation.values().all(|a| a.complete()));
    assert!(report.relationships.unresolved_references);
    assert!(!report.complete);
}

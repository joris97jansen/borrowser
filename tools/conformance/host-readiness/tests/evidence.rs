//! Synthetic format specimens only; never native host-readiness evidence.
use borrowser_host_readiness::{Sha256, evidence::*, report::HostProbe};
fn source(commit: &str) -> SourceIdentity {
    SourceIdentity {
        commit: commit.into(),
        source: "artifact".into(),
        lockfile: "artifact".into(),
        clean_checkout: "artifact".into(),
        review: "artifact".into(),
        build: "artifact".into(),
        executable: "artifact".into(),
    }
}
fn fixture() -> HostReadinessManifest {
    use ProofAuthority::*;
    use Property::*;
    let mut probes = vec![];
    for (id, selection, gate, properties) in [
        (
            "inventory-platform",
            PLATFORM_INVENTORY,
            1,
            vec![NativePlatform, ImageSnapshotKernel],
        ),
        (
            "inventory-security",
            SECURITY_INVENTORY,
            2,
            vec![UnprivilegedHost, SecurityPolicy, Resources],
        ),
        (
            "inventory-tooling",
            TOOLING_INVENTORY,
            3,
            vec![OfflineExecution, FrozenSourcesTools],
        ),
        ("review-10", "evidence-review", 10, vec![]),
        ("review-11", "host-ready-review", 11, vec![]),
    ] {
        probes.push(ProbeDefinition {
            id: id.into(),
            authority: IndependentHostInventory,
            gate,
            properties,
            requirement: Requirement::Production,
            exact_selection: selection.into(),
            execution: ExecutionKind::RecordedReview,
        });
    }
    for (id, selection, authority, gate, properties, count) in [
        (
            "helper-descriptor",
            HELPER_DESCRIPTOR_TEST,
            PinnedHelperDeterministic,
            5,
            vec![HelperDescriptorImplementation],
            1,
        ),
        (
            "helper-smoke",
            HELPER_NAMESPACE_TEST,
            PinnedHelperNamespaceSmoke,
            6,
            vec![HelperNamespaceSmoke],
            1,
        ),
        (
            "helper-suite",
            HELPER_SUITE,
            PinnedHelperDeterministic,
            5,
            vec![
                PrivateIpc,
                PidfdSignalsReaping,
                HelperDescriptorImplementation,
            ],
            42,
        ),
        (
            "collector-suite",
            COLLECTOR_SUITE,
            FrozenCollectorRuntime,
            9,
            vec![PidfdSignalsReaping, RetainedExecutable, CollectorRuntime],
            24,
        ),
        (
            "prelaunch",
            COLLECTOR_PRELAUNCH,
            FrozenCollectorRuntime,
            8,
            vec![CollectorPrelaunch],
            1,
        ),
    ] {
        probes.push(ProbeDefinition {
            id: id.into(),
            authority,
            gate,
            properties,
            requirement: Requirement::Production,
            exact_selection: selection.into(),
            execution: ExecutionKind::Tests {
                expected_top_level_tests: count,
            },
        });
    }
    for probe in HostProbe::ALL {
        probes.push(ProbeDefinition {
            id: probe.command().into(),
            authority: HostPrerequisiteOnly,
            gate: host_gate(probe),
            properties: host_properties(probe).to_vec(),
            requirement: Requirement::Production,
            exact_selection: probe.command().into(),
            execution: ExecutionKind::HostCommand,
        });
    }
    for (i, selection) in FROZEN_RUNTIME_TESTS.iter().enumerate() {
        probes.push(ProbeDefinition {
            id: format!("runtime-{i}"),
            authority: FrozenCollectorRuntime,
            gate: 9,
            properties: regression_properties(selection).unwrap().to_vec(),
            requirement: Requirement::FrozenRegression { test_only: None },
            exact_selection: (*selection).into(),
            execution: ExecutionKind::Tests {
                expected_top_level_tests: 1,
            },
        });
    }
    probes.sort_by(|a, b| a.id.cmp(&b.id));
    let gates = GATES
        .iter()
        .enumerate()
        .map(|(i, id)| Gate {
            ordinal: i as u32 + 1,
            id: (*id).into(),
            proofs: probes
                .iter()
                .filter(|p| p.gate == i as u32 + 1)
                .map(|p| p.id.clone())
                .collect(),
        })
        .collect();
    let results = probes
        .iter()
        .enumerate()
        .map(|(i, p)| ProbeResult {
            probe: p.id.clone(),
            execution_ordinal: p.gate * 100 + i as u32,
            command: "artifact".into(),
            raw_result: "artifact".into(),
            outcome: Outcome::RequiredExecutedPass {
                execution: match p.execution {
                    ExecutionKind::Tests {
                        expected_top_level_tests,
                    } => ExecutionEvidence::Tests {
                        executed_tests: expected_top_level_tests,
                        exit_status: 0,
                    },
                    ExecutionKind::HostCommand => ExecutionEvidence::HostCommand {
                        invocations: 1,
                        exit_status: 0,
                    },
                    ExecutionKind::RecordedReview => {
                        ExecutionEvidence::RecordedReview { accepted: true }
                    }
                },
            },
        })
        .collect();
    HostReadinessManifest {
        format: FORMAT.into(),
        authority: AUTHORITY.into(),
        run_id: "synthetic-format-only".into(),
        sources: Sources {
            helper: source(HELPER),
            collector: source(COLLECTOR),
            host_probes: source(&"1".repeat(40)),
            collector_manifests: "artifact".into(),
        },
        environment: Environment {
            platform_provenance: "artifact".into(),
            image_snapshot: "artifact".into(),
            kernel: "artifact".into(),
            security: "artifact".into(),
            tools: "artifact".into(),
            libraries: "artifact".into(),
            resources: "artifact".into(),
            offline: "artifact".into(),
        },
        artifacts: vec![ArtifactRef {
            id: "artifact".into(),
            path: "raw/data".into(),
            byte_length: 3,
            sha256: format!("{:x}", Sha256::digest(b"abc")),
        }],
        probes,
        gates,
        attempts: vec![Attempt {
            ordinal: 1,
            id: "attempt-1".into(),
            observed_at: Some("2026-09-17T10:00:00Z".into()),
            results,
        }],
        selected_attempt: 1,
        review: "artifact".into(),
    }
}
fn applicability() -> HostReadinessManifest {
    let mut m = fixture();
    m.probes.iter_mut().find(|p|p.id=="runtime-3").unwrap().requirement=Requirement::FrozenRegression{test_only:Some(TestOnlyPrerequisite{id:"namespaced-ns-last-pid".into(),frozen_source:"artifact".into(),source_location:"crates/external_browser_capture/src/isolation/population.rs and frozen capture contract: test-only ns_last_pid".into(),explanation:"Synthetic absence record for format testing; no host measured".into()})};
    m.attempts[0].results.iter_mut().find(|r|r.probe=="runtime-3").unwrap().outcome=Outcome::NotApplicableTestOnlyPrerequisite{
        prerequisite:"namespaced-ns-last-pid".into(),measured_absence:"artifact".into(),raw_evidence:"artifact".into(),alternatives:vec![
            AlternativeProof{property:Property::PidfdSignalsReaping,proof:HostProbe::ProcessInspection.command().into()},
            AlternativeProof{property:Property::ProcessInspection,proof:HostProbe::ProcessInspection.command().into()},
        ],unavailable_coverage:"Forced PID reuse and exact quiescence algorithm scenarios unexecuted; fixture is synthetic, not equivalent runtime coverage".into(),review:"artifact".into()};
    m
}
fn golden_fixture() -> HostReadinessManifest {
    let mut m = applicability();
    let mut failed = m.attempts[0].clone();
    failed.results.sort_by_key(|r| r.execution_ordinal);
    failed.results.truncate(1);
    failed.observed_at = None;
    failed.results[0].outcome = Outcome::Failed {
        reason: "Synthetic historical failure; no qualification performed".into(),
    };
    m.attempts[0].id = "attempt-2".into();
    m.attempts[0].ordinal = 2;
    m.selected_attempt = 2;
    m.attempts.insert(0, failed);
    m
}

#[test]
fn canonical_roundtrip_bytes_and_grammar() {
    let m = fixture();
    let bytes = m.canonical().unwrap();
    assert!(bytes.starts_with(
        format!("{{\"format\":\"{FORMAT}\",\"authority\":\"{AUTHORITY}\",\"run_id\":").as_bytes()
    ));
    assert!(bytes.ends_with(b"}\n"));
    assert_eq!(
        HostReadinessManifest::parse(&bytes)
            .unwrap()
            .canonical()
            .unwrap(),
        bytes
    );
    for bad in [
        bytes[..bytes.len() - 1].to_vec(),
        [b"\xef\xbb\xbf".as_slice(), &bytes].concat(),
        [b" ".as_slice(), &bytes].concat(),
        [bytes.as_slice(), b"\n"].concat(),
    ] {
        assert!(HostReadinessManifest::parse(&bad).is_err());
    }
    let raw = String::from_utf8(bytes).unwrap();
    for bad in [
        raw.replacen('{', "{\"unknown\":0,", 1),
        raw.replacen('{', "{\"run_id\":\"duplicate\",", 1),
        raw.replace("\"byte_length\":3", "\"byte_length\":3,\"byte_length\":3"),
        raw.replace("\"byte_length\":3", "\"byte_length\":3.0"),
        raw.replace("\"byte_length\":3", "\"byte_length\":-1"),
        raw.replace("\"byte_length\":3", "\"byte_length\":18446744073709551616"),
    ] {
        assert!(HostReadinessManifest::parse(bad.as_bytes()).is_err());
    }
}
#[test]
fn duplicate_identity_path_ordinal_and_order_fail() {
    let mut m = fixture();
    m.artifacts.push(m.artifacts[0].clone());
    assert!(m.validate().is_err());
    m = fixture();
    let mut a = m.artifacts[0].clone();
    a.path = "raw/other".into();
    m.artifacts.push(a);
    assert!(m.validate().is_err());
    m = fixture();
    m.probes.swap(0, 1);
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts.push(m.attempts[0].clone());
    assert!(m.validate().is_err());
    m = fixture();
    m.gates.swap(0, 1);
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts[0].results.swap(0, 1);
    assert!(m.validate().is_err());
    m = fixture();
    let p = m
        .probes
        .iter_mut()
        .find(|p| !p.properties.is_empty())
        .unwrap();
    p.properties.push(p.properties[0]);
    assert!(m.validate().is_err());
}
#[test]
fn bounded_populations_and_arithmetic() {
    assert_eq!(total_lengths([ARTIFACT_BYTES; 16]).unwrap(), TOTAL_BYTES);
    for lengths in [
        vec![ARTIFACT_BYTES + 1],
        vec![ARTIFACT_BYTES; 17],
        vec![u64::MAX, 1],
    ] {
        assert!(total_lengths(lengths).is_err());
    }
    let mut m = fixture();
    m.artifacts = vec![m.artifacts[0].clone(); ARTIFACTS + 1];
    assert!(m.validate().is_err());
    m = fixture();
    m.probes = vec![m.probes[0].clone(); DEFINITIONS + 1];
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts = vec![m.attempts[0].clone(); ATTEMPTS + 1];
    assert!(m.validate().is_err());
    m = fixture();
    m.gates = vec![m.gates[0].clone(); 33];
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts[0].results = vec![m.attempts[0].results[0].clone(); RESULTS + 1];
    assert!(m.validate().is_err());
    assert!(HostReadinessManifest::parse(&vec![b' '; MANIFEST_BYTES + 1]).is_err());
    assert!(identifier(&"a".repeat(128)).is_ok());
    assert!(identifier(&"a".repeat(129)).is_err());
    assert!(description(&"a".repeat(1024)).is_ok());
    assert!(description(&"a".repeat(1025)).is_err());
    assert!(description("a\n").is_err());
    assert!(artifact_path(&format!("{}/{}", "a".repeat(64), "b".repeat(64))).is_ok());
    assert!(artifact_path(&"a".repeat(65)).is_err());
    assert!(artifact_path(&vec!["a"; 129].join("/")).is_err());
    m = fixture();
    m.probes[0].execution = ExecutionKind::Tests {
        expected_top_level_tests: 65537,
    };
    assert!(m.validate().is_err());
}
#[test]
fn malformed_digest_path_missing_artifact_and_authority_fail() {
    for path in [
        "", "/abs", "a//b", "a/./b", "a/../b", "../b", "a\\b", "a/", "a b", "é",
    ] {
        assert!(artifact_path(path).is_err(), "{path}");
    }
    for digest in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
    ] {
        assert!(hex(&digest, 64).is_err());
    }
    let mut m = fixture();
    m.authority = "mechanism-go".into();
    assert!(m.validate().is_err());
    m = fixture();
    m.sources.collector.commit = HELPER.into();
    assert!(m.validate().is_err());
    m = fixture();
    m.sources.host_probes.clean_checkout = "missing".into();
    assert!(m.validate().is_err());
    m = fixture();
    m.probes
        .iter_mut()
        .find(|p| {
            p.properties
                .contains(&Property::HelperDescriptorImplementation)
        })
        .unwrap()
        .authority = ProofAuthority::HostPrerequisiteOnly;
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts[0].observed_at = Some("2026-02-30T00:00:00Z".into());
    assert!(m.validate().is_err());
}
#[test]
fn incomplete_failed_zero_tests_and_wrong_selection_reject() {
    let mut m = fixture();
    m.attempts[0].results.pop();
    assert!(m.validate().is_err());
    m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.properties.contains(&Property::NativePlatform))
        .unwrap()
        .properties
        .clear();
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts[0].results[0].outcome = Outcome::Failed {
        reason: "unsupported host".into(),
    };
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts[0].results[0].outcome = Outcome::RequiredExecutedPass {
        execution: ExecutionEvidence::Tests {
            executed_tests: 0,
            exit_status: 0,
        },
    };
    assert!(m.validate().is_err());
    m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.properties.contains(&Property::CollectorPrelaunch))
        .unwrap()
        .exact_selection = "other".into();
    assert!(m.validate().is_err());
}
#[test]
fn applicability_requires_test_only_authority_measurement_and_actual_alternatives() {
    assert!(applicability().canonical().is_ok());
    let mut m = applicability();
    m.probes
        .iter_mut()
        .find(|p| p.id == "runtime-3")
        .unwrap()
        .requirement = Requirement::Production;
    assert!(m.validate().is_err());
    m = applicability();
    m.probes
        .iter_mut()
        .find(|p| p.id == "runtime-3")
        .unwrap()
        .requirement = Requirement::FrozenRegression { test_only: None };
    assert!(m.validate().is_err());
    m = applicability();
    if let Outcome::NotApplicableTestOnlyPrerequisite { alternatives, .. } = &mut m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "runtime-3")
        .unwrap()
        .outcome
    {
        alternatives.clear();
    }
    assert!(m.validate().is_err());
    m = applicability();
    if let Outcome::NotApplicableTestOnlyPrerequisite {
        measured_absence, ..
    } = &mut m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "runtime-3")
        .unwrap()
        .outcome
    {
        *measured_absence = "missing".into();
    }
    assert!(m.validate().is_err());
    m = applicability();
    if let Outcome::NotApplicableTestOnlyPrerequisite { alternatives, .. } = &mut m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "runtime-3")
        .unwrap()
        .outcome
    {
        alternatives[0].proof = "runtime-3".into();
    }
    assert!(m.validate().is_err());
    m = applicability();
    if let Outcome::NotApplicableTestOnlyPrerequisite {
        unavailable_coverage,
        ..
    } = &mut m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "runtime-3")
        .unwrap()
        .outcome
    {
        unavailable_coverage.clear();
    }
    assert!(m.validate().is_err());
}
#[test]
fn failed_attempts_are_retained_but_cannot_be_selected_or_hide_later_failure() {
    let mut m = fixture();
    let mut failed = m.attempts[0].clone();
    failed.results.sort_by_key(|r| r.execution_ordinal);
    failed.results.truncate(1);
    failed.results[0].outcome = Outcome::Failed {
        reason: "observed failure".into(),
    };
    m.attempts[0].ordinal = 2;
    m.attempts[0].id = "attempt-2".into();
    m.selected_attempt = 2;
    m.attempts.insert(0, failed);
    assert!(m.validate().is_ok());
    m.selected_attempt = 1;
    assert!(m.validate().is_err());
}
#[cfg(unix)]
#[test]
fn external_artifacts_must_actually_exist_and_match() {
    let m = fixture();
    let d = tempfile::tempdir().unwrap();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    assert!(borrowser_host_readiness::artifacts::verify(d.path(), &m, end).is_err());
    std::fs::create_dir(d.path().join("raw")).unwrap();
    std::fs::write(d.path().join("raw/data"), b"abc").unwrap();
    assert!(borrowser_host_readiness::artifacts::verify(d.path(), &m, end).is_ok());
    std::fs::write(d.path().join("raw/data"), b"abd").unwrap();
    assert!(borrowser_host_readiness::artifacts::verify(d.path(), &m, end).is_err());
}

#[test]
fn required_inventory_and_execution_order_cannot_be_omitted() {
    let mut m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.exact_selection == HELPER_NAMESPACE_TEST)
        .unwrap()
        .exact_selection = "wrong-smoke".into();
    assert!(m.validate().is_err());
    m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.exact_selection == FROZEN_RUNTIME_TESTS[0])
        .unwrap()
        .exact_selection = "other".into();
    assert!(m.validate().is_err());
    m = fixture();
    m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "review-11")
        .unwrap()
        .execution_ordinal = 1;
    assert!(m.validate().is_err());
    m = fixture();
    let ordinal = m.attempts[0].results[0].execution_ordinal;
    m.attempts[0].results[1].execution_ordinal = ordinal;
    assert!(m.validate().is_err());
    m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.exact_selection == FROZEN_RUNTIME_TESTS[0])
        .unwrap()
        .properties
        .clear();
    assert!(m.validate().is_err());
    assert!(
        artifact_path(
            &[
                "a".repeat(64),
                "a".repeat(64),
                "a".repeat(64),
                "a".repeat(61)
            ]
            .join("/")
        )
        .is_ok()
    );
}

#[test]
fn reviewed_synthetic_v1_golden_is_exact() {
    // Never update this fixture from a test. Incompatible changes need version review.
    let expected = include_bytes!("fixtures/synthetic-host-readiness-v1.golden.json");
    assert_eq!(golden_fixture().canonical().unwrap(), expected);
    assert_eq!(
        HostReadinessManifest::parse(expected)
            .unwrap()
            .canonical()
            .unwrap(),
        expected
    );
}

#[test]
fn every_property_rejects_wrong_authority_gate_and_selector() {
    let original = fixture();
    for property in PROPERTIES {
        let index = original
            .probes
            .iter()
            .position(|p| p.properties.contains(property))
            .unwrap();
        let mut m = original.clone();
        m.probes[index].authority =
            if m.probes[index].authority == ProofAuthority::IndependentHostInventory {
                ProofAuthority::HostPrerequisiteOnly
            } else {
                ProofAuthority::IndependentHostInventory
            };
        assert!(
            !property.allows(&m.probes[index]),
            "authority: {property:?}"
        );
        assert!(m.validate().is_err());
        let mut m = original.clone();
        m.probes[index].gate = 11;
        assert!(!property.allows(&m.probes[index]), "gate: {property:?}");
        assert!(m.validate().is_err());
        let mut m = original.clone();
        m.probes[index].exact_selection = "authored-substitute".into();
        assert!(!property.allows(&m.probes[index]), "selector: {property:?}");
        assert!(m.validate().is_err());
    }
}
#[test]
fn mandatory_host_commands_cannot_be_missing_duplicated_or_relabelled() {
    for probe in HostProbe::ALL {
        let mut m = fixture();
        let id = probe.command();
        m.probes.retain(|p| p.id != id);
        for g in &mut m.gates {
            g.proofs.retain(|p| p != id);
        }
        m.attempts[0].results.retain(|r| r.probe != id);
        assert!(m.validate().is_err(), "omitted {id}");
        let mut m = fixture();
        let mut p = m.probes.iter().find(|p| p.id == id).unwrap().clone();
        p.id = "duplicate-host-command".into();
        m.gates[(p.gate - 1) as usize].proofs.push(p.id.clone());
        m.gates[(p.gate - 1) as usize].proofs.sort();
        let mut duplicate_result = m.attempts[0]
            .results
            .iter()
            .find(|r| r.probe == id)
            .unwrap()
            .clone();
        duplicate_result.probe = p.id.clone();
        duplicate_result.execution_ordinal = p.gate * 100 + 99;
        m.attempts[0].results.push(duplicate_result);
        m.attempts[0].results.sort_by(|a, b| a.probe.cmp(&b.probe));
        m.probes.push(p);
        m.probes.sort_by(|a, b| a.id.cmp(&b.id));
        assert!(
            m.validate()
                .unwrap_err()
                .to_string()
                .contains("mandatory host command")
        );
        let mut m = fixture();
        let p = m.probes.iter_mut().find(|p| p.id == id).unwrap();
        p.exact_selection = "author-invented-probe".into();
        assert!(m.validate().is_err());
        let mut m = fixture();
        let p = m.probes.iter_mut().find(|p| p.id == id).unwrap();
        p.properties.push(Property::CollectorRuntime);
        p.properties.sort();
        assert!(m.validate().is_err());
        let mut m = fixture();
        m.probes
            .iter_mut()
            .find(|p| p.id == id)
            .unwrap()
            .properties
            .clear();
        assert!(m.validate().is_err());
    }
}
#[test]
fn typed_execution_cannot_turn_zero_tests_or_reviews_into_runtime_proof() {
    for probe in HostProbe::ALL {
        for execution in [
            ExecutionEvidence::Tests {
                executed_tests: 0,
                exit_status: 0,
            },
            ExecutionEvidence::Tests {
                executed_tests: 1,
                exit_status: 0,
            },
            ExecutionEvidence::HostCommand {
                invocations: 0,
                exit_status: 0,
            },
            ExecutionEvidence::HostCommand {
                invocations: 2,
                exit_status: 0,
            },
            ExecutionEvidence::HostCommand {
                invocations: 1,
                exit_status: 1,
            },
            ExecutionEvidence::RecordedReview { accepted: true },
        ] {
            let mut m = fixture();
            m.attempts[0]
                .results
                .iter_mut()
                .find(|r| r.probe == probe.command())
                .unwrap()
                .outcome = Outcome::RequiredExecutedPass { execution };
            assert!(m.validate().is_err(), "{probe:?}: {execution:?}");
        }
        for execution in [
            ExecutionKind::Tests {
                expected_top_level_tests: 0,
            },
            ExecutionKind::Tests {
                expected_top_level_tests: 1,
            },
            ExecutionKind::RecordedReview,
        ] {
            let mut m = fixture();
            m.probes
                .iter_mut()
                .find(|p| p.id == probe.command())
                .unwrap()
                .execution = execution;
            assert!(m.validate().is_err());
        }
    }
    let mut m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.id == "helper-suite")
        .unwrap()
        .execution = ExecutionKind::Tests {
        expected_top_level_tests: 0,
    };
    assert!(m.validate().is_err());
    let mut m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.id == "review-11")
        .unwrap()
        .properties
        .push(Property::NetworkClosure);
    assert!(m.validate().is_err());
    let mut m = fixture();
    m.probes
        .iter_mut()
        .find(|p| p.id == "inventory-platform")
        .unwrap()
        .execution = ExecutionKind::HostCommand;
    assert!(m.validate().is_err());
    let mut m = fixture();
    m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "review-11")
        .unwrap()
        .outcome = Outcome::RequiredExecutedPass {
        execution: ExecutionEvidence::RecordedReview { accepted: false },
    };
    assert!(m.validate().is_err());
}
#[test]
fn applicability_alternatives_must_pass_the_same_property_and_execution_policies() {
    let mut m = applicability();
    let p = m
        .probes
        .iter_mut()
        .find(|p| p.id == HostProbe::ProcessInspection.command())
        .unwrap();
    p.authority = ProofAuthority::IndependentHostInventory;
    p.execution = ExecutionKind::RecordedReview;
    assert!(m.validate().is_err());
    let mut m = applicability();
    m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == HostProbe::ProcessInspection.command())
        .unwrap()
        .outcome = Outcome::RequiredExecutedPass {
        execution: ExecutionEvidence::HostCommand {
            invocations: 0,
            exit_status: 0,
        },
    };
    assert!(m.validate().is_err());
    assert!(applicability().validate().is_ok());
    let mut m = applicability();
    if let Outcome::NotApplicableTestOnlyPrerequisite { alternatives, .. } = &mut m.attempts[0]
        .results
        .iter_mut()
        .find(|r| r.probe == "runtime-3")
        .unwrap()
        .outcome
    {
        alternatives[0].proof = "helper-suite".into();
    }
    assert!(
        m.validate().is_ok(),
        "pidfd evidence may come from the pinned helper suite while process inspection remains host-only"
    );
}

use std::fs;
use std::path::{Path, PathBuf};

use conformance_test_support::{
    DerivedAdaptationDecision, EngineCapabilityKind, EnvironmentRequirementKind,
    GenericAssertionRequirement, GenericHarnessRequirement, GenericResourceRequirement,
    RequirementTag, SourceSelectionDecision, load_external_assessment_profile,
};
use wpt_test_support::{
    WPT_IMPORT_SUMMARY_PATH, WptAutomationRequirement, WptReadinessRequirement,
    WptReferenceRelation, WptRegistryError, WptServerRequirement, WptSourceForm, WptSummaryCheck,
    account_wpt_source_set, check_repository_wpt_summary, generate_repository_wpt_summary,
    interpret_wpt_source_set, load_wpt_selection_policy, load_wpt_source_metadata,
    load_wpt_source_set, materialize_wpt_source_set, serialize_wpt_summary,
    update_repository_wpt_summary, validate_materialized_sources,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn copy_inputs(destination: &Path) {
    let source = repository_root();
    for relative in [
        "tests/conformance/external/wpt/sources.toml",
        "tests/conformance/external/wpt/source-metadata.toml",
        "tests/conformance/external/wpt/selection-policy.toml",
        "tests/conformance/external/wpt/accounting-summary.toml",
        "tests/conformance/external/assessment-profile.toml",
        "tests/conformance/external/registries.toml",
        "tests/wpt/external/LICENSE-3-Clause.txt",
        "docs/conformance/ag1-conformance-harness-architecture-no-js-scope.md",
        "docs/conformance/ag7-static-structural-reference-comparison.md",
        "docs/conformance/ag8-wpt-import-filtering-classification.md",
        "crates/layout/src/box_tree/tests/debug.rs",
        "crates/gfx/src/paint/mod.rs",
        "docs/rendering/w3-display-to-box-generation-behavior.md",
        "crates/rendering_test_support/src/paired_fixture.rs",
        "crates/rendering_test_support/src/execute.rs",
    ] {
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(source.join(relative), target).unwrap();
    }
    let set = load_wpt_source_set(&source).unwrap();
    for file in set.files() {
        let target = destination.join(file.local_path());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(source.join(file.local_path()), target).unwrap();
    }
}

#[test]
fn repository_population_is_exact_and_integrity_checked() {
    let root = repository_root();
    let set = load_wpt_source_set(&root).unwrap();
    assert_eq!(
        set.revision().as_str(),
        "384c6d4cc3eeaef056564c29446bdfe853991da8"
    );
    assert_eq!(set.records().len(), 7);
    assert_eq!(set.files().len(), 13);
    assert_eq!(
        set.files()
            .iter()
            .filter(|file| file.role().as_str() == "accounted-source")
            .count(),
        7
    );
    assert_eq!(
        set.files()
            .iter()
            .filter(|file| file.role().as_str() != "accounted-source")
            .count(),
        6
    );
    validate_materialized_sources(&root, &set).unwrap();
    assert!(
        set.files()
            .iter()
            .all(|file| file.local_path().starts_with(&format!(
                "tests/conformance/external/wpt/raw/{}/",
                set.revision().as_str()
            )))
    );
}

#[test]
fn interpretation_keeps_upstream_requirements_separate_from_borrowser_assessment() {
    let root = repository_root();
    let set = load_wpt_source_set(&root).unwrap();
    let records = interpret(&root, &set);
    let static_reftest = record(&records, "wpt-css-body-background-display-none");
    assert!(
        static_reftest
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::NoJs)
    );
    let harness = record(&records, "wpt-css-background-attachment-valid");
    assert_eq!(harness.source_form(), WptSourceForm::TestHarness);
    assert!(
        harness
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::RequiresJs)
    );
    assert!(
        harness
            .generic_requirements()
            .capabilities()
            .iter()
            .any(|value| value.kind() == EngineCapabilityKind::JavaScriptExecution)
    );
    assert!(
        !harness
            .generic_requirements()
            .harness()
            .iter()
            .any(|value| matches!(value, GenericHarnessRequirement::SourceFormatInterpreter(_)))
    );
    assert_eq!(
        harness.automation_requirements(),
        &[WptAutomationRequirement::TestHarnessJavascript]
    );
    let webdriver = record(&records, "wpt-webdriver-navigator-webdriver-active");
    assert_eq!(webdriver.source_form(), WptSourceForm::WdSpec);
    assert!(
        webdriver
            .generic_requirements()
            .environment()
            .iter()
            .all(|value| value.kind() != EnvironmentRequirementKind::ExternalBrowser)
    );
    assert!(
        webdriver
            .generic_requirements()
            .capabilities()
            .iter()
            .any(
                |value| value.kind() == EngineCapabilityKind::BrowserRuntimeFeature
                    && value
                        .feature()
                        .is_some_and(|feature| feature.as_str() == "webdriver")
            )
    );
    let server = record(&records, "wpt-trusted-types-server-substitution");
    assert_eq!(
        server.server_requirements(),
        &[
            WptServerRequirement::Substitution,
            WptServerRequirement::SpecialOrigins,
            WptServerRequirement::PipesAndHeaders,
        ]
    );
    assert!(
        server
            .generic_requirements()
            .resources()
            .iter()
            .any(|value| matches!(value, GenericResourceRequirement::ServerBehavior { .. }))
    );
    assert!(
        server
            .generic_requirements()
            .resources()
            .iter()
            .any(|value| matches!(value, GenericResourceRequirement::ControlledHttp { .. }))
    );
    assert!(
        server
            .generic_requirements()
            .capabilities()
            .iter()
            .any(|value| value.kind() == EngineCapabilityKind::Networking)
    );
    for (kind, feature) in [
        (EngineCapabilityKind::DomApi, "document-and-iframe-mutation"),
        (
            EngineCapabilityKind::BrowserRuntimeFeature,
            "window-message-events",
        ),
        (
            EngineCapabilityKind::BrowserRuntimeFeature,
            "document-navigation",
        ),
        (
            EngineCapabilityKind::BrowserRuntimeFeature,
            "csp-trusted-types",
        ),
    ] {
        assert!(has_capability(server, kind, feature));
    }
}

#[test]
fn reference_graph_preserves_relations_and_dynamic_readiness() {
    let root = repository_root();
    let set = load_wpt_source_set(&root).unwrap();
    let records = interpret(&root, &set);
    let multi = record(&records, "wpt-css-attr-case-sensitivity-multi-reference");
    let edges = multi.reference_graph().unwrap().edges();
    assert_eq!(edges.len(), 2);
    assert!(
        edges
            .iter()
            .any(|edge| edge.relation() == WptReferenceRelation::Match)
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge.relation() == WptReferenceRelation::Mismatch)
    );
    assert!(
        multi
            .generic_requirements()
            .assertions()
            .contains(&GenericAssertionRequirement::MultipleReferenceAssertion)
    );
    let wait = record(&records, "wpt-css-background-clip-reftest-wait");
    assert_eq!(
        wait.readiness_requirements(),
        &[WptReadinessRequirement::ReftestWait]
    );
    assert!(
        wait.generic_requirements()
            .assertions()
            .contains(&GenericAssertionRequirement::DynamicReadiness)
    );
    assert!(has_capability(
        wait,
        EngineCapabilityKind::DomApi,
        "document-style-mutation"
    ));
    assert!(has_capability(
        wait,
        EngineCapabilityKind::BrowserRuntimeFeature,
        "animation-frame-scheduling"
    ));
}

#[test]
fn every_validated_record_has_one_decision_and_all_blockers_remain_visible() {
    let root = repository_root();
    let set = load_wpt_source_set(&root).unwrap();
    let accounted = account(&root, &set);
    assert_eq!(accounted.len(), set.records().len());
    assert_eq!(
        accounted
            .iter()
            .filter(|value| matches!(
                value.generic_accounting().decision(),
                SourceSelectionDecision::SelectedForDirectExecution
            ))
            .count(),
        0
    );
    assert_eq!(
        accounted
            .iter()
            .filter(|value| matches!(
                value.generic_accounting().decision(),
                SourceSelectionDecision::NotSelected
            ))
            .count(),
        7
    );
    assert_eq!(
        accounted
            .iter()
            .flat_map(|record| record.derived_adaptations())
            .filter(|adaptation| adaptation.decision() == &DerivedAdaptationDecision::Selected)
            .count(),
        1
    );
    let wait = accounted
        .iter()
        .find(|value| {
            value.interpreted().source_record_id().as_str()
                == "wpt-css-background-clip-reftest-wait"
        })
        .unwrap()
        .generic_accounting();
    assert!(
        wait.production_assessment()
            .facts()
            .iter()
            .any(|fact| fact.state().as_str() == "unsupported")
    );
    assert!(
        wait.harness_assessment()
            .facts()
            .iter()
            .any(|fact| fact.state().as_str() == "unsupported")
    );
    assert!(
        wait.environment_assessment()
            .facts()
            .iter()
            .any(|fact| fact.state().as_str() == "unsupported")
    );
    assert!(
        wait.representation_assessment()
            .facts()
            .iter()
            .filter(|fact| fact.state().as_str() == "unsupported")
            .count()
            >= 2
    );
}

#[test]
fn filter_policy_accounts_every_required_dimension_before_selection() {
    let root = repository_root();
    let set = load_wpt_source_set(&root).unwrap();
    let accounted = account(&root, &set);
    for record in &accounted {
        let dimensions = record
            .filter_assessment()
            .facts()
            .iter()
            .map(|fact| fact.dimension())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(dimensions.len(), 7);
    }
    let body = accounted
        .iter()
        .find(|record| {
            record.interpreted().source_record_id().as_str()
                == "wpt-css-body-background-display-none"
        })
        .unwrap();
    assert_eq!(
        body.generic_accounting().decision(),
        &SourceSelectionDecision::NotSelected
    );
    assert_eq!(
        body.derived_adaptations()[0].decision(),
        &DerivedAdaptationDecision::Selected
    );
    let webdriver = accounted
        .iter()
        .find(|record| {
            record.interpreted().source_record_id().as_str()
                == "wpt-webdriver-navigator-webdriver-active"
        })
        .unwrap();
    assert_eq!(
        webdriver
            .filter_assessment()
            .facts()
            .iter()
            .find(|fact| {
                fact.dimension() == wpt_test_support::WptFilterDimension::NoJsCompatibility
            })
            .unwrap()
            .outcome(),
        wpt_test_support::WptFilterOutcome::NotYetEstablished
    );
}

#[test]
fn assessment_profile_changes_accounting_without_reinterpreting_wpt() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let policy = load_wpt_selection_policy(temp.path(), &set).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let interpreted = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    let profile = load_external_assessment_profile(temp.path()).unwrap();
    let selected = account_wpt_source_set(&set, &policy, &profile, interpreted.clone()).unwrap();
    assert_eq!(
        selected
            .iter()
            .flat_map(|record| record.derived_adaptations())
            .filter(|adaptation| adaptation.decision() == &DerivedAdaptationDecision::Selected)
            .count(),
        1
    );

    let profile_path = temp
        .path()
        .join("tests/conformance/external/assessment-profile.toml");
    let text = fs::read_to_string(&profile_path).unwrap().replacen(
        "feature = \"display-none-subtree-paint-suppression\"\nstate = \"supported\"",
        "feature = \"display-none-subtree-paint-suppression\"\nstate = \"unsupported\"",
        1,
    );
    fs::write(&profile_path, text).unwrap();
    let changed_profile = load_external_assessment_profile(temp.path()).unwrap();
    let reinterpreted = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    assert_eq!(reinterpreted, interpreted);
    let changed = account_wpt_source_set(&set, &policy, &changed_profile, interpreted).unwrap();
    assert_eq!(
        changed
            .iter()
            .flat_map(|record| record.derived_adaptations())
            .filter(|adaptation| adaptation.decision() == &DerivedAdaptationDecision::Selected)
            .count(),
        0
    );
}

#[test]
fn selection_policy_changes_do_not_change_interpreted_wpt_records() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let before = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    let path = temp
        .path()
        .join("tests/conformance/external/wpt/selection-policy.toml");
    let changed = fs::read_to_string(&path).unwrap().replacen(
        "no_js = \"required\"",
        "no_js = \"allowed\"",
        1,
    );
    fs::write(&path, changed).unwrap();
    load_wpt_selection_policy(temp.path(), &set).unwrap();
    let after = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    assert_eq!(after, before);
}

#[test]
fn selection_policy_schema_is_strict_and_default_deny() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let path = temp
        .path()
        .join("tests/conformance/external/wpt/selection-policy.toml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replacen("format = ", "unknown = true\nformat = ", 1),
    )
    .unwrap();
    assert_eq!(
        load_wpt_selection_policy(temp.path(), &set).unwrap_err(),
        wpt_test_support::WptSelectionPolicyError::InvalidSchema
    );
}

#[test]
fn source_metadata_is_strict_and_changes_only_the_annotated_record() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let before = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    let path = temp
        .path()
        .join("tests/conformance/external/wpt/source-metadata.toml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replacen(
            "capabilities = []",
            "capabilities = [{ kind = \"dom-api\", feature = \"document-inspection\", evidence_kind = \"source-path\", evidence_value = \"css/css-backgrounds/background-color-body-propagation-004.html\" }]",
            1,
        ),
    )
    .unwrap();
    let changed_metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let after = interpret_wpt_source_set(temp.path(), &set, &changed_metadata).unwrap();
    for before_record in &before {
        let after_record = record(&after, before_record.source_record_id().as_str());
        if before_record.source_record_id().as_str() == "wpt-css-body-background-display-none" {
            assert_ne!(after_record, before_record);
            assert!(has_capability(
                after_record,
                EngineCapabilityKind::DomApi,
                "document-inspection"
            ));
        } else {
            assert_eq!(after_record, before_record);
        }
    }

    fs::write(
        &path,
        original.replacen("format = ", "unknown = true\nformat = ", 1),
    )
    .unwrap();
    assert_eq!(
        load_wpt_source_metadata(temp.path(), &set).unwrap_err(),
        wpt_test_support::WptSourceMetadataError::InvalidSchema
    );
}

#[test]
fn canonical_summary_is_exact_current_bytes_and_read_only_check() {
    let root = repository_root();
    let first = generate_repository_wpt_summary(&root).unwrap();
    let second = generate_repository_wpt_summary(&root).unwrap();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert_eq!(
        first.as_bytes(),
        fs::read(root.join(WPT_IMPORT_SUMMARY_PATH)).unwrap()
    );
    let before = fs::read(root.join(WPT_IMPORT_SUMMARY_PATH)).unwrap();
    assert_eq!(
        check_repository_wpt_summary(&root, &first).unwrap(),
        WptSummaryCheck::Current
    );
    assert_eq!(
        fs::read(root.join(WPT_IMPORT_SUMMARY_PATH)).unwrap(),
        before
    );
    let text = std::str::from_utf8(first.as_bytes()).unwrap();
    for forbidden in ["timestamp", "generated_at", "/Users/", "current-host"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn strict_schema_hash_failure_and_stale_update_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let registry = temp
        .path()
        .join("tests/conformance/external/wpt/sources.toml");
    let original = fs::read_to_string(&registry).unwrap();
    fs::write(
        &registry,
        original.replacen("format = ", "unknown = \"x\"\nformat = ", 1),
    )
    .unwrap();
    assert_eq!(
        load_wpt_source_set(temp.path()).unwrap_err(),
        WptRegistryError::InvalidSchema
    );
    fs::write(
        &registry,
        original.replacen(
            "css/css-backgrounds/background-color-body-propagation-004.html",
            "../path-escape.html",
            1,
        ),
    )
    .unwrap();
    assert_eq!(
        load_wpt_source_set(temp.path()).unwrap_err(),
        WptRegistryError::InvalidIdentity
    );
    fs::write(&registry, original).unwrap();
    let set = load_wpt_source_set(temp.path()).unwrap();
    let path = temp.path().join(set.files()[0].local_path());
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        validate_materialized_sources(temp.path(), &set),
        Err(WptRegistryError::HashMismatch)
    );
    copy_inputs(temp.path());
    let summary = generate_repository_wpt_summary(temp.path()).unwrap();
    let output = temp.path().join(WPT_IMPORT_SUMMARY_PATH);
    fs::write(&output, b"stale\n").unwrap();
    assert_eq!(
        check_repository_wpt_summary(temp.path(), &summary).unwrap(),
        WptSummaryCheck::Stale
    );
    assert_eq!(fs::read(&output).unwrap(), b"stale\n");
    update_repository_wpt_summary(temp.path(), &summary).unwrap();
    assert_eq!(
        check_repository_wpt_summary(temp.path(), &summary).unwrap(),
        WptSummaryCheck::Current
    );
}

#[cfg(unix)]
#[test]
fn symlink_and_failed_materialization_do_not_replace_valid_inputs() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let source = temp.path().join(set.files()[0].local_path());
    let original = fs::read(&source).unwrap();
    let outside = temp.path().join("outside");
    fs::write(&outside, &original).unwrap();
    fs::remove_file(&source).unwrap();
    symlink(&outside, &source).unwrap();
    assert_eq!(
        validate_materialized_sources(temp.path(), &set),
        Err(WptRegistryError::Symlink)
    );
    fs::remove_file(&source).unwrap();
    fs::write(&source, &original).unwrap();
    let invalid_checkout = tempfile::tempdir().unwrap();
    assert!(materialize_wpt_source_set(temp.path(), invalid_checkout.path(), &set).is_err());
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn authoritative_wpt_inputs_and_summary_reject_symlinked_parent_directories() {
    for loader in [
        "registry",
        "metadata",
        "policy",
        "summary-check",
        "summary-update",
    ] {
        let temp = tempfile::tempdir().unwrap();
        copy_inputs(temp.path());
        let set = load_wpt_source_set(temp.path()).unwrap();
        let summary = generate_repository_wpt_summary(temp.path()).unwrap();
        symlink_wpt_parent(temp.path());
        match loader {
            "registry" => assert!(load_wpt_source_set(temp.path()).is_err()),
            "metadata" => assert!(load_wpt_source_metadata(temp.path(), &set).is_err()),
            "policy" => assert!(load_wpt_selection_policy(temp.path(), &set).is_err()),
            "summary-check" => {
                assert!(check_repository_wpt_summary(temp.path(), &summary).is_err())
            }
            "summary-update" => {
                assert!(update_repository_wpt_summary(temp.path(), &summary).is_err())
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(unix)]
fn symlink_wpt_parent(root: &Path) {
    use std::os::unix::fs::symlink;
    let parent = root.join("tests/conformance/external/wpt");
    let real = root.join("tests/conformance/external/real-wpt");
    fs::rename(&parent, &real).unwrap();
    symlink(&real, &parent).unwrap();
}

#[test]
fn dependency_boundaries_are_direct_and_generic_ag_never_names_wpt() {
    let root = repository_root();
    let generic_manifest =
        fs::read_to_string(root.join("crates/conformance_test_support/Cargo.toml")).unwrap();
    let generic_sources =
        fs::read_to_string(root.join("crates/conformance_test_support/src/lib.rs")).unwrap()
            + &fs::read_to_string(
                root.join("crates/conformance_test_support/src/external_source.rs"),
            )
            .unwrap();
    assert!(!generic_manifest.contains("wpt-test-support"));
    assert!(!generic_sources.contains("InterpretedWpt"));
    assert!(!generic_sources.contains("WptReference"));
    assert!(!generic_sources.contains("SourceRecordId::parse(lineage_id"));
    let html_manifest =
        fs::read_to_string(root.join("crates/html_test_support/Cargo.toml")).unwrap();
    assert!(!html_manifest.contains("conformance-test-support"));
    let provenance_manifest =
        fs::read_to_string(root.join("crates/external_test_provenance/Cargo.toml")).unwrap();
    for forbidden in [
        "html =",
        "conformance-test-support",
        "wpt-test-support",
        "css =",
        "layout =",
        "gfx =",
    ] {
        assert!(!provenance_manifest.contains(forbidden));
    }

    let wpt_accounting =
        fs::read_to_string(root.join("crates/wpt_test_support/src/accounting.rs")).unwrap();
    for forbidden in [
        "repository_profiles",
        "EngineCapabilityKind::",
        "AssessmentState::",
    ] {
        assert!(
            !wpt_accounting.contains(forbidden),
            "WPT-owned accounting must consume generic assessment authority, not hard-code {forbidden}"
        );
    }
}

#[test]
fn source_set_schema_accepts_future_revisions_and_counts_within_explicit_bounds() {
    let temp = tempfile::tempdir().unwrap();
    write_synthetic_registry(temp.path(), &"b".repeat(40), 0);
    let set = load_wpt_source_set(temp.path()).unwrap();
    assert_eq!(set.revision().as_str(), "b".repeat(40));
    assert_eq!(set.records().len(), 1);
    assert_eq!(set.files().len(), 1);
}

#[test]
fn closure_bound_counts_all_files_for_each_record_not_parent_fanout() {
    let temp = tempfile::tempdir().unwrap();
    write_synthetic_registry(
        temp.path(),
        &"c".repeat(40),
        wpt_test_support::WPT_MAX_CLOSURE_FILES_PER_RECORD + 1,
    );
    assert_eq!(
        load_wpt_source_set(temp.path()).unwrap_err(),
        WptRegistryError::ClosureBoundExceeded
    );
}

#[test]
fn summary_closure_count_uses_file_roles_when_records_share_a_source() {
    let temp = tempfile::tempdir().unwrap();
    write_shared_source_registry(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    assert_eq!(set.records().len(), 2);
    assert_eq!(set.files().len(), 1);
    let summary = serialize_wpt_summary(&set, "metadata", "policy", "profile", &[]);
    let summary = std::str::from_utf8(&summary).unwrap();
    assert!(summary.contains("declared_records = 2\n"));
    assert!(summary.contains("declared_closure_files = 0\n"));
}

#[test]
fn fuzzy_metadata_preserves_owning_graph_nodes_in_model_and_summary() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    write_fuzzy_population(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let interpreted = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    let graph = interpreted[0].reference_graph().unwrap();
    let fuzzy = graph
        .fuzzy_metadata()
        .iter()
        .map(|value| (value.owner().as_str(), value.value()))
        .collect::<Vec<_>>();
    assert_eq!(
        fuzzy,
        vec![
            ("future/reference.html", "3-4;5-6"),
            ("future/root.html", "0-1;0-2"),
        ]
    );
    let policy = load_wpt_selection_policy(temp.path(), &set).unwrap();
    let profile = load_external_assessment_profile(temp.path()).unwrap();
    let accounted = account_wpt_source_set(&set, &policy, &profile, interpreted).unwrap();
    let summary = serialize_wpt_summary(
        &set,
        metadata.id(),
        policy.id(),
        profile.id().as_str(),
        &accounted,
    );
    let summary = std::str::from_utf8(&summary).unwrap();
    assert!(summary.contains("future/reference.html|3-4;5-6"));
    assert!(summary.contains("future/root.html|0-1;0-2"));
}

#[test]
fn valid_deferred_manual_form_and_inert_script_are_not_malformed_or_javascript() {
    let temp = tempfile::tempdir().unwrap();
    let revision = "d".repeat(40);
    let bytes = b"<!doctype html><script type=application/json>{\"data\":true}</script>\n";
    write_single_source_registry(temp.path(), &revision, "future/example-manual.html", bytes);
    let policy_path = temp
        .path()
        .join("tests/conformance/external/wpt/selection-policy.toml");
    fs::write(
        policy_path,
        r#"format = "borrowser-wpt-selection-policy-v1"
policy = "synthetic-manual-policy"
derived = []
[direct]
source_forms = ["reftest"]
path_categories = ["future"]
feature_areas = ["future-manual"]
no_js = "required"
resource_classes = ["self-contained"]
pixel_assertions = "exclude"
platform_dependencies = "exclude"
[[records]]
id = "future-record"
category = "future"
path_prefix = "future/"
"#,
    )
    .unwrap();
    let set = load_wpt_source_set(temp.path()).unwrap();
    write_synthetic_source_metadata(temp.path(), "future/example-manual.html", bytes);
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let records = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    assert_eq!(records[0].source_form(), WptSourceForm::Manual);
    assert!(
        !records[0]
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::NoJs)
    );
    assert!(
        !records[0]
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::RequiresJs)
    );
}

#[test]
fn javascript_and_no_js_requirements_require_positive_evidence() {
    for bytes in [
        b"<!doctype html><script>window.value = 1</script>\n".as_slice(),
        b"<!doctype html><script type=module>window.value = 1</script>\n".as_slice(),
    ] {
        let records = interpret_synthetic_html(bytes, false).unwrap();
        assert!(
            records[0]
                .generic_requirements()
                .requirement_tags()
                .contains(&RequirementTag::RequiresJs)
        );
        assert!(
            !records[0]
                .generic_requirements()
                .requirement_tags()
                .contains(&RequirementTag::NoJs)
        );
    }

    for bytes in [
        b"<!doctype html><script type=application/json>{\"data\":true}</script>\n".as_slice(),
        b"<!doctype html><button onclick=run()>run</button>\n".as_slice(),
        b"<!doctype html><a href=javascript:run()>run</a>\n".as_slice(),
    ] {
        let records = interpret_synthetic_html(bytes, false).unwrap();
        let tags = records[0].generic_requirements().requirement_tags();
        assert!(!tags.contains(&RequirementTag::RequiresJs));
        assert!(!tags.contains(&RequirementTag::NoJs));
    }

    let reviewed = interpret_synthetic_html(b"<!doctype html><p>static</p>\n", true).unwrap();
    assert!(
        reviewed[0]
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::NoJs)
    );
    assert!(
        !reviewed[0]
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::RequiresJs)
    );

    assert_eq!(
        interpret_synthetic_html(b"<!doctype html><script>run()</script>\n", true).unwrap_err(),
        wpt_test_support::WptInterpretationError::SourceMetadata(
            wpt_test_support::WptSourceMetadataError::ContradictoryNoJs
        )
    );
}

#[test]
fn record_local_reference_limits_preserve_population_accounting() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    write_mixed_reference_population(temp.path());
    let set = load_wpt_source_set(temp.path()).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    let interpreted = interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap();
    assert_eq!(interpreted.len(), 4);
    assert_eq!(
        record(&interpreted, "depth-record").interpretation_status(),
        wpt_test_support::WptInterpretationStatus::BoundedImportLimitation(
            wpt_test_support::WptInterpretationLimitation::ReferenceDepthBound
        )
    );
    assert_eq!(
        record(&interpreted, "cycle-record").interpretation_status(),
        wpt_test_support::WptInterpretationStatus::BoundedImportLimitation(
            wpt_test_support::WptInterpretationLimitation::ReferenceCycle
        )
    );
    assert!(
        record(&interpreted, "depth-record")
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::RequiresPixelComparison)
    );
    for id in ["ordinary-one", "ordinary-two"] {
        assert_eq!(
            record(&interpreted, id).interpretation_status(),
            wpt_test_support::WptInterpretationStatus::Complete
        );
    }

    let policy = load_wpt_selection_policy(temp.path(), &set).unwrap();
    let profile = load_external_assessment_profile(temp.path()).unwrap();
    let accounted = account_wpt_source_set(&set, &policy, &profile, interpreted).unwrap();
    assert_eq!(accounted.len(), set.records().len());
    assert_eq!(
        accounted
            .iter()
            .map(|record| record.interpreted().source_record_id())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );

    let depth_source = set
        .file_by_id(
            set.record(&conformance_test_support::SourceRecordId::parse("depth-record").unwrap())
                .unwrap()
                .source_file_id(),
        )
        .unwrap();
    let depth_path = temp.path().join(depth_source.local_path());
    let mut corrupted = fs::read(&depth_path).unwrap();
    corrupted[0] ^= 1;
    fs::write(depth_path, corrupted).unwrap();
    assert_eq!(
        interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap_err(),
        wpt_test_support::WptInterpretationError::Registry(WptRegistryError::HashMismatch)
    );
}

#[test]
fn controlled_feature_metadata_must_match_interpreted_upstream_evidence() {
    let temp = tempfile::tempdir().unwrap();
    copy_inputs(temp.path());
    let metadata_path = temp
        .path()
        .join("tests/conformance/external/wpt/source-metadata.toml");
    let metadata_text = fs::read_to_string(&metadata_path).unwrap().replace(
        "https://drafts.csswg.org/css-backgrounds/#special-backgrounds",
        "https://example.invalid/invented-feature",
    );
    fs::write(&metadata_path, metadata_text).unwrap();
    let set = load_wpt_source_set(temp.path()).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    assert_eq!(
        interpret_wpt_source_set(temp.path(), &set, &metadata).unwrap_err(),
        wpt_test_support::WptInterpretationError::SourceMetadata(
            wpt_test_support::WptSourceMetadataError::EvidenceMismatch
        )
    );
}

fn write_synthetic_registry(root: &Path, revision: &str, closure_count: usize) {
    let registry = root.join("tests/conformance/external/wpt/sources.toml");
    fs::create_dir_all(registry.parent().unwrap()).unwrap();
    let license_path = root.join("LICENSE.txt");
    fs::write(&license_path, b"synthetic license\n").unwrap();
    let license_hash = external_test_provenance::sha256(b"synthetic license\n").to_hex();
    let mut text = format!(
        r#"format = "borrowser-external-source-set-v1"
lineage_registry_format = "borrowser-external-lineage-registry-v1"
source_set = "synthetic-future-set"
upstream_project = "example/project"
revision = "{revision}"
license = "BSD-3-Clause"
license_notice_path = "LICENSE.txt"
license_notice_sha256 = "{license_hash}"
attribution = "Synthetic fixture"
lineages = []

[[files]]
id = "source"
path = "future/source.html"
sha256 = "{}"
role = "accounted-source"
parents = ["future-record"]
"#,
        "0".repeat(64)
    );
    for index in 0..closure_count {
        text.push_str(&format!(
            r#"
[[files]]
id = "closure-{index}"
path = "future/resource-{index}.dat"
sha256 = "{}"
role = "static-resource"
parents = ["future-record"]
"#,
            "0".repeat(64)
        ));
    }
    text.push_str(
        r#"
[[records]]
id = "future-record"
source_file = "source"
"#,
    );
    fs::write(registry, text).unwrap();
}

fn write_single_source_registry(root: &Path, revision: &str, upstream_path: &str, bytes: &[u8]) {
    let registry = root.join("tests/conformance/external/wpt/sources.toml");
    fs::create_dir_all(registry.parent().unwrap()).unwrap();
    fs::write(root.join("LICENSE.txt"), b"synthetic license\n").unwrap();
    let license_hash = external_test_provenance::sha256(b"synthetic license\n").to_hex();
    let source_hash = external_test_provenance::sha256(bytes).to_hex();
    fs::write(
        &registry,
        format!(
            r#"format = "borrowser-external-source-set-v1"
lineage_registry_format = "borrowser-external-lineage-registry-v1"
source_set = "synthetic-manual-set"
upstream_project = "example/project"
revision = "{revision}"
license = "BSD-3-Clause"
license_notice_path = "LICENSE.txt"
license_notice_sha256 = "{license_hash}"
attribution = "Synthetic fixture"
lineages = []
[[files]]
id = "source"
path = "{upstream_path}"
sha256 = "{source_hash}"
role = "accounted-source"
parents = ["future-record"]
[[records]]
id = "future-record"
source_file = "source"
"#
        ),
    )
    .unwrap();
    let raw = root.join(format!(
        "tests/conformance/external/wpt/raw/{revision}/{upstream_path}"
    ));
    fs::create_dir_all(raw.parent().unwrap()).unwrap();
    fs::write(raw, bytes).unwrap();
}

fn write_shared_source_registry(root: &Path) {
    let registry = root.join("tests/conformance/external/wpt/sources.toml");
    fs::create_dir_all(registry.parent().unwrap()).unwrap();
    fs::write(root.join("LICENSE.txt"), b"synthetic license\n").unwrap();
    let license_hash = external_test_provenance::sha256(b"synthetic license\n").to_hex();
    fs::write(
        registry,
        format!(
            r#"format = "borrowser-external-source-set-v1"
lineage_registry_format = "borrowser-external-lineage-registry-v1"
source_set = "synthetic-shared-source-set"
upstream_project = "example/project"
revision = "{}"
license = "BSD-3-Clause"
license_notice_path = "LICENSE.txt"
license_notice_sha256 = "{license_hash}"
attribution = "Synthetic fixture"
lineages = []
[[files]]
id = "shared-source"
path = "future/shared.html"
sha256 = "{}"
role = "accounted-source"
parents = ["future-record-one", "future-record-two"]
[[records]]
id = "future-record-one"
source_file = "shared-source"
[[records]]
id = "future-record-two"
source_file = "shared-source"
"#,
            "a".repeat(40),
            "0".repeat(64)
        ),
    )
    .unwrap();
}

fn write_fuzzy_population(root: &Path) {
    let revision = "9".repeat(40);
    let root_bytes = b"<!doctype html><meta name=fuzzy content=\"0-1;0-2\"><link rel=match href=reference.html>\n";
    let reference_bytes = b"<!doctype html><meta name=fuzzy content=\"3-4;5-6\">\n";
    fs::write(root.join("LICENSE.txt"), b"synthetic license\n").unwrap();
    let license_hash = external_test_provenance::sha256(b"synthetic license\n").to_hex();
    let registry = format!(
        r#"format = "borrowser-external-source-set-v1"
lineage_registry_format = "borrowser-external-lineage-registry-v1"
source_set = "synthetic-fuzzy-source-set"
upstream_project = "example/project"
revision = "{revision}"
license = "BSD-3-Clause"
license_notice_path = "LICENSE.txt"
license_notice_sha256 = "{license_hash}"
attribution = "Synthetic fixture"
lineages = []
[[files]]
id = "fuzzy-source"
path = "future/root.html"
sha256 = "{}"
role = "accounted-source"
parents = ["fuzzy-record"]
[[files]]
id = "fuzzy-reference"
path = "future/reference.html"
sha256 = "{}"
role = "reference-node"
parents = ["fuzzy-record"]
[[records]]
id = "fuzzy-record"
source_file = "fuzzy-source"
"#,
        external_test_provenance::sha256(root_bytes).to_hex(),
        external_test_provenance::sha256(reference_bytes).to_hex()
    );
    fs::write(
        root.join("tests/conformance/external/wpt/sources.toml"),
        registry,
    )
    .unwrap();
    for (path, bytes) in [
        ("future/root.html", root_bytes.as_slice()),
        ("future/reference.html", reference_bytes.as_slice()),
    ] {
        let destination = root.join(format!(
            "tests/conformance/external/wpt/raw/{revision}/{path}"
        ));
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, bytes).unwrap();
    }
    fs::write(
        root.join("tests/conformance/external/wpt/source-metadata.toml"),
        r#"format = "borrowser-wpt-source-metadata-v1"
source_metadata = "synthetic-fuzzy-metadata"
[[records]]
id = "fuzzy-record"
feature_areas = [{ id = "synthetic-fuzzy", evidence_kind = "source-path", evidence_value = "future/root.html" }]
capabilities = []
server_requirements = []
controlled_http = []
"#,
    )
    .unwrap();
    fs::write(
        root.join("tests/conformance/external/wpt/selection-policy.toml"),
        r#"format = "borrowser-wpt-selection-policy-v1"
policy = "synthetic-fuzzy-policy"
derived = []
[direct]
source_forms = ["reftest"]
path_categories = ["future"]
feature_areas = ["synthetic-fuzzy"]
no_js = "allowed"
resource_classes = ["pinned-local-static"]
pixel_assertions = "allow"
platform_dependencies = "allow"
[[records]]
id = "fuzzy-record"
category = "future"
path_prefix = "future/"
"#,
    )
    .unwrap();
}

fn write_synthetic_source_metadata(root: &Path, upstream_path: &str, bytes: &[u8]) {
    let path = root.join("tests/conformance/external/wpt/source-metadata.toml");
    fs::write(
        path,
        format!(
            r#"format = "borrowser-wpt-source-metadata-v1"
source_metadata = "synthetic-source-metadata"
[[records]]
id = "future-record"
feature_areas = [{{ id = "future-manual", evidence_kind = "source-path", evidence_value = "{upstream_path}" }}]
capabilities = []
server_requirements = []
controlled_http = []
# source digest: {}
"#,
            external_test_provenance::sha256(bytes).to_hex()
        ),
    )
    .unwrap();
}

fn interpret_synthetic_html(
    bytes: &[u8],
    establish_no_js: bool,
) -> Result<Vec<wpt_test_support::InterpretedWptRecord>, wpt_test_support::WptInterpretationError> {
    let temp = tempfile::tempdir().unwrap();
    let revision = "e".repeat(40);
    let upstream_path = "future/example-manual.html";
    write_single_source_registry(temp.path(), &revision, upstream_path, bytes);
    write_synthetic_source_metadata(temp.path(), upstream_path, bytes);
    if establish_no_js {
        let metadata_path = temp
            .path()
            .join("tests/conformance/external/wpt/source-metadata.toml");
        let metadata = fs::read_to_string(&metadata_path).unwrap().replacen(
            "id = \"future-record\"",
            &format!(
                "id = \"future-record\"\nno_js = {{ evidence_kind = \"source-sha256\", evidence_value = \"{}\" }}",
                external_test_provenance::sha256(bytes).to_hex()
            ),
            1,
        );
        fs::write(metadata_path, metadata).unwrap();
    }
    let set = load_wpt_source_set(temp.path()).unwrap();
    let metadata = load_wpt_source_metadata(temp.path(), &set).unwrap();
    interpret_wpt_source_set(temp.path(), &set, &metadata)
}

fn write_mixed_reference_population(root: &Path) {
    let revision = "f".repeat(40);
    let mut files = vec![
        (
            "ordinary-one-source".to_owned(),
            "future/ordinary-one-manual.html".to_owned(),
            b"<!doctype html><p>ordinary one</p>\n".to_vec(),
            "accounted-source",
            "ordinary-one".to_owned(),
        ),
        (
            "depth-source".to_owned(),
            "future/depth.html".to_owned(),
            b"<!doctype html><link rel=match href=depth-1.html>\n".to_vec(),
            "accounted-source",
            "depth-record".to_owned(),
        ),
        (
            "cycle-source".to_owned(),
            "future/cycle.html".to_owned(),
            b"<!doctype html><link rel=match href=cycle-ref.html>\n".to_vec(),
            "accounted-source",
            "cycle-record".to_owned(),
        ),
        (
            "cycle-reference".to_owned(),
            "future/cycle-ref.html".to_owned(),
            b"<!doctype html><link rel=match href=cycle.html>\n".to_vec(),
            "reference-node",
            "cycle-record".to_owned(),
        ),
        (
            "ordinary-two-source".to_owned(),
            "future/ordinary-two-manual.html".to_owned(),
            b"<!doctype html><p>ordinary two</p>\n".to_vec(),
            "accounted-source",
            "ordinary-two".to_owned(),
        ),
    ];
    for index in 1..=wpt_test_support::WPT_MAX_REFERENCE_DEPTH {
        let next = if index == wpt_test_support::WPT_MAX_REFERENCE_DEPTH {
            String::new()
        } else {
            format!("<link rel=match href=depth-{}.html>", index + 1)
        };
        files.push((
            format!("depth-reference-{index}"),
            format!("future/depth-{index}.html"),
            format!("<!doctype html>{next}\n").into_bytes(),
            "reference-node",
            "depth-record".to_owned(),
        ));
    }
    let license_bytes = b"synthetic license\n";
    fs::write(root.join("LICENSE.txt"), license_bytes).unwrap();
    let mut registry = format!(
        r#"format = "borrowser-external-source-set-v1"
lineage_registry_format = "borrowser-external-lineage-registry-v1"
source_set = "synthetic-mixed-reference-set"
upstream_project = "example/project"
revision = "{revision}"
license = "BSD-3-Clause"
license_notice_path = "LICENSE.txt"
license_notice_sha256 = "{}"
attribution = "Synthetic fixture"
lineages = []
"#,
        external_test_provenance::sha256(license_bytes).to_hex()
    );
    for (id, path, bytes, role, parent) in &files {
        registry.push_str(&format!(
            r#"
[[files]]
id = "{id}"
path = "{path}"
sha256 = "{}"
role = "{role}"
parents = ["{parent}"]
"#,
            external_test_provenance::sha256(bytes).to_hex()
        ));
        let destination = root.join(format!(
            "tests/conformance/external/wpt/raw/{revision}/{path}"
        ));
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, bytes).unwrap();
    }
    for (id, source_file) in [
        ("ordinary-one", "ordinary-one-source"),
        ("depth-record", "depth-source"),
        ("cycle-record", "cycle-source"),
        ("ordinary-two", "ordinary-two-source"),
    ] {
        registry.push_str(&format!(
            "\n[[records]]\nid = \"{id}\"\nsource_file = \"{source_file}\"\n"
        ));
    }
    fs::write(
        root.join("tests/conformance/external/wpt/sources.toml"),
        registry,
    )
    .unwrap();

    let mut metadata = String::from(
        "format = \"borrowser-wpt-source-metadata-v1\"\nsource_metadata = \"synthetic-mixed-metadata\"\n",
    );
    for (id, path) in [
        ("ordinary-one", "future/ordinary-one-manual.html"),
        ("depth-record", "future/depth.html"),
        ("cycle-record", "future/cycle.html"),
        ("ordinary-two", "future/ordinary-two-manual.html"),
    ] {
        metadata.push_str(&format!(
            r#"
[[records]]
id = "{id}"
feature_areas = [{{ id = "synthetic-reference", evidence_kind = "source-path", evidence_value = "{path}" }}]
capabilities = []
server_requirements = []
controlled_http = []
"#
        ));
    }
    fs::write(
        root.join("tests/conformance/external/wpt/source-metadata.toml"),
        metadata,
    )
    .unwrap();

    let mut policy = String::from(
        r#"format = "borrowser-wpt-selection-policy-v1"
policy = "synthetic-mixed-policy"
derived = []
[direct]
source_forms = ["reftest"]
path_categories = ["future"]
feature_areas = ["synthetic-reference"]
no_js = "allowed"
resource_classes = ["self-contained", "pinned-local-static"]
pixel_assertions = "allow"
platform_dependencies = "allow"
"#,
    );
    for id in [
        "ordinary-one",
        "depth-record",
        "cycle-record",
        "ordinary-two",
    ] {
        policy.push_str(&format!(
            "\n[[records]]\nid = \"{id}\"\ncategory = \"future\"\npath_prefix = \"future/\"\n"
        ));
    }
    fs::write(
        root.join("tests/conformance/external/wpt/selection-policy.toml"),
        policy,
    )
    .unwrap();
}

fn interpret(
    root: &Path,
    set: &wpt_test_support::ValidatedWptSourceSet,
) -> Vec<wpt_test_support::InterpretedWptRecord> {
    let metadata = load_wpt_source_metadata(root, set).unwrap();
    interpret_wpt_source_set(root, set, &metadata).unwrap()
}

fn account(
    root: &Path,
    set: &wpt_test_support::ValidatedWptSourceSet,
) -> Vec<wpt_test_support::AccountedWptRecord> {
    let policy = load_wpt_selection_policy(root, set).unwrap();
    let profile = load_external_assessment_profile(root).unwrap();
    account_wpt_source_set(set, &policy, &profile, interpret(root, set)).unwrap()
}

fn record<'a>(
    records: &'a [wpt_test_support::InterpretedWptRecord],
    id: &str,
) -> &'a wpt_test_support::InterpretedWptRecord {
    records
        .iter()
        .find(|record| record.source_record_id().as_str() == id)
        .unwrap()
}

fn has_capability(
    record: &wpt_test_support::InterpretedWptRecord,
    kind: EngineCapabilityKind,
    feature: &str,
) -> bool {
    record
        .generic_requirements()
        .capabilities()
        .iter()
        .any(|value| {
            value.kind() == kind
                && value
                    .feature()
                    .is_some_and(|candidate| candidate.as_str() == feature)
        })
}

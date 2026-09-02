use std::fs;

use conformance_test_support::{
    AssessmentEvidence, AssessmentState, CapabilityRequirement, EngineCapabilityKind,
    ExternalAssessmentProfileError, RequirementTag, SelectionPolicyAssessment,
    SelectionPolicyState, SourceRecordId, SourceRequirementsBuilder, SourceSelectionDecision,
    assess_external_source, load_external_assessment_profile,
};

fn write_profile(root: &std::path::Path, state: &str, extra: &str) {
    let path = root.join("tests/conformance/external/assessment-profile.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(root.join("evidence.txt"), b"stable repository evidence\n").unwrap();
    fs::write(
        path,
        format!(
            r#"format = "borrowser-external-assessment-profile-v1"
profile = "test-profile-v1"
{extra}
production = [{{ kind = "javascript-execution", state = "{state}", evidence = "Stable repository evidence for JavaScript capability.", evidence_refs = ["evidence.txt"] }}]
harness = []
environment = []
resource = []
representation = []
"#,
        ),
    )
    .unwrap();
}

#[test]
fn strict_profile_is_default_deny_and_changes_accounting_without_source_reinterpretation() {
    let temp = tempfile::tempdir().unwrap();
    write_profile(temp.path(), "unsupported", "");
    let unavailable = load_external_assessment_profile(temp.path()).unwrap();
    let js = CapabilityRequirement::new(EngineCapabilityKind::JavaScriptExecution, None).unwrap();
    let mut builder = SourceRequirementsBuilder::new();
    builder
        .requirement_tag(RequirementTag::RequiresJs)
        .capability(js);
    let requirements = builder.build().unwrap();
    let policy = || {
        SelectionPolicyAssessment::new(
            SelectionPolicyState::Included,
            vec![AssessmentEvidence::parse("Included by test policy.").unwrap()],
        )
    };
    let first = assess_external_source(
        SourceRecordId::parse("same-source").unwrap(),
        &requirements,
        unavailable.profiles(),
        policy(),
    );
    assert_eq!(first.decision(), &SourceSelectionDecision::NotSelected);

    write_profile(temp.path(), "supported", "");
    let available = load_external_assessment_profile(temp.path()).unwrap();
    let second = assess_external_source(
        SourceRecordId::parse("same-source").unwrap(),
        &requirements,
        available.profiles(),
        policy(),
    );
    assert_eq!(
        second.decision(),
        &SourceSelectionDecision::SelectedForDirectExecution
    );
    assert_eq!(
        second.production_assessment().facts()[0].state(),
        AssessmentState::Supported
    );
    assert_eq!(
        requirements.requirement_tags(),
        &[RequirementTag::RequiresJs]
    );
}

#[test]
fn unknown_fields_versions_and_unsupported_strong_evidence_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    write_profile(temp.path(), "unsupported", "unknown = true");
    assert_eq!(
        load_external_assessment_profile(temp.path()).unwrap_err(),
        ExternalAssessmentProfileError::InvalidSchema
    );
    write_profile(temp.path(), "supported", "");
    let path = temp
        .path()
        .join("tests/conformance/external/assessment-profile.toml");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("evidence_refs = [\"evidence.txt\"]", "evidence_refs = []");
    fs::write(&path, text).unwrap();
    assert_eq!(
        load_external_assessment_profile(temp.path()).unwrap_err(),
        ExternalAssessmentProfileError::MissingEvidenceReference
    );
}

#[cfg(unix)]
#[test]
fn assessment_profile_rejects_symlinked_parent_directory() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    write_profile(temp.path(), "unsupported", "");
    let parent = temp.path().join("tests/conformance/external");
    let real = temp.path().join("tests/conformance/real-external");
    fs::rename(&parent, &real).unwrap();
    symlink(&real, &parent).unwrap();
    assert!(load_external_assessment_profile(temp.path()).is_err());
}

use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::Path;

use conformance_test_support::{
    DerivedAdaptationDecision, ExternalAssessmentProfileError, SourceSelectionDecision,
    load_external_assessment_profile,
};
use external_test_provenance::{
    ConfinedFileError, read_confined_regular_file, validate_confined_output_file,
};

use crate::{
    AccountedWptRecord, ValidatedWptSourceSet, WptAccountingError, WptFileRole,
    WptInterpretationError, WptRegistryError, WptSelectionPolicyError, WptSourceMetadataError,
    account_wpt_source_set, interpret_wpt_source_set, load_wpt_selection_policy,
    load_wpt_source_metadata, load_wpt_source_set, validate_materialized_sources,
};

pub const WPT_IMPORT_SUMMARY_FORMAT_V1: &str = "borrowser-wpt-import-summary-v1";
pub const WPT_IMPORT_SUMMARY_PATH: &str = "tests/conformance/external/wpt/accounting-summary.toml";
const MAX_SUMMARY_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug)]
pub struct WptAccountingSummary {
    bytes: Vec<u8>,
}
impl WptAccountingSummary {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WptSummaryCheck {
    Current,
    Missing,
    Stale,
}
#[derive(Debug)]
pub enum WptSummaryError {
    Registry(WptRegistryError),
    Interpretation(WptInterpretationError),
    Accounting(WptAccountingError),
    AssessmentProfile(ExternalAssessmentProfileError),
    SelectionPolicy(WptSelectionPolicyError),
    SourceMetadata(WptSourceMetadataError),
    Io,
    UnsafeOutput,
}
impl From<WptRegistryError> for WptSummaryError {
    fn from(v: WptRegistryError) -> Self {
        Self::Registry(v)
    }
}
impl From<WptInterpretationError> for WptSummaryError {
    fn from(v: WptInterpretationError) -> Self {
        Self::Interpretation(v)
    }
}
impl From<WptAccountingError> for WptSummaryError {
    fn from(v: WptAccountingError) -> Self {
        Self::Accounting(v)
    }
}
impl From<ExternalAssessmentProfileError> for WptSummaryError {
    fn from(v: ExternalAssessmentProfileError) -> Self {
        Self::AssessmentProfile(v)
    }
}
impl From<WptSelectionPolicyError> for WptSummaryError {
    fn from(v: WptSelectionPolicyError) -> Self {
        Self::SelectionPolicy(v)
    }
}
impl From<WptSourceMetadataError> for WptSummaryError {
    fn from(v: WptSourceMetadataError) -> Self {
        Self::SourceMetadata(v)
    }
}

pub fn generate_repository_wpt_summary(
    repository_root: &Path,
) -> Result<WptAccountingSummary, WptSummaryError> {
    let set = load_wpt_source_set(repository_root)?;
    validate_materialized_sources(repository_root, &set)?;
    let source_metadata = load_wpt_source_metadata(repository_root, &set)?;
    let selection_policy = load_wpt_selection_policy(repository_root, &set)?;
    let assessment_profile = load_external_assessment_profile(repository_root)?;
    let interpreted = interpret_wpt_source_set(repository_root, &set, &source_metadata)?;
    let accounted =
        account_wpt_source_set(&set, &selection_policy, &assessment_profile, interpreted)?;
    Ok(WptAccountingSummary {
        bytes: serialize_wpt_summary(
            &set,
            source_metadata.id(),
            selection_policy.id(),
            assessment_profile.id().as_str(),
            &accounted,
        ),
    })
}

pub fn serialize_wpt_summary(
    set: &ValidatedWptSourceSet,
    source_metadata: &str,
    selection_policy: &str,
    assessment_profile: &str,
    records: &[AccountedWptRecord],
) -> Vec<u8> {
    let mut output = String::new();
    field(&mut output, "format", WPT_IMPORT_SUMMARY_FORMAT_V1);
    field(&mut output, "source_set", set.source_set());
    field(&mut output, "source_metadata", source_metadata);
    field(&mut output, "selection_policy", selection_policy);
    field(&mut output, "assessment_profile", assessment_profile);
    field(&mut output, "upstream_project", set.project().as_str());
    field(&mut output, "revision", set.revision().as_str());
    field(&mut output, "license", set.license().as_str());
    field(&mut output, "license_notice", set.license_notice().as_str());
    field(
        &mut output,
        "license_notice_sha256",
        &set.license_notice_sha256().to_hex(),
    );
    field(&mut output, "attribution", set.attribution().as_str());
    writeln!(&mut output, "declared_records = {}", set.records().len()).unwrap();
    writeln!(
        &mut output,
        "declared_closure_files = {}",
        set.files()
            .iter()
            .filter(|file| file.role() != WptFileRole::AccountedSource)
            .count()
    )
    .unwrap();
    writeln!(&mut output, "accounted_records = {}", records.len()).unwrap();
    writeln!(
        &mut output,
        "directly_selected_records = {}",
        records
            .iter()
            .filter(|r| matches!(
                r.generic_accounting().decision(),
                SourceSelectionDecision::SelectedForDirectExecution
            ))
            .count()
    )
    .unwrap();
    writeln!(
        &mut output,
        "directly_not_selected_records = {}",
        records
            .iter()
            .filter(|r| matches!(
                r.generic_accounting().decision(),
                SourceSelectionDecision::NotSelected
            ))
            .count()
    )
    .unwrap();
    writeln!(
        &mut output,
        "selected_derived_adaptations = {}",
        records
            .iter()
            .flat_map(AccountedWptRecord::derived_adaptations)
            .filter(|a| a.decision() == &DerivedAdaptationDecision::Selected)
            .count()
    )
    .unwrap();
    for record in records {
        let interpreted = record.interpreted();
        let generic = record.generic_accounting();
        let source_record = set
            .record(interpreted.source_record_id())
            .expect("accounted record belongs to validated source set");
        let source = set
            .file_by_id(source_record.source_file_id())
            .expect("validated source file");
        output.push_str("\n[[records]]\n");
        field(&mut output, "id", interpreted.source_record_id().as_str());
        field(
            &mut output,
            "upstream_path",
            source.identity().path().as_str(),
        );
        field(
            &mut output,
            "source_sha256",
            &source.identity().sha256().to_hex(),
        );
        field(
            &mut output,
            "source_form",
            interpreted.source_form().as_str(),
        );
        field(
            &mut output,
            "interpretation_status",
            interpreted.interpretation_status().as_str(),
        );
        field(
            &mut output,
            "interpretation_limitation",
            interpreted
                .interpretation_status()
                .limitation()
                .map(|value| value.as_str())
                .unwrap_or("none"),
        );
        array(
            &mut output,
            "requirement_tags",
            interpreted
                .generic_requirements()
                .requirement_tags()
                .iter()
                .map(|v| v.as_str().to_owned()),
        );
        array(
            &mut output,
            "capability_requirements",
            interpreted
                .generic_requirements()
                .capabilities()
                .iter()
                .map(|v| v.as_key()),
        );
        array(
            &mut output,
            "harness_requirements",
            interpreted
                .generic_requirements()
                .harness()
                .iter()
                .map(|v| v.as_key()),
        );
        array(
            &mut output,
            "environment_requirements",
            interpreted
                .generic_requirements()
                .environment()
                .iter()
                .map(|v| format!("{}:{}", v.kind().as_str(), v.profile().as_str())),
        );
        array(
            &mut output,
            "resource_requirements",
            interpreted
                .generic_requirements()
                .resources()
                .iter()
                .map(|v| v.as_key()),
        );
        array(
            &mut output,
            "assertion_requirements",
            interpreted
                .generic_requirements()
                .assertions()
                .iter()
                .map(|v| v.as_key()),
        );
        array(
            &mut output,
            "automation_requirements",
            interpreted
                .automation_requirements()
                .iter()
                .map(|v| v.as_str().to_owned()),
        );
        array(
            &mut output,
            "readiness_requirements",
            interpreted
                .readiness_requirements()
                .iter()
                .map(|v| v.as_str().to_owned()),
        );
        array(
            &mut output,
            "server_requirements",
            interpreted
                .server_requirements()
                .iter()
                .map(|v| v.as_str().to_owned()),
        );
        array(
            &mut output,
            "resource_details",
            interpreted.resource_details().iter().map(|v| v.as_str()),
        );
        if let Some(graph) = interpreted.reference_graph() {
            array(
                &mut output,
                "reference_relations",
                graph.edges().iter().map(|edge| {
                    format!(
                        "{}:{}->{}",
                        edge.relation().as_str(),
                        edge.source().as_str(),
                        edge.target().as_str()
                    )
                }),
            );
            array(
                &mut output,
                "fuzzy_metadata",
                graph
                    .fuzzy_metadata()
                    .iter()
                    .map(|value| format!("{}|{}", value.owner().as_str(), value.value())),
            );
        } else {
            array(
                &mut output,
                "reference_relations",
                std::iter::empty::<String>(),
            );
            array(&mut output, "fuzzy_metadata", std::iter::empty::<String>());
        }
        assessment_array(
            &mut output,
            "production_assessment",
            generic.production_assessment().facts(),
        );
        assessment_array(
            &mut output,
            "harness_assessment",
            generic.harness_assessment().facts(),
        );
        assessment_array(
            &mut output,
            "environment_assessment",
            generic.environment_assessment().facts(),
        );
        assessment_array(
            &mut output,
            "resource_assessment",
            generic.resource_assessment().facts(),
        );
        assessment_array(
            &mut output,
            "representation_assessment",
            generic.representation_assessment().facts(),
        );
        array(
            &mut output,
            "filter_assessment",
            record.filter_assessment().facts().iter().map(|fact| {
                format!(
                    "{}|{}|{}",
                    fact.dimension().as_str(),
                    fact.outcome().as_str(),
                    fact.evidence()
                )
            }),
        );
        field(&mut output, "direct_decision", generic.decision().as_str());
        for adaptation in record.derived_adaptations() {
            output.push_str("\n[[records.derived_adaptations]]\n");
            field(&mut output, "lineage_id", adaptation.lineage_id().as_str());
            field(
                &mut output,
                "decision",
                match adaptation.decision() {
                    DerivedAdaptationDecision::Selected => "selected",
                    DerivedAdaptationDecision::NotSelected => "not-selected",
                    DerivedAdaptationDecision::NotYetClassifiable => "not-yet-classifiable",
                },
            );
            assessment_array(
                &mut output,
                "production_assessment",
                adaptation.production_assessment().facts(),
            );
            assessment_array(
                &mut output,
                "harness_assessment",
                adaptation.harness_assessment().facts(),
            );
            assessment_array(
                &mut output,
                "environment_assessment",
                adaptation.environment_assessment().facts(),
            );
            assessment_array(
                &mut output,
                "resource_assessment",
                adaptation.resource_assessment().facts(),
            );
            assessment_array(
                &mut output,
                "representation_assessment",
                adaptation.representation_assessment().facts(),
            );
        }
    }
    output.into_bytes()
}

fn assessment_array(
    output: &mut String,
    key: &str,
    facts: &[conformance_test_support::RequirementAssessment],
) {
    array(
        output,
        key,
        facts.iter().map(|fact| {
            format!(
                "{}|{}|{}",
                fact.key(),
                fact.state().as_str(),
                fact.evidence().as_str()
            )
        }),
    )
}
fn field(output: &mut String, key: &str, value: &str) {
    writeln!(output, "{key} = {}", toml::Value::String(value.to_owned())).unwrap()
}
fn array(output: &mut String, key: &str, values: impl IntoIterator<Item = String>) {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    write!(output, "{key} = [").unwrap();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(", ")
        }
        write!(output, "{}", toml::Value::String(value.clone())).unwrap();
    }
    output.push_str("]\n")
}

pub fn check_repository_wpt_summary(
    repository_root: &Path,
    summary: &WptAccountingSummary,
) -> Result<WptSummaryCheck, WptSummaryError> {
    match read_confined_regular_file(
        repository_root,
        Path::new(WPT_IMPORT_SUMMARY_PATH),
        MAX_SUMMARY_BYTES,
    ) {
        Ok(bytes) => Ok(if bytes == summary.bytes {
            WptSummaryCheck::Current
        } else {
            WptSummaryCheck::Stale
        }),
        Err(ConfinedFileError::Missing) => Ok(WptSummaryCheck::Missing),
        Err(ConfinedFileError::Io) => Err(WptSummaryError::Io),
        Err(_) => Err(WptSummaryError::UnsafeOutput),
    }
}

pub fn update_repository_wpt_summary(
    repository_root: &Path,
    summary: &WptAccountingSummary,
) -> Result<(), WptSummaryError> {
    let path = validate_confined_output_file(repository_root, Path::new(WPT_IMPORT_SUMMARY_PATH))
        .map_err(|error| match error {
        ConfinedFileError::Io => WptSummaryError::Io,
        _ => WptSummaryError::UnsafeOutput,
    })?;
    let parent = path.parent().ok_or(WptSummaryError::UnsafeOutput)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| WptSummaryError::Io)?;
    temporary
        .write_all(&summary.bytes)
        .map_err(|_| WptSummaryError::Io)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| WptSummaryError::Io)?;
    temporary.persist(&path).map_err(|_| WptSummaryError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_fields_do_not_admit_time_or_host_state() {
        assert!(!WPT_IMPORT_SUMMARY_FORMAT_V1.contains("time"));
        let mut text = String::new();
        array(&mut text, "x", vec!["b".to_owned(), "a".to_owned()]);
        assert_eq!(text, "x = [\"a\", \"b\"]\n");
    }
}

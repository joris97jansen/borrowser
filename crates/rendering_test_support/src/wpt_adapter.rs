use std::path::Path;

use conformance_test_support::{
    DerivedAdaptationDecision, FixtureSource, InventoryRepository, ObservationSurface,
    ReferenceKind, ReferenceRelation, SourceSelectionDecision, discover_inventory,
    load_external_assessment_profile,
};
use wpt_test_support::{
    WptFileRole, WptReferenceGraph, WptReferenceRelation, WptSourceFile, WptSourceForm,
    account_wpt_source_set, interpret_wpt_source_set, load_wpt_selection_policy,
    load_wpt_source_metadata, load_wpt_source_set, read_declared_file,
    validate_materialized_sources,
};

pub const AG8_DERIVED_LINEAGE_ID: &str = "wpt-body-background-display-none-paint-v1";
pub const AG8_DERIVED_TEST_ID: &str = "wpt-derived-body-background-display-none";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptRenderingAdaptationError {
    SourceSet,
    Integrity,
    Interpretation,
    Accounting,
    MissingSource,
    UntruthfulSourceAssertion,
    MissingFixture,
    InvalidFixtureLineage,
    InvalidFixtureObservation,
    DerivedArtifactDrift,
}

/// Verifies the one AG8 rendering adaptation without claiming a WPT raster result.
pub fn validate_ag8_rendering_adaptation(
    repository_root: &Path,
) -> Result<(), WptRenderingAdaptationError> {
    let set =
        load_wpt_source_set(repository_root).map_err(|_| WptRenderingAdaptationError::SourceSet)?;
    validate_materialized_sources(repository_root, &set)
        .map_err(|_| WptRenderingAdaptationError::Integrity)?;
    let selection_policy = load_wpt_selection_policy(repository_root, &set)
        .map_err(|_| WptRenderingAdaptationError::SourceSet)?;
    let source_metadata = load_wpt_source_metadata(repository_root, &set)
        .map_err(|_| WptRenderingAdaptationError::SourceSet)?;
    let assessment_profile = load_external_assessment_profile(repository_root)
        .map_err(|_| WptRenderingAdaptationError::Accounting)?;
    let interpreted = interpret_wpt_source_set(repository_root, &set, &source_metadata)
        .map_err(|_| WptRenderingAdaptationError::Interpretation)?;
    let accounted =
        account_wpt_source_set(&set, &selection_policy, &assessment_profile, interpreted)
            .map_err(|_| WptRenderingAdaptationError::Accounting)?;
    let source = accounted
        .iter()
        .find(|record| {
            record.interpreted().source_record_id().as_str()
                == "wpt-css-body-background-display-none"
        })
        .ok_or(WptRenderingAdaptationError::MissingSource)?;
    let graph = source
        .interpreted()
        .reference_graph()
        .ok_or(WptRenderingAdaptationError::UntruthfulSourceAssertion)?;
    if source.interpreted().source_form() != WptSourceForm::Reftest
        || graph.edges().len() != 1
        || graph.edges()[0].relation() != WptReferenceRelation::Match
        || !source.interpreted().readiness_requirements().is_empty()
        || source.generic_accounting().decision() != &SourceSelectionDecision::NotSelected
        || !source.derived_adaptations().iter().any(|adaptation| {
            adaptation.lineage_id().as_str() == AG8_DERIVED_LINEAGE_ID
                && adaptation.decision() == &DerivedAdaptationDecision::Selected
        })
    {
        return Err(WptRenderingAdaptationError::UntruthfulSourceAssertion);
    }
    let inventory = discover_inventory(&InventoryRepository::new(
        repository_root,
        repository_root.join("tests/conformance/fixtures"),
    ))
    .map_err(|_| WptRenderingAdaptationError::MissingFixture)?;
    let fixture = inventory
        .fixtures()
        .iter()
        .find(|fixture| fixture.id().as_str() == AG8_DERIVED_TEST_ID)
        .ok_or(WptRenderingAdaptationError::MissingFixture)?;
    if !matches!(fixture.source(),FixtureSource::ExternalDerived{lineage_id,adapter,adapter_version}if lineage_id.as_str()==AG8_DERIVED_LINEAGE_ID && adapter.as_str()=="rendering-paired-semantic" && adapter_version.as_str()=="1")
    {
        return Err(WptRenderingAdaptationError::InvalidFixtureLineage);
    }
    let reference = fixture
        .reference()
        .ok_or(WptRenderingAdaptationError::InvalidFixtureObservation)?;
    if fixture.observation() != ObservationSurface::PaintOperations
        || reference.kind() != ReferenceKind::Semantic
        || reference.relation() != ReferenceRelation::Match
    {
        return Err(WptRenderingAdaptationError::InvalidFixtureObservation);
    }
    let lineage = set
        .lineages()
        .iter()
        .find(|lineage| lineage.id().as_str() == AG8_DERIVED_LINEAGE_ID)
        .ok_or(WptRenderingAdaptationError::InvalidFixtureLineage)?;
    let lineage_reference = set
        .file_by_id(lineage.reference_file_id())
        .ok_or(WptRenderingAdaptationError::MissingSource)?;
    if !lineage_reference_matches_graph(graph, lineage_reference) {
        return Err(WptRenderingAdaptationError::UntruthfulSourceAssertion);
    }
    let source_record = set
        .record(lineage.source_record())
        .ok_or(WptRenderingAdaptationError::MissingSource)?;
    let upstream_test = read_declared_file(
        repository_root,
        set.file_by_id(source_record.source_file_id())
            .ok_or(WptRenderingAdaptationError::MissingSource)?,
    )
    .map_err(|_| WptRenderingAdaptationError::Integrity)?;
    let upstream_reference = read_declared_file(repository_root, lineage_reference)
        .map_err(|_| WptRenderingAdaptationError::Integrity)?;
    let derived_root = repository_root.join(
        "tests/conformance/fixtures/rendering/wpt-derived-body-background-display-none/rendering",
    );
    let derived_test = std::fs::read(derived_root.join("test.html"))
        .map_err(|_| WptRenderingAdaptationError::DerivedArtifactDrift)?;
    let derived_reference = std::fs::read(derived_root.join("reference.html"))
        .map_err(|_| WptRenderingAdaptationError::DerivedArtifactDrift)?;
    validate_exact_copy_bytes(
        &upstream_test,
        &upstream_reference,
        &derived_test,
        &derived_reference,
    )?;
    Ok(())
}

fn lineage_reference_matches_graph(
    graph: &WptReferenceGraph,
    lineage_reference: &WptSourceFile,
) -> bool {
    graph.edges().len() == 1
        && graph.edges()[0].relation() == WptReferenceRelation::Match
        && graph.edges()[0].target() == lineage_reference.identity().path()
        && lineage_reference.role() == WptFileRole::ReferenceNode
}

fn validate_exact_copy_bytes(
    upstream_test: &[u8],
    upstream_reference: &[u8],
    derived_test: &[u8],
    derived_reference: &[u8],
) -> Result<(), WptRenderingAdaptationError> {
    if derived_test == upstream_test && derived_reference == upstream_reference {
        Ok(())
    } else {
        Err(WptRenderingAdaptationError::DerivedArtifactDrift)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_copy_adapter_rejects_unrelated_or_drifted_derived_bytes() {
        assert_eq!(
            validate_exact_copy_bytes(b"test", b"reference", b"test", b"reference"),
            Ok(())
        );
        assert_eq!(
            validate_exact_copy_bytes(b"test", b"reference", b"unrelated", b"reference"),
            Err(WptRenderingAdaptationError::DerivedArtifactDrift)
        );
        assert_eq!(
            validate_exact_copy_bytes(b"test", b"reference", b"test", b"unrelated"),
            Err(WptRenderingAdaptationError::DerivedArtifactDrift)
        );
    }

    #[test]
    fn lineage_reference_must_be_the_actual_match_target() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let set = load_wpt_source_set(root).unwrap();
        let metadata = load_wpt_source_metadata(root, &set).unwrap();
        let interpreted = interpret_wpt_source_set(root, &set, &metadata).unwrap();
        let source = interpreted
            .iter()
            .find(|record| {
                record.source_record_id().as_str() == "wpt-css-body-background-display-none"
            })
            .unwrap();
        let graph = source.reference_graph().unwrap();
        assert!(lineage_reference_matches_graph(
            graph,
            set.file_by_id("blank-reference").unwrap()
        ));
        assert!(!lineage_reference_matches_graph(
            graph,
            set.file_by_id("wait-reference").unwrap()
        ));
    }
}

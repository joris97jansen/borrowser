//! Deterministic inventory and classification tooling for Borrowser conformance fixtures.
//!
//! This crate owns fixture discovery, inventory validation, manifest
//! generation, and expected-result metadata validation. It does not execute
//! fixtures or implement browser semantics.

mod assessment_profile;
mod classification;
mod descriptor;
mod diagnostic;
mod discovery;
mod expected_results;
mod external_source;
mod lineage_registry;
mod manifest;
mod model;
mod write;

pub use assessment_profile::{
    EXTERNAL_ASSESSMENT_PROFILE_FORMAT_V1, EXTERNAL_ASSESSMENT_PROFILE_PATH,
    ExternalAssessmentProfileError, ValidatedExternalAssessmentProfile,
    load_external_assessment_profile,
};
pub use classification::{
    CapabilityFeatureId, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirementKind,
    HarnessLimitationKind, RequirementTag, SemanticIdentifierError,
};
pub use diagnostic::{InventoryDiagnostic, InventoryDiagnosticKind, InventoryErrors};
pub use discovery::{InventoryRepository, discover_inventory};
pub use expected_results::{
    ClassificationView, ClassifiedExpectedResultView, EngineCapabilityView,
    EnvironmentRequirementView, ExecutionBlocker, ExecutionEligibility,
    ExecutionEnvironmentAssessment, ExpectationView, ExpectedFailureClassification,
    ExpectedResultView, ExpectedResultsErrors, HarnessLimitationView, HarnessLimitationViews,
    HarnessReadinessView, LaneExclusionView, LanePolicyScope, MissingCapabilityView,
    MissingCapabilityViews, StabilityView, SubsystemOwner, UnresolvedPrerequisite,
    ValidatedExpectedResults, evaluate_execution_eligibility, load_expected_results,
    serialize_expected_results_summary,
};
pub use external_source::{
    AccountedDerivedAdaptation, AccountedExternalSource, AssessmentEvidence, AssessmentProfileId,
    AssessmentState, CapabilityRequirement, DerivedAdaptationDecision,
    EnvironmentSupportAssessment, ExternalSourceAssessmentProfiles, GenericAssertionRequirement,
    GenericHarnessRequirement, GenericResourceRequirement, HarnessAssessment, HarnessFeatureId,
    ProductionCapabilityAssessment, ProfileEntry, RepresentationAssessment,
    RepresentationFeatureId, RequirementAssessment, ResourceProfileId,
    SelectionEnvironmentAssessment, SelectionPolicyAssessment, SelectionPolicyState,
    SourceEnvironmentRequirement, SourceRecordId, SourceRequirements, SourceRequirementsBuilder,
    SourceSelectionDecision, account_malformed_external_source, assess_derived_adaptation,
    assess_external_source,
};
pub use lineage_registry::{
    EXTERNAL_LINEAGE_REGISTRY_FORMAT_V1, EXTERNAL_REGISTRY_INDEX_FORMAT_V1,
    EXTERNAL_REGISTRY_INDEX_PATH, ExternalLineageDeclaration, ExternalLineageRegistryError,
    ValidatedExternalLineageRegistry, load_external_lineage_registry,
    reconcile_external_fixture_lineages,
};
pub use manifest::{
    CONFORMANCE_MANIFEST_FORMAT_V1, CONFORMANCE_MANIFEST_FORMAT_V2, CONFORMANCE_MANIFEST_FORMAT_V3,
    CONFORMANCE_MANIFEST_FORMAT_V4, ConformanceManifest, build_manifest, generate_manifest_bytes,
    serialize_manifest,
};
pub use model::{
    CONFORMANCE_FIXTURE_FORMAT_V1, CONFORMANCE_FIXTURE_FORMAT_V2, CONFORMANCE_FIXTURE_FORMAT_V3,
    CONFORMANCE_FIXTURE_FORMAT_V4, ExecutionPackage, ExternalAdapterVersion, ExternalLineageId,
    FixtureFormat, FixtureSource, InventoryScope, MAX_DESCRIPTOR_BYTES,
    MAX_EXECUTION_SUPPORT_PATHS_V2, ObservationSurface, ReferenceDeclaration, ReferenceKind,
    ReferenceRelation, RepositoryPath, SourceKind, TestId, TestIdValidationError, ValidatedFixture,
    ValidatedInventory,
};
pub use write::{ManifestCheck, ManifestOutputError, check_manifest, update_manifest};

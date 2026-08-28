//! Deterministic inventory and classification tooling for Borrowser conformance fixtures.
//!
//! This crate owns fixture discovery, inventory validation, manifest
//! generation, and expected-result metadata validation. It does not execute
//! fixtures or implement browser semantics.

mod descriptor;
mod diagnostic;
mod discovery;
mod expected_results;
mod manifest;
mod model;
mod write;

pub use diagnostic::{InventoryDiagnostic, InventoryDiagnosticKind, InventoryErrors};
pub use discovery::{InventoryRepository, discover_inventory};
pub use expected_results::{
    ExpectedResultsErrors, ValidatedExpectedResults, load_expected_results,
    serialize_expected_results_summary,
};
pub use manifest::{
    CONFORMANCE_MANIFEST_FORMAT_V1, ConformanceManifest, build_manifest, generate_manifest_bytes,
    serialize_manifest,
};
pub use model::{
    CONFORMANCE_FIXTURE_FORMAT_V1, InventoryScope, MAX_DESCRIPTOR_BYTES, ObservationSurface,
    ReferenceDeclaration, ReferenceKind, RepositoryPath, SourceKind, TestId, ValidatedFixture,
    ValidatedInventory,
};
pub use write::{ManifestCheck, ManifestOutputError, check_manifest, update_manifest};

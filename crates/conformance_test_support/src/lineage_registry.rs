//! Source-neutral repository reconciliation for externally derived fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use external_test_provenance::{ConfinedFileError, read_confined_regular_file};
use serde::Deserialize;

use crate::{
    ExternalAdapterVersion, ExternalLineageId, FixtureSource, HarnessFeatureId, SourceRecordId,
    TestId, ValidatedInventory,
};

pub const EXTERNAL_REGISTRY_INDEX_FORMAT_V1: &str = "borrowser-external-registry-index-v1";
pub const EXTERNAL_REGISTRY_INDEX_PATH: &str = "tests/conformance/external/registries.toml";
pub const EXTERNAL_LINEAGE_REGISTRY_FORMAT_V1: &str = "borrowser-external-lineage-registry-v1";
const MAX_INDEX_BYTES: u64 = 64 * 1024;
const MAX_REGISTRY_BYTES: u64 = 512 * 1024;
const MAX_REGISTRIES: usize = 16;
const MAX_LINEAGES: usize = 256;

#[derive(Clone, Debug)]
pub struct ExternalLineageDeclaration {
    id: ExternalLineageId,
    source_record: SourceRecordId,
    adapter: HarnessFeatureId,
    adapter_version: ExternalAdapterVersion,
    derived_test_id: TestId,
}
impl ExternalLineageDeclaration {
    pub fn id(&self) -> &ExternalLineageId {
        &self.id
    }
    pub fn source_record(&self) -> &SourceRecordId {
        &self.source_record
    }
    pub fn adapter(&self) -> &HarnessFeatureId {
        &self.adapter
    }
    pub fn adapter_version(&self) -> &ExternalAdapterVersion {
        &self.adapter_version
    }
    pub fn derived_test_id(&self) -> &TestId {
        &self.derived_test_id
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedExternalLineageRegistry {
    declarations: BTreeMap<ExternalLineageId, ExternalLineageDeclaration>,
}
impl ValidatedExternalLineageRegistry {
    pub fn declarations(&self) -> impl Iterator<Item = &ExternalLineageDeclaration> {
        self.declarations.values()
    }
    pub fn get(&self, id: &ExternalLineageId) -> Option<&ExternalLineageDeclaration> {
        self.declarations.get(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalLineageRegistryError {
    Io,
    TooLarge,
    InvalidSchema,
    UnsupportedVersion,
    InvalidPath,
    DuplicateLineage,
    DanglingSourceRecord,
    DanglingFixtureLineage,
    DerivedTestMismatch,
    AdapterMismatch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexWire {
    format: String,
    registries: Vec<String>,
}

pub fn load_external_lineage_registry(
    repository_root: &Path,
) -> Result<ValidatedExternalLineageRegistry, ExternalLineageRegistryError> {
    let bytes = read_confined(
        repository_root,
        Path::new(EXTERNAL_REGISTRY_INDEX_PATH),
        MAX_INDEX_BYTES,
    )?;
    let index: IndexWire =
        toml::from_slice(&bytes).map_err(|_| ExternalLineageRegistryError::InvalidSchema)?;
    if index.format != EXTERNAL_REGISTRY_INDEX_FORMAT_V1 {
        return Err(ExternalLineageRegistryError::UnsupportedVersion);
    }
    if index.registries.is_empty() || index.registries.len() > MAX_REGISTRIES {
        return Err(ExternalLineageRegistryError::TooLarge);
    }
    let mut seen_paths = BTreeSet::new();
    let mut declarations = BTreeMap::new();
    for registry_path in index.registries {
        if !seen_paths.insert(registry_path.clone()) {
            return Err(ExternalLineageRegistryError::InvalidSchema);
        }
        let bytes = read_confined(
            repository_root,
            Path::new(&registry_path),
            MAX_REGISTRY_BYTES,
        )?;
        let value: toml::Value =
            toml::from_slice(&bytes).map_err(|_| ExternalLineageRegistryError::InvalidSchema)?;
        let table = value
            .as_table()
            .ok_or(ExternalLineageRegistryError::InvalidSchema)?;
        if table
            .get("lineage_registry_format")
            .and_then(toml::Value::as_str)
            != Some(EXTERNAL_LINEAGE_REGISTRY_FORMAT_V1)
        {
            return Err(ExternalLineageRegistryError::UnsupportedVersion);
        }
        let records = table
            .get("records")
            .and_then(toml::Value::as_array)
            .ok_or(ExternalLineageRegistryError::InvalidSchema)?;
        let mut record_ids = BTreeSet::new();
        for record in records {
            let id = record
                .as_table()
                .and_then(|v| v.get("id"))
                .and_then(toml::Value::as_str)
                .ok_or(ExternalLineageRegistryError::InvalidSchema)?;
            record_ids.insert(
                SourceRecordId::parse(id)
                    .map_err(|_| ExternalLineageRegistryError::InvalidSchema)?,
            );
        }
        let lineages = table
            .get("lineages")
            .and_then(toml::Value::as_array)
            .ok_or(ExternalLineageRegistryError::InvalidSchema)?;
        if lineages.len() > MAX_LINEAGES {
            return Err(ExternalLineageRegistryError::TooLarge);
        }
        for lineage in lineages {
            let lineage = lineage
                .as_table()
                .ok_or(ExternalLineageRegistryError::InvalidSchema)?;
            let allowed = [
                "id",
                "source_record",
                "adapter",
                "adapter_version",
                "derived_test_id",
                "description",
                "transformation",
                "reference_file",
                "test_artifact_sha256",
                "reference_artifact_sha256",
            ];
            if lineage.keys().any(|key| !allowed.contains(&key.as_str())) {
                return Err(ExternalLineageRegistryError::InvalidSchema);
            }
            let get = |key: &str| {
                lineage
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .ok_or(ExternalLineageRegistryError::InvalidSchema)
            };
            let id = ExternalLineageId::parse(get("id")?)
                .map_err(|_| ExternalLineageRegistryError::InvalidSchema)?;
            let source_record = SourceRecordId::parse(get("source_record")?)
                .map_err(|_| ExternalLineageRegistryError::InvalidSchema)?;
            if !record_ids.contains(&source_record) {
                return Err(ExternalLineageRegistryError::DanglingSourceRecord);
            }
            let declaration = ExternalLineageDeclaration {
                id: id.clone(),
                source_record,
                adapter: HarnessFeatureId::parse(get("adapter")?)
                    .map_err(|_| ExternalLineageRegistryError::InvalidSchema)?,
                adapter_version: ExternalAdapterVersion::parse(get("adapter_version")?)
                    .map_err(|_| ExternalLineageRegistryError::InvalidSchema)?,
                derived_test_id: TestId::parse(get("derived_test_id")?)
                    .map_err(|_| ExternalLineageRegistryError::InvalidSchema)?,
            };
            if declarations.insert(id, declaration).is_some() {
                return Err(ExternalLineageRegistryError::DuplicateLineage);
            }
        }
    }
    Ok(ValidatedExternalLineageRegistry { declarations })
}

pub fn reconcile_external_fixture_lineages(
    inventory: &ValidatedInventory,
    registry: &ValidatedExternalLineageRegistry,
) -> Result<(), ExternalLineageRegistryError> {
    for fixture in inventory.fixtures() {
        let FixtureSource::ExternalDerived {
            lineage_id,
            adapter,
            adapter_version,
        } = fixture.source()
        else {
            continue;
        };
        let declaration = registry
            .get(lineage_id)
            .ok_or(ExternalLineageRegistryError::DanglingFixtureLineage)?;
        if declaration.derived_test_id() != fixture.id() {
            return Err(ExternalLineageRegistryError::DerivedTestMismatch);
        }
        if declaration.adapter() != adapter || declaration.adapter_version() != adapter_version {
            return Err(ExternalLineageRegistryError::AdapterMismatch);
        }
    }
    Ok(())
}

fn read_confined(
    root: &Path,
    relative: &Path,
    maximum: u64,
) -> Result<Vec<u8>, ExternalLineageRegistryError> {
    read_confined_regular_file(root, relative, maximum).map_err(|error| match error {
        ConfinedFileError::TooLarge => ExternalLineageRegistryError::TooLarge,
        ConfinedFileError::Io | ConfinedFileError::Missing => ExternalLineageRegistryError::Io,
        _ => ExternalLineageRegistryError::InvalidPath,
    })
}

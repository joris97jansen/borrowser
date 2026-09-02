use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use external_test_provenance::{
    Attribution, ConfinedFileError, ExternalFileIdentity, ImmutableRevision, LicenseIdentifier,
    LicenseNotice, Sha256Digest, UpstreamPath, UpstreamProjectId, read_confined_regular_file,
    sha256,
};
use serde::Deserialize;

use crate::model::{
    DerivedFixtureAdaptation, DerivedFixtureAdapter, DerivedFixtureLineage,
    WPT_MAX_CLOSURE_FILES_PER_RECORD, WPT_MAX_FILE_BYTES, WPT_MAX_SOURCE_FILES,
    WPT_MAX_SOURCE_RECORDS, WPT_MAX_TOTAL_BYTES, WptAdaptationTransformation, WptFileRole,
    WptSourceFile,
};
use conformance_test_support::{
    ExternalAdapterVersion, ExternalLineageId, HarnessFeatureId, SourceRecordId, TestId,
};

pub const WPT_SOURCE_SET_FORMAT_V1: &str = "borrowser-external-source-set-v1";
pub const EXTERNAL_LINEAGE_REGISTRY_FORMAT_V1: &str = "borrowser-external-lineage-registry-v1";
pub const WPT_SOURCE_SET_PATH: &str = "tests/conformance/external/wpt/sources.toml";
const MAX_REGISTRY_BYTES: u64 = 256 * 1024;

#[derive(Debug)]
pub struct ValidatedWptSourceSet {
    source_set: String,
    project: UpstreamProjectId,
    revision: ImmutableRevision,
    license: LicenseIdentifier,
    license_notice: LicenseNotice,
    license_notice_sha256: Sha256Digest,
    attribution: Attribution,
    files: Vec<WptSourceFile>,
    records: Vec<WptSourceRecord>,
    lineages: Vec<DerivedFixtureLineage>,
}
impl ValidatedWptSourceSet {
    pub fn source_set(&self) -> &str {
        &self.source_set
    }
    pub fn project(&self) -> &UpstreamProjectId {
        &self.project
    }
    pub fn revision(&self) -> &ImmutableRevision {
        &self.revision
    }
    pub fn license(&self) -> &LicenseIdentifier {
        &self.license
    }
    pub fn license_notice(&self) -> &LicenseNotice {
        &self.license_notice
    }
    pub fn license_notice_sha256(&self) -> Sha256Digest {
        self.license_notice_sha256
    }
    pub fn attribution(&self) -> &Attribution {
        &self.attribution
    }
    pub fn files(&self) -> &[WptSourceFile] {
        &self.files
    }
    pub fn records(&self) -> &[WptSourceRecord] {
        &self.records
    }
    pub fn lineages(&self) -> &[DerivedFixtureLineage] {
        &self.lineages
    }
    pub fn file_by_id(&self, id: &str) -> Option<&WptSourceFile> {
        self.files.iter().find(|file| file.id() == id)
    }
    pub fn file_by_path(&self, path: &UpstreamPath) -> Option<&WptSourceFile> {
        self.files
            .iter()
            .find(|file| file.identity().path() == path)
    }
    pub fn record(&self, id: &SourceRecordId) -> Option<&WptSourceRecord> {
        self.records.iter().find(|record| record.id() == id)
    }
}

#[derive(Debug)]
pub struct WptSourceRecord {
    id: SourceRecordId,
    source_file_id: String,
}
impl WptSourceRecord {
    pub fn id(&self) -> &SourceRecordId {
        &self.id
    }
    pub fn source_file_id(&self) -> &str {
        &self.source_file_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptRegistryError {
    Io,
    TooLarge,
    InvalidSchema,
    UnsupportedVersion,
    InvalidIdentity,
    DuplicateId,
    UnknownParent,
    PopulationBoundExceeded,
    ClosureBoundExceeded,
    InvalidLicenseNotice,
    MissingMaterializedFile,
    Symlink,
    NonRegularFile,
    HashMismatch,
    FileTooLarge,
    TotalBytesExceeded,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    lineage_registry_format: String,
    source_set: String,
    upstream_project: String,
    revision: String,
    license: String,
    license_notice_path: String,
    license_notice_sha256: String,
    attribution: String,
    files: Vec<FileWire>,
    records: Vec<RecordWire>,
    lineages: Vec<LineageWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileWire {
    id: String,
    path: String,
    sha256: String,
    role: String,
    parents: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordWire {
    id: String,
    source_file: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineageWire {
    id: String,
    source_record: String,
    adapter: String,
    adapter_version: String,
    derived_test_id: String,
    description: String,
    transformation: String,
    reference_file: String,
    test_artifact_sha256: String,
    reference_artifact_sha256: String,
}

pub fn load_wpt_source_set(
    repository_root: &Path,
) -> Result<ValidatedWptSourceSet, WptRegistryError> {
    let bytes = read_confined_regular_file(
        repository_root,
        Path::new(WPT_SOURCE_SET_PATH),
        MAX_REGISTRY_BYTES,
    )
    .map_err(map_confined_error)?;
    let wire: Wire = toml::from_slice(&bytes).map_err(|_| WptRegistryError::InvalidSchema)?;
    if wire.format != WPT_SOURCE_SET_FORMAT_V1 {
        return Err(WptRegistryError::UnsupportedVersion);
    }
    if wire.lineage_registry_format != EXTERNAL_LINEAGE_REGISTRY_FORMAT_V1 {
        return Err(WptRegistryError::UnsupportedVersion);
    }
    let project = UpstreamProjectId::parse(&wire.upstream_project)
        .map_err(|_| WptRegistryError::InvalidIdentity)?;
    let revision = ImmutableRevision::parse_git_commit(&wire.revision)
        .map_err(|_| WptRegistryError::InvalidIdentity)?;
    let license =
        LicenseIdentifier::parse(&wire.license).map_err(|_| WptRegistryError::InvalidIdentity)?;
    let notice_bytes = read_confined_regular_file(
        repository_root,
        Path::new(&wire.license_notice_path),
        MAX_REGISTRY_BYTES,
    )
    .map_err(|_| WptRegistryError::InvalidLicenseNotice)?;
    if notice_bytes.is_empty() {
        return Err(WptRegistryError::InvalidLicenseNotice);
    }
    let notice_sha256 = Sha256Digest::parse(&wire.license_notice_sha256)
        .map_err(|_| WptRegistryError::InvalidLicenseNotice)?;
    if sha256(&notice_bytes) != notice_sha256 {
        return Err(WptRegistryError::InvalidLicenseNotice);
    }
    let notice = LicenseNotice::parse(&wire.license_notice_path)
        .map_err(|_| WptRegistryError::InvalidLicenseNotice)?;
    let attribution =
        Attribution::parse(&wire.attribution).map_err(|_| WptRegistryError::InvalidIdentity)?;
    if wire.files.is_empty()
        || wire.records.is_empty()
        || wire.files.len() > WPT_MAX_SOURCE_FILES
        || wire.records.len() > WPT_MAX_SOURCE_RECORDS
    {
        return Err(WptRegistryError::PopulationBoundExceeded);
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut files = Vec::new();
    for value in wire.files {
        if !ids.insert(value.id.clone()) {
            return Err(WptRegistryError::DuplicateId);
        }
        let path =
            UpstreamPath::parse(&value.path).map_err(|_| WptRegistryError::InvalidIdentity)?;
        if !paths.insert(path.clone()) {
            return Err(WptRegistryError::DuplicateId);
        }
        let digest =
            Sha256Digest::parse(&value.sha256).map_err(|_| WptRegistryError::InvalidIdentity)?;
        let role = match value.role.as_str() {
            "accounted-source" => WptFileRole::AccountedSource,
            "reference-node" => WptFileRole::ReferenceNode,
            "static-resource" => WptFileRole::StaticResource,
            _ => return Err(WptRegistryError::InvalidSchema),
        };
        let mut parents = value
            .parents
            .into_iter()
            .map(|v| SourceRecordId::parse(&v).map_err(|_| WptRegistryError::InvalidIdentity))
            .collect::<Result<Vec<_>, _>>()?;
        parents.sort();
        parents.dedup();
        let local_path = format!(
            "tests/conformance/external/wpt/raw/{}/{}",
            revision.as_str(),
            path.as_str()
        );
        files.push(WptSourceFile::new(
            value.id,
            ExternalFileIdentity::new(project.clone(), revision.clone(), path, digest),
            local_path,
            role,
            parents,
        ));
    }
    let mut records = Vec::new();
    let mut record_ids = BTreeSet::new();
    for value in wire.records {
        let id = SourceRecordId::parse(&value.id).map_err(|_| WptRegistryError::InvalidIdentity)?;
        if !record_ids.insert(id.clone()) {
            return Err(WptRegistryError::DuplicateId);
        }
        let source = files
            .iter()
            .find(|file| file.id() == value.source_file)
            .ok_or(WptRegistryError::InvalidSchema)?;
        if source.role() != WptFileRole::AccountedSource || !source.parents().contains(&id) {
            return Err(WptRegistryError::InvalidSchema);
        }
        records.push(WptSourceRecord {
            id,
            source_file_id: value.source_file,
        });
    }
    for file in &files {
        if file
            .parents()
            .iter()
            .any(|parent| !record_ids.contains(parent))
        {
            return Err(WptRegistryError::UnknownParent);
        }
    }
    for record in &records {
        let closure_count = files
            .iter()
            .filter(|file| {
                file.parents().contains(record.id()) && file.role() != WptFileRole::AccountedSource
            })
            .count();
        if closure_count > WPT_MAX_CLOSURE_FILES_PER_RECORD {
            return Err(WptRegistryError::ClosureBoundExceeded);
        }
    }
    let mut lineage_ids = BTreeSet::new();
    let mut lineages = Vec::new();
    for value in wire.lineages {
        let id =
            ExternalLineageId::parse(&value.id).map_err(|_| WptRegistryError::InvalidIdentity)?;
        let source_record = SourceRecordId::parse(&value.source_record)
            .map_err(|_| WptRegistryError::InvalidIdentity)?;
        let adapter = HarnessFeatureId::parse(&value.adapter)
            .map_err(|_| WptRegistryError::InvalidIdentity)?;
        let adapter_version = ExternalAdapterVersion::parse(&value.adapter_version)
            .map_err(|_| WptRegistryError::InvalidIdentity)?;
        let derived_test_id =
            TestId::parse(&value.derived_test_id).map_err(|_| WptRegistryError::InvalidIdentity)?;
        let test_artifact_sha256 = Sha256Digest::parse(&value.test_artifact_sha256)
            .map_err(|_| WptRegistryError::InvalidIdentity)?;
        let reference_artifact_sha256 = Sha256Digest::parse(&value.reference_artifact_sha256)
            .map_err(|_| WptRegistryError::InvalidIdentity)?;
        let transformation = match value.transformation.as_str() {
            "exact-copy-v1" => WptAdaptationTransformation::ExactCopyV1,
            _ => return Err(WptRegistryError::InvalidSchema),
        };
        let source_file = records
            .iter()
            .find(|record| record.id() == &source_record)
            .and_then(|record| {
                files
                    .iter()
                    .find(|file| file.id() == record.source_file_id())
            })
            .ok_or(WptRegistryError::InvalidSchema)?;
        let reference_file = files
            .iter()
            .find(|file| file.id() == value.reference_file)
            .ok_or(WptRegistryError::InvalidSchema)?;
        if !lineage_ids.insert(id.clone())
            || !record_ids.contains(&source_record)
            || !reference_file.parents().contains(&source_record)
            || reference_file.role() != WptFileRole::ReferenceNode
            || source_file.identity().sha256() != test_artifact_sha256
            || reference_file.identity().sha256() != reference_artifact_sha256
        {
            return Err(WptRegistryError::InvalidSchema);
        }
        lineages.push(DerivedFixtureLineage::new(
            id,
            source_record,
            DerivedFixtureAdapter::new(adapter, adapter_version, derived_test_id),
            DerivedFixtureAdaptation::new(
                value.description,
                transformation,
                value.reference_file,
                test_artifact_sha256,
                reference_artifact_sha256,
            ),
        ));
    }
    files.sort_by(|a, b| a.identity().path().cmp(b.identity().path()));
    records.sort_by(|a, b| a.id.cmp(&b.id));
    lineages.sort_by(|a, b| a.id().cmp(b.id()));
    Ok(ValidatedWptSourceSet {
        source_set: wire.source_set,
        project,
        revision,
        license,
        license_notice: notice,
        license_notice_sha256: notice_sha256,
        attribution,
        files,
        records,
        lineages,
    })
}

fn map_confined_error(error: ConfinedFileError) -> WptRegistryError {
    match error {
        ConfinedFileError::TooLarge => WptRegistryError::TooLarge,
        ConfinedFileError::Io | ConfinedFileError::Missing => WptRegistryError::Io,
        ConfinedFileError::Symlink => WptRegistryError::Symlink,
        _ => WptRegistryError::InvalidIdentity,
    }
}

pub fn validate_materialized_sources(
    repository_root: &Path,
    set: &ValidatedWptSourceSet,
) -> Result<(), WptRegistryError> {
    let mut total = 0_u64;
    for file in &set.files {
        let path = repository_root.join(file.local_path());
        reject_symlink_chain(repository_root, &path)?;
        let metadata =
            fs::metadata(&path).map_err(|_| WptRegistryError::MissingMaterializedFile)?;
        if !metadata.is_file() {
            return Err(WptRegistryError::NonRegularFile);
        }
        if metadata.len() > WPT_MAX_FILE_BYTES {
            return Err(WptRegistryError::FileTooLarge);
        }
        total = total
            .checked_add(metadata.len())
            .ok_or(WptRegistryError::TotalBytesExceeded)?;
        if total > WPT_MAX_TOTAL_BYTES {
            return Err(WptRegistryError::TotalBytesExceeded);
        }
        let bytes = fs::read(&path).map_err(|_| WptRegistryError::Io)?;
        if sha256(&bytes) != file.identity().sha256() {
            return Err(WptRegistryError::HashMismatch);
        }
    }
    Ok(())
}

pub fn read_declared_file(
    repository_root: &Path,
    file: &WptSourceFile,
) -> Result<Vec<u8>, WptRegistryError> {
    let path = repository_root.join(file.local_path());
    reject_symlink_chain(repository_root, &path)?;
    let metadata = fs::metadata(&path).map_err(|_| WptRegistryError::MissingMaterializedFile)?;
    if !metadata.is_file() {
        return Err(WptRegistryError::NonRegularFile);
    }
    if metadata.len() > WPT_MAX_FILE_BYTES {
        return Err(WptRegistryError::FileTooLarge);
    }
    let bytes = fs::read(path).map_err(|_| WptRegistryError::Io)?;
    if sha256(&bytes) != file.identity().sha256() {
        return Err(WptRegistryError::HashMismatch);
    }
    Ok(bytes)
}

fn reject_symlink_chain(root: &Path, path: &Path) -> Result<(), WptRegistryError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| WptRegistryError::InvalidIdentity)?;
    let mut current = PathBuf::from(root);
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err(WptRegistryError::InvalidIdentity);
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(WptRegistryError::Symlink);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(WptRegistryError::Io),
        }
    }
    Ok(())
}

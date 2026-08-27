use std::fmt::Write;

use crate::diagnostic::InventoryErrors;
use crate::discovery::{InventoryRepository, discover_inventory};
use crate::model::{
    InventoryScope, ObservationSurface, ReferenceKind, RepositoryPath, SourceKind, TestId,
    ValidatedInventory,
};

pub const CONFORMANCE_MANIFEST_FORMAT_V1: &str = "borrowser-conformance-manifest-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConformanceManifest {
    entries: Vec<ManifestEntry>,
}

impl ConformanceManifest {
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestEntry {
    id: TestId,
    fixture_path: RepositoryPath,
    test_path: RepositoryPath,
    metadata_path: RepositoryPath,
    scope: InventoryScope,
    observation: ObservationSurface,
    source_kind: SourceKind,
    reference: Option<ManifestReference>,
}

impl ManifestEntry {
    pub fn id(&self) -> &TestId {
        &self.id
    }

    pub fn fixture_path(&self) -> &RepositoryPath {
        &self.fixture_path
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestReference {
    kind: ReferenceKind,
    path: RepositoryPath,
}

pub fn build_manifest(inventory: &ValidatedInventory) -> ConformanceManifest {
    let mut entries = inventory
        .fixtures()
        .iter()
        .map(|fixture| ManifestEntry {
            id: fixture.id().clone(),
            fixture_path: fixture.fixture_path().clone(),
            test_path: fixture.test_path().clone(),
            metadata_path: fixture.metadata_path().clone(),
            scope: fixture.scope(),
            observation: fixture.observation(),
            source_kind: fixture.source_kind(),
            reference: fixture.reference().map(|reference| ManifestReference {
                kind: reference.kind(),
                path: reference.path().clone(),
            }),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    ConformanceManifest { entries }
}

pub fn generate_manifest_bytes(
    repository: &InventoryRepository,
) -> Result<Vec<u8>, InventoryErrors> {
    let inventory = discover_inventory(repository)?;
    Ok(serialize_manifest(&build_manifest(&inventory)))
}

pub fn serialize_manifest(manifest: &ConformanceManifest) -> Vec<u8> {
    let mut output = String::new();
    write_field(&mut output, "format", CONFORMANCE_MANIFEST_FORMAT_V1);
    for entry in &manifest.entries {
        output.push_str("\n[[tests]]\n");
        write_field(&mut output, "id", entry.id.as_str());
        write_field(&mut output, "fixture_path", entry.fixture_path.as_str());
        write_field(&mut output, "test_path", entry.test_path.as_str());
        write_field(&mut output, "metadata_path", entry.metadata_path.as_str());
        write_field(&mut output, "scope", entry.scope.as_str());
        write_field(&mut output, "observation", entry.observation.as_str());
        write_field(&mut output, "source_kind", entry.source_kind.as_str());
        if let Some(reference) = &entry.reference {
            write_field(&mut output, "reference_kind", reference.kind.as_str());
            write_field(&mut output, "reference_path", reference.path.as_str());
        }
    }
    output.into_bytes()
}

fn write_field(output: &mut String, key: &str, value: &str) {
    let encoded = encode_toml_string(value);
    writeln!(output, "{key} = {encoded}").expect("writing to String cannot fail");
}

fn encode_toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_library_encodes_arbitrary_string_scalars() {
        for value in [
            "contains \"quotes\"",
            "contains \\ backslashes",
            "Unicode: café 東京",
            "contains\ta tab",
            "contains\na newline",
        ] {
            let encoded = encode_toml_string(value);
            let decoded = encoded.parse::<toml::Value>().expect("valid TOML scalar");
            assert_eq!(decoded.as_str(), Some(value));
            assert_eq!(encode_toml_string(value), encoded);
        }
    }
}

//! Same-object verification of explicitly supplied source manifests. Stage 0 makes no committed-source claim.
use crate::{CaptureError as E, Result, limits::CONFIG_BYTES, wire};
use external_test_provenance::sha256;
use serde::Deserialize;
use std::path::Path;
mod reviewed {
    include!("source_set.rs");
}

#[derive(Clone, Copy)]
pub enum SourceSet {
    Collector,
    Qualification,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    #[serde(deserialize_with = "wire::sources")]
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    sha256: String,
}

pub fn verify_manifest(root: &Path, path: &str, expected: &str, set: SourceSet) -> Result<()> {
    let bytes = wire::read(root, path, CONFIG_BYTES)?;
    if sha256(&bytes).to_string() != expected {
        return Err(E::Digest);
    }
    let manifest: Manifest = wire::parse(&bytes, CONFIG_BYTES)?;
    if manifest.format != "borrowser-capture-source-manifest-v1"
        || manifest.entries.is_empty()
        || manifest.entries.len() > 128
    {
        return Err(E::Source);
    }
    let mut writer = wire::Writer::new(CONFIG_BYTES);
    writer.field("format", &manifest.format)?;
    let mut previous = "";
    let mut total = 0usize;
    let reviewed = match set {
        SourceSet::Collector => reviewed::COLLECTOR,
        SourceSet::Qualification => reviewed::QUALIFICATION,
    };
    if manifest.entries.len() != reviewed.len() {
        return Err(E::Source);
    }
    for (entry, (expected_path, expected_bytes)) in manifest.entries.iter().zip(reviewed) {
        wire::path(&entry.path)?;
        wire::digest(&entry.sha256)?;
        if entry.path.as_str() <= previous {
            return Err(E::Source);
        }
        previous = &entry.path;
        writer.raw("[[entries]]\n")?;
        writer.field("path", &entry.path)?;
        writer.field("sha256", &entry.sha256)?;
        let source = wire::read(root, &entry.path, 1_048_576)?;
        total = total.checked_add(source.len()).ok_or(E::Limit)?;
        if total > 4 * 1_048_576 {
            return Err(E::Limit);
        }
        if entry.path != *expected_path
            || source != *expected_bytes
            || sha256(&source).to_string() != entry.sha256
        {
            return Err(E::Source);
        }
    }
    if writer.finish() != bytes {
        return Err(E::NonCanonical);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collector_membership_includes_private_transaction() {
        assert!(reviewed::COLLECTOR.iter().any(|(path, bytes)| *path
            == "crates/external_browser_capture/src/transaction.rs"
            && *bytes == include_bytes!("transaction.rs")));
        assert!(
            !reviewed::QUALIFICATION
                .iter()
                .any(|(path, _)| *path == "crates/external_browser_capture/src/transaction.rs")
        );
    }
    #[test]
    fn checked_in_manifests_match_the_exact_compiled_source_sets() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for (path, set) in [
            (
                "tools/conformance/static-dom-capture-source-manifest-v1.toml",
                SourceSet::Collector,
            ),
            (
                "tools/conformance/static-dom-qualification-source-manifest-v1.toml",
                SourceSet::Qualification,
            ),
        ] {
            let bytes = wire::read(root, path, CONFIG_BYTES).unwrap();
            verify_manifest(root, path, &sha256(&bytes).to_string(), set).unwrap();
        }
    }
    #[test]
    fn exact_source_membership_and_bytes_are_required() {
        let root = tempfile::tempdir().unwrap();
        let mut writer = wire::Writer::new(CONFIG_BYTES);
        writer
            .field("format", &"borrowser-capture-source-manifest-v1")
            .unwrap();
        for (path, bytes) in reviewed::QUALIFICATION {
            let destination = root.path().join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(destination, bytes).unwrap();
            writer.raw("[[entries]]\n").unwrap();
            writer.field("path", path).unwrap();
            writer.field("sha256", &sha256(bytes).to_string()).unwrap();
        }
        let manifest = writer.finish();
        let digest = sha256(&manifest).to_string();
        std::fs::write(root.path().join("manifest.toml"), &manifest).unwrap();
        assert_eq!(
            verify_manifest(
                root.path(),
                "manifest.toml",
                &digest,
                SourceSet::Qualification
            ),
            Ok(())
        );
        assert_eq!(
            verify_manifest(root.path(), "manifest.toml", &digest, SourceSet::Collector),
            Err(E::Source)
        );
        std::fs::write(root.path().join(reviewed::QUALIFICATION[0].0), b"changed").unwrap();
        assert_eq!(
            verify_manifest(
                root.path(),
                "manifest.toml",
                &digest,
                SourceSet::Qualification
            ),
            Err(E::Source)
        );
    }
}

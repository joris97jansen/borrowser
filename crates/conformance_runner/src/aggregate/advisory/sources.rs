use super::model::AdvisoryComparisonFailure as Failure;
use external_test_provenance::{
    ExternalCaptureProvenanceV1, SameObjectConfinedReadError, Sha256Digest,
    read_confined_regular_file_same_object, sha256,
};
use std::path::Path;

pub const CAPTURE_ALGORITHM_PATH_V1: &str = "tools/conformance/web-observable-dom-tree-v1.mjs";
pub const CAPTURE_CONFIGURATION_PATH_V1: &str =
    "tools/conformance/web-observable-dom-tree-v1.config.json";
pub const MAX_CAPTURE_SOURCE_BYTES_V1: u64 = 65_536;
const ALGORITHM: &[u8] =
    include_bytes!("../../../../../tools/conformance/web-observable-dom-tree-v1.mjs");
const CONFIG: &[u8] =
    include_bytes!("../../../../../tools/conformance/web-observable-dom-tree-v1.config.json");

#[derive(Debug)]
pub enum CaptureSourceError {
    Read {
        path: &'static str,
        error: SameObjectConfinedReadError,
    },
    UnrecognizedSource {
        path: &'static str,
    },
}
/// Reviewed source identities, calculated from exact bounded raw bytes. Does
/// not assert that a real browser capture had an acceptable historical context.
#[derive(Debug)]
pub struct VerifiedCaptureSourcesV1 {
    algorithm: Sha256Digest,
    configuration: Sha256Digest,
}
impl VerifiedCaptureSourcesV1 {
    pub fn load(root: &Path) -> Result<Self, CaptureSourceError> {
        let algorithm = read_source(root, CAPTURE_ALGORITHM_PATH_V1)?;
        let configuration = read_source(root, CAPTURE_CONFIGURATION_PATH_V1)?;
        // Algorithm/version has one reviewed implementation in this release.
        // A changed source must be reviewed/rebuilt, not relabelled as version 1.
        if algorithm != ALGORITHM {
            return Err(CaptureSourceError::UnrecognizedSource {
                path: CAPTURE_ALGORITHM_PATH_V1,
            });
        }
        if configuration != CONFIG {
            return Err(CaptureSourceError::UnrecognizedSource {
                path: CAPTURE_CONFIGURATION_PATH_V1,
            });
        }
        Ok(Self {
            algorithm: sha256(&algorithm),
            configuration: sha256(&configuration),
        })
    }
    pub const fn algorithm_sha256(&self) -> Sha256Digest {
        self.algorithm
    }
    pub const fn configuration_sha256(&self) -> Sha256Digest {
        self.configuration
    }
    pub(super) fn verify(&self, provenance: &ExternalCaptureProvenanceV1) -> Result<(), Failure> {
        if provenance.capture_algorithm().as_str() != "web-observable-dom-tree-v1-inspector"
            || provenance.capture_algorithm_version().as_str() != "1"
        {
            return Err(Failure::SourceIdentityMismatch);
        }
        if provenance.capture_algorithm_source_sha256() != self.algorithm {
            return Err(Failure::AlgorithmSourceMismatch);
        }
        if provenance.capture_configuration_sha256() != self.configuration {
            return Err(Failure::ConfigurationSourceMismatch);
        }
        Ok(())
    }
}
fn read_source(root: &Path, path: &'static str) -> Result<Vec<u8>, CaptureSourceError> {
    read_confined_regular_file_same_object(root, Path::new(path), MAX_CAPTURE_SOURCE_BYTES_V1)
        .map_err(|error| CaptureSourceError::Read { path, error })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sources_are_raw_bounded_and_reviewed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let sources = VerifiedCaptureSourcesV1::load(&root).unwrap();
        assert_eq!(sources.algorithm_sha256(), sha256(ALGORITHM));
        assert_eq!(sources.configuration_sha256(), sha256(CONFIG));
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("tools/conformance")).unwrap();
        for (path, bytes) in [
            (CAPTURE_ALGORITHM_PATH_V1, ALGORITHM),
            (CAPTURE_CONFIGURATION_PATH_V1, CONFIG),
        ] {
            std::fs::write(tmp.path().join(path), bytes).unwrap();
        }
        std::fs::write(
            tmp.path().join(CAPTURE_CONFIGURATION_PATH_V1),
            String::from_utf8(CONFIG.to_vec())
                .unwrap()
                .replace('\n', "\r\n"),
        )
        .unwrap();
        assert!(matches!(
            VerifiedCaptureSourcesV1::load(tmp.path()),
            Err(CaptureSourceError::UnrecognizedSource { .. })
        ));
        for path in [CAPTURE_ALGORITHM_PATH_V1, CAPTURE_CONFIGURATION_PATH_V1] {
            std::fs::write(tmp.path().join(path), vec![b'a'; 65_536]).unwrap();
            assert_eq!(read_source(tmp.path(), path).unwrap().len(), 65_536);
            std::fs::write(tmp.path().join(path), vec![b'a'; 65_537]).unwrap();
            assert!(read_source(tmp.path(), path).is_err());
        }
    }
}

use std::path::Path;

use external_test_provenance::{Sha256Digest, read_confined_regular_file_same_object, sha256};

use super::super::baseline::{BASELINE_MAX_BYTES_V1, decode_baseline_v1};
use super::{ConformanceTrendV1, TrendError, TrendInputSide, build_trend_v1, compare_baselines_v1};

#[derive(Clone, Copy)]
pub struct BaselineFileInputV1<'a> {
    pub root: &'a Path,
    pub relative_path: &'a Path,
    pub expected_sha256: Sha256Digest,
}

pub fn compare_baseline_files_v1(
    old: BaselineFileInputV1<'_>,
    new: BaselineFileInputV1<'_>,
) -> Result<ConformanceTrendV1, TrendError> {
    let old_bytes = read(old, TrendInputSide::Old)?;
    if sha256(&old_bytes) != old.expected_sha256 {
        return Err(TrendError::DigestMismatch {
            input: TrendInputSide::Old,
        });
    }
    let new_bytes = read(new, TrendInputSide::New)?;
    if sha256(&new_bytes) != new.expected_sha256 {
        return Err(TrendError::DigestMismatch {
            input: TrendInputSide::New,
        });
    }
    let old = decode_baseline_v1(&old_bytes).map_err(|source| TrendError::InvalidBaseline {
        input: TrendInputSide::Old,
        source,
    })?;
    let new = decode_baseline_v1(&new_bytes).map_err(|source| TrendError::InvalidBaseline {
        input: TrendInputSide::New,
        source,
    })?;
    compare_baselines_v1(&old, &new)
}

pub fn compare_baseline_files_and_build_trend_v1(
    old: BaselineFileInputV1<'_>,
    new: BaselineFileInputV1<'_>,
) -> Result<Vec<u8>, TrendError> {
    build_trend_v1(&compare_baseline_files_v1(old, new)?)
}

fn read(input: BaselineFileInputV1<'_>, side: TrendInputSide) -> Result<Vec<u8>, TrendError> {
    read_confined_regular_file_same_object(
        input.root,
        input.relative_path,
        u64::try_from(BASELINE_MAX_BYTES_V1).map_err(|_| TrendError::Arithmetic)?,
    )
    .map_err(|source| TrendError::Input {
        input: side,
        source,
    })
}

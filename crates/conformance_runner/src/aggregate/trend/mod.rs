mod compare;
#[cfg(test)]
mod decode;
mod encode;
mod fingerprint;
mod input;
mod model;

pub(crate) use compare::compare_baselines_v1;
pub use encode::{TREND_VERSIONS_V1, build_and_write_trend_v1, build_trend_v1};
pub use input::{
    BaselineFileInputV1, compare_baseline_files_and_build_trend_v1, compare_baseline_files_v1,
};
pub use model::*;

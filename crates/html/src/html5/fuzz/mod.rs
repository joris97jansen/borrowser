mod config;
mod digest;
mod driver;
#[cfg(any(test, feature = "dom-snapshot"))]
mod regression;

#[cfg(test)]
mod tests;

pub use config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzError, Html5PipelineFuzzSummary,
    Html5PipelineFuzzTermination, derive_html5_pipeline_fuzz_seed,
};
pub use driver::run_seeded_html5_pipeline_fuzz_case;
#[cfg(any(test, feature = "dom-snapshot"))]
pub use regression::{Html5PipelineRegressionError, render_html5_pipeline_regression_snapshot};

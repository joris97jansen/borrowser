//! Bounded WPT-authored source interpretation for Borrowser conformance tooling.

mod accounting;
mod html_metadata;
mod interpret;
mod materialize;
mod model;
mod registry;
mod selection_policy;
mod source_metadata;
mod summary;

pub use accounting::{
    WptAccountingError, account_wpt_source_set, directly_selected_records,
    selected_derived_adaptations,
};
pub use html_metadata::HtmlMetadataError;
pub use interpret::{WptInterpretationError, interpret_wpt_record, interpret_wpt_source_set};
pub use materialize::{WptMaterializationError, materialize_wpt_source_set};
pub use model::*;
pub use registry::{
    ValidatedWptSourceSet, WptRegistryError, WptSourceRecord, load_wpt_source_set,
    read_declared_file, validate_materialized_sources,
};
pub use selection_policy::{
    ValidatedWptSelectionPolicy, WPT_SELECTION_POLICY_FORMAT_V1, WPT_SELECTION_POLICY_PATH,
    WptSelectionPolicyError, evaluate_wpt_filter, load_wpt_selection_policy,
};
pub use source_metadata::{
    ValidatedWptSourceMetadata, WPT_SOURCE_METADATA_FORMAT_V1, WPT_SOURCE_METADATA_PATH,
    WptSourceMetadataError, load_wpt_source_metadata,
};
pub use summary::{
    WPT_IMPORT_SUMMARY_FORMAT_V1, WPT_IMPORT_SUMMARY_PATH, WptAccountingSummary, WptSummaryCheck,
    WptSummaryError, check_repository_wpt_summary, generate_repository_wpt_summary,
    serialize_wpt_summary, update_repository_wpt_summary,
};

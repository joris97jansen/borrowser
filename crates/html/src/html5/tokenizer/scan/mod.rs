mod classify;
mod end_tag;
mod script;
mod shared;

pub(crate) use classify::{
    is_attribute_name_stop, is_html_space, is_html_space_byte, is_tag_name_stop,
    is_unquoted_attr_value_stop,
};
pub(crate) use end_tag::{IncrementalEndTagMatch, IncrementalEndTagMatcher};
pub(crate) use script::{ScriptTagBoundaryMatch, match_script_tag_boundary_at};
pub(crate) use shared::{DoctypeKeywordKind, QuotedParse, match_ascii_prefix_ci_at};

mod drive;
mod early_modes;
mod form_controls;
mod in_body;
mod select;
mod start_tag;
mod template;

pub(in crate::html5::tree_builder) use drive::DispatchOutcome;
pub(in crate::html5::tree_builder) use start_tag::SelfClosingFlagDisposition;

use crate::html5::shared::AtomId;
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::modes::InsertionMode;

/// Parser-algorithm outcome for the original tokenizer self-closing flag.
/// Tree-construction handlers do not own this finalization step; dispatch does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum SelfClosingFlagDisposition {
    Acknowledge,
    LeaveUnacknowledged,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn finalize_html_start_tag_self_closing_flag(
        &mut self,
        name: AtomId,
        self_closing: bool,
        disposition: SelfClosingFlagDisposition,
        processed_insertion_mode: InsertionMode,
    ) {
        if self_closing && disposition == SelfClosingFlagDisposition::LeaveUnacknowledged {
            self.record_parse_error(
                "non-void-html-element-start-tag-with-trailing-solidus",
                Some(name),
                Some(processed_insertion_mode),
            );
        }
    }
}

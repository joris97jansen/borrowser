use crate::Node;
use crate::dom_patch::DomPatch;

use super::types::{HtmlParseCounters, HtmlParseEvent};

/// Final parse result returned by [`parse_document`] or [`HtmlParser::into_output`].
#[derive(Debug)]
pub struct ParseOutput {
    pub document: Node,
    /// Patches drained by `into_output()`.
    ///
    /// For `parse_document(...)`, this is the full emitted patch history because
    /// no earlier draining is possible. For streaming use, if the caller has
    /// already consumed patches via `take_patches()` or `take_patch_batch()`,
    /// this contains only the undrained remainder.
    pub patches: Vec<DomPatch>,
    /// True when `patches` contains the full session patch history.
    pub contains_full_patch_history: bool,
    pub counters: HtmlParseCounters,
    pub parse_errors: Vec<HtmlParseEvent>,
}

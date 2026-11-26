//! Tab-level orchestration and streaming state.
//!
//! Invariants:
//! - Active request identity comes exclusively from the installed FetchRequest.
//!   A checked Tab-owned counter reserves every navigation identity exactly once.
//!   Pending navigation events are routed independently until response admission;
//!   only the active request can publish document/parser/subresource events.
//! - Loading combines parser completion with pending stylesheet work. Network
//!   completion and intermediate publications do not establish parser success.
//!   Document failure stays latched until navigation replaces the document.
//! - Each navigation accepts one HTML response start, after redirects resolve.
//!   Duplicate starts preserve streaming state; late HTML network events cannot
//!   reopen a terminal parser lifecycle.
//! - Each `Tab` owns its `PageState`, `ResourceManager`, and `DocumentInputState`.
//!   There is no cross-tab sharing of DOM, resources, or input state; any
//!   shared work must go through the bus/runtime layers.

mod css;
mod discovery;
mod dom_style;
mod events;
mod html;
mod image;
mod load_observation;
mod parsed_document_observation;
pub use crate::page::stylesheet_discovery::StylesheetDiscoverySnapshotError;
#[cfg(feature = "conformance-observation")]
pub use crate::page::stylesheet_discovery::{
    AlternateFact, AlternateTitleFact, DisabledFact, HrefObservation, InlineStyleText,
    LinkDiscovery, LinkPolicyFacts, LinkTypeFact, MediaFact, OccurrenceIndex, PolicyFact,
    RelationFacts, RelationKind, SetSelectionFact, StyleBlockDiscovery, StyleBlockLinkAttributes,
    StyleTextChunks, StyleTypeFact, StylesheetDependencyOrder, StylesheetDiscovery,
    StylesheetDiscoveryError, StylesheetDiscoveryLimit, StylesheetDiscoveryStorage,
    StylesheetHrefOutcome, StylesheetIndexDomain, StylesheetOccurrence, StylesheetSource,
    TitleFact,
};
pub use parsed_document_observation::ParsedDocumentSnapshotError;
#[cfg(feature = "conformance-observation")]
pub use parsed_document_observation::{
    DiagnosticAvailability, ParsedAddressScope, ParsedDocumentObservation, ParsedDocumentPhase,
    ParsedTreeScope,
};
mod nav;
mod state;
mod status;
#[cfg(test)]
mod tests;
mod ui;

#[cfg(feature = "conformance-observation")]
mod addressing_observation;
#[cfg(feature = "conformance-observation")]
pub use crate::page::addressing::BaseOutcome;
#[cfg(feature = "conformance-observation")]
pub use crate::page::css_resources::{
    CssBase, CssReferenceOutcome, CssSource, CssSourceReferences,
};
#[cfg(feature = "conformance-observation")]
pub use crate::page::html_resources::{
    HtmlReferenceBase, HtmlReferenceOutcome, HtmlResourceReference,
};
#[cfg(feature = "conformance-observation")]
pub use addressing_observation::{
    AddressingObservationError, AddressingRequestState, BaseSelectionObservation,
    DocumentAddressObservation,
};

pub use self::nav::NavigationError;
pub use self::state::{HistoryEntry, Tab};
pub use self::ui::PageFrameStatus;
pub use dom_style::{inherited_color, page_background};

impl Tab {
    /// Inspect committed document/base state and authored HTML/CSS references without
    /// issuing commands. Contents are redacted; this internal regression text is
    /// not a stable public schema or URL serialization interface.
    pub fn document_address_debug_snapshot(&self) -> Option<String> {
        self.page.document_address_debug_snapshot()
    }
}

#[cfg(feature = "conformance-observation")]
pub(crate) mod observation;
#[cfg(feature = "conformance-observation")]
pub use observation::{CompletedPageFrame, FrameElementStyle, FrameObservationError};

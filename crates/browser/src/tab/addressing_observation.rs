//! Borrowed, exact engine-test observations. No formatting, mutation, or I/O.
//! This module is exposed only by `conformance-observation`, never by Debug.
use super::Tab;
use crate::page::addressing::BaseOutcome;
use crate::page::css_resources::{CssBase, CssSource, CssSourceReferences};
use crate::page::{DocumentAddressState, HtmlReferenceOutcome, HtmlResourceReference};
use addressing::{BrowserUrl, Origin, ResourceKind, UrlDiagnostic};
use css::{StylesheetCollectionInputBuildError, resource_references::CssResourceDiscovery};
use html::{Node, internal::Id};

#[derive(Debug)]
pub enum AddressingObservationError {
    SourceIdentityUnavailable,
    StylesheetInput(StylesheetCollectionInputBuildError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressingRequestState {
    Pending,
    InProgress,
    Complete,
}

pub struct BaseSelectionObservation<'tab> {
    source: Id,
    url: &'tab BrowserUrl,
    outcome: &'tab BaseOutcome,
    diagnostics: &'tab [UrlDiagnostic],
}
impl BaseSelectionObservation<'_> {
    pub fn source_node(&self) -> Id {
        self.source
    }
    pub fn url(&self) -> &BrowserUrl {
        self.url
    }
    pub fn outcome(&self) -> &BaseOutcome {
        self.outcome
    }
    pub fn diagnostics(&self) -> &[UrlDiagnostic] {
        self.diagnostics
    }
}

/// Borrows one committed document. The borrow prevents publication/navigation
/// while tooling correlates the ephemeral production inventories with nodes.
pub struct DocumentAddressObservation<'tab> {
    document: &'tab Node,
    addresses: &'tab DocumentAddressState,
    selected: Option<BaseSelectionObservation<'tab>>,
    html: Vec<HtmlResourceReference>,
    css: Vec<CssSourceReferences<'tab>>,
}
impl DocumentAddressObservation<'_> {
    pub fn document(&self) -> &Node {
        self.document
    }
    pub fn document_url(&self) -> &BrowserUrl {
        self.addresses.document_url()
    }
    pub fn fallback_base(&self) -> &BrowserUrl {
        self.addresses.fallback_base_url()
    }
    pub fn active_base(&self) -> &BrowserUrl {
        self.addresses.active_base_url()
    }
    pub fn origin(&self) -> &Origin {
        self.addresses.document_origin()
    }
    pub fn selected_base(&self) -> Option<&BaseSelectionObservation<'_>> {
        self.selected.as_ref()
    }
    pub fn html_references(&self) -> &[HtmlResourceReference] {
        &self.html
    }
    pub fn css_sources(&self) -> &[CssSourceReferences<'_>] {
        &self.css
    }
}
impl HtmlResourceReference {
    pub fn resolution_base(&self) -> crate::page::html_resources::HtmlReferenceBase {
        self.base
    }
    pub fn source_node(&self) -> Id {
        self.element
    }
    pub fn attribute(&self) -> &'static str {
        self.attribute
    }
    pub fn kind(&self) -> ResourceKind {
        self.kind
    }
    pub fn outcome(&self) -> &HtmlReferenceOutcome {
        &self.outcome
    }
}
impl CssSourceReferences<'_> {
    pub fn owner(&self) -> Id {
        self.owner
    }
    pub fn source(&self) -> &CssSource {
        &self.source
    }
    pub fn base(&self) -> &CssBase<'_> {
        &self.base
    }
    pub fn condition_deferred(&self) -> bool {
        self.condition_deferred
    }
    pub fn discovery(&self) -> &CssResourceDiscovery<'_> {
        &self.discovery
    }
}
impl Tab {
    /// Correlate a fixture-observed command with its actual retained load state.
    /// This does not admit a request or settle pending work.
    pub fn observe_addressing_request(
        &self,
        kind: core_types::ResourceKind,
        url: &str,
        request: core_types::RequestId,
        slot: Option<core_types::StylesheetSlotId>,
    ) -> Option<AddressingRequestState> {
        use super::state::StylesheetLoadPhase;
        if request != self.nav_gen {
            return None;
        }
        match (kind, slot) {
            (core_types::ResourceKind::Image, None) => match self.resources.image_state_by_url(url)
            {
                crate::resources::ImageState::Loading { .. } => {
                    Some(AddressingRequestState::Pending)
                }
                _ => None,
            },
            (core_types::ResourceKind::Css, Some(slot)) => {
                let load = self.stylesheet_loads.get(&slot)?;
                if load.request_id != request || load.issued_key.as_str() != url {
                    return None;
                }
                Some(match load.phase {
                    StylesheetLoadPhase::AwaitingStart => AddressingRequestState::Pending,
                    StylesheetLoadPhase::Complete { .. } => AddressingRequestState::Complete,
                    _ => AddressingRequestState::InProgress,
                })
            }
            _ => None,
        }
    }
    /// Observe committed addressing without requiring a frame or settled loads.
    /// `None` means no managed committed document, not an empty inventory.
    pub fn observe_document_addressing(
        &self,
    ) -> Result<Option<DocumentAddressObservation<'_>>, AddressingObservationError> {
        let (Some(document), Some(addresses)) =
            (self.page.dom.as_deref(), self.page.addresses.as_ref())
        else {
            return Ok(None);
        };
        let selected = addresses
            .selected_base_facts()
            .map(|(key, url, outcome, diagnostics)| {
                let handle = self
                    .dom_handle
                    .ok_or(AddressingObservationError::SourceIdentityUnavailable)?;
                let ids = self
                    .dom_store
                    .resolve_mutation_node_ids(handle, &[key])
                    .map_err(|_| AddressingObservationError::SourceIdentityUnavailable)?;
                let source = *ids
                    .into_parts()
                    .0
                    .first()
                    .ok_or(AddressingObservationError::SourceIdentityUnavailable)?;
                Ok(BaseSelectionObservation {
                    source,
                    url,
                    outcome,
                    diagnostics,
                })
            })
            .transpose()?;
        let html = self
            .page
            .html_resource_references()
            .ok_or(AddressingObservationError::SourceIdentityUnavailable)?;
        let css = self
            .page
            .css_resource_references()
            .map_err(AddressingObservationError::StylesheetInput)?
            .ok_or(AddressingObservationError::SourceIdentityUnavailable)?;
        Ok(Some(DocumentAddressObservation {
            document,
            addresses,
            selected,
            html,
            css,
        }))
    }
}

use crate::dom_store::DomStore;
use crate::input_state::DocumentInputState;
use crate::page::{DomPublicationRenderInvalidation, PageState};
use crate::rendering::{PendingRenderWork, RenderFrameExecutionTrace, RenderInvalidationRequest};
use crate::resources::ResourceManager;
use app_api::RepaintHandle;
use bus::{CoreCommand, DocumentPublicationFailure, HtmlParseFailure};
use core_types::{DomHandle, DomVersion, RequestId, StylesheetSlotId, TabId};
use net::{BodyProgress, FetchRequest, NetworkErrorKind, NetworkResponseInfo};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;

/// A history URL and its Browser-owned document-state association.
#[derive(Clone)]
pub struct HistoryEntry {
    pub(super) url: String,
    pub(super) document_state: Option<DocumentState>,
}

impl HistoryEntry {
    /// The URL associated with this entry.
    pub fn url(&self) -> &str {
        &self.url
    }
}

// The allocation is an opaque identity, not the document or a cache of it.
// Entries for the same document state share it; the active identity is derived
// from the current entry only while its page is available. It is independent of
// URLs, DOM handles and requests.
#[derive(Clone)]
pub(super) struct DocumentState(Arc<()>);

impl DocumentState {
    pub(super) fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(super) fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

// Existing history intent, deferred until response activation.
#[derive(Clone, Copy)]
pub(super) enum HistoryTarget {
    Append,
    Existing(usize),
}

/// One issued navigation, separate from the actual active document. No page or
/// parser state is retained here. A terminal result is latched once.
pub(super) struct NavigationAttempt {
    pub(super) request: FetchRequest,
    pub(super) history_target: HistoryTarget,
    pub(super) non_replacing_response: Option<NetworkResponseInfo>,
    pub(super) terminal: Option<Result<BodyProgress, DocumentLoadFailure>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DocumentResponseRejection {
    NotModifiedWithoutRepresentation,
    PartialRepresentationUnsupported,
    InstanceManipulationUnsupported,
    MissingContentType,
    EmptyContentType,
    UnusableContentType,
    UnsupportedContentType,
    DownloadUnsupported,
    UnsupportedDocumentCharset,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) enum DocumentParseStatus {
    #[default]
    Pending,
    Complete,
    Failed(DocumentLoadFailure),
}

/// Only relationships checked by the current load observation, never free-form
/// diagnostic text or a generalized resource error taxonomy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoadStateInvariant {
    BodyWithoutAdmission,
    BodyAccounting,
    CommittedDocument,
    ParserCompletion,
    SlotWithoutRequest,
    RequestWithoutSlot,
    RequestGeneration,
    RequestProvenance,
    SlotPhase,
    RetentionAccounting,
    FetchEvidence,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum DocumentLoadFailure {
    Network {
        kind: NetworkErrorKind,
        response_context: Option<NetworkResponseInfo>,
        body_progress: Option<BodyProgress>,
    },
    Parser(HtmlParseFailure),
    ParserCommandDeliveryUnavailable,
    UnexpectedNoContentBody,
    ResponseRejected {
        reason: DocumentResponseRejection,
        response_context: NetworkResponseInfo,
    },
    Publication(DocumentPublicationFailure),
    StylesheetInventory(crate::stylesheet_fetch::StylesheetInventoryError),
    ResponseRequestMismatch(NetworkResponseInfo),
    ResponseUrlUnavailable(NetworkResponseInfo),
    UnsupportedDocumentCreation {
        cause: crate::page::DocumentAddressError,
        response_context: NetworkResponseInfo,
    },
    ChunkBeforeAdmission,
    ChunkProvenanceMismatch,
    CompletionBeforeAdmission,
    CompletionMetadataMismatch(NetworkResponseInfo),
    CompletionAccountingMismatch(BodyProgress),
    ParserCompletionBeforeBody,
    LoadStateInvariantViolation {
        relationship: LoadStateInvariant,
        slot: Option<StylesheetSlotId>,
        slot_records: usize,
        request_records: usize,
    },
}

impl std::fmt::Debug for DocumentLoadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The presentation projection never formats nested owning error data.
        super::status::write_document_failure_status(f, self)
    }
}

impl std::fmt::Debug for DocumentParseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::Complete => f.write_str("Complete"),
            Self::Failed(failure) => f.debug_tuple("Failed").field(failure).finish(),
        }
    }
}

/// The actual admitted UTF-8 choice, retained without copying arbitrary labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DocumentEncodingDecision {
    ResponseCharset,
    MissingCharsetDefault,
    EmptyCharsetDefault,
}

#[derive(Default)]
pub(super) struct DocumentLoadState {
    // Private: only activation installs the actual admitted request.
    request: Option<FetchRequest>,
    // Immutable snapshot installed by successful HTML response-start admission.
    // Completion validates it; Pending alone is not admission.
    pub(super) response: Option<NetworkResponseInfo>,
    pub(super) encoding_decision: Option<DocumentEncodingDecision>,
    pub(super) decode_report: Option<html::internal::HtmlDecodeReport>,
    pub(super) diagnostics: Option<html::internal::HtmlParserDiagnosticReport>,
    pub(super) bytes_received: usize,
    // Only a matching, validated main-document NetworkDone installs this fact.
    // It proves neither parser acknowledgement nor response applicability.
    pub(super) body_completion: Option<BodyProgress>,
    pub(super) parse_status: DocumentParseStatus,
}

impl DocumentLoadState {
    pub(super) fn has_finalized_diagnostics(&self) -> bool {
        self.diagnostics
            .as_ref()
            .is_some_and(|r| r.completion() == html::internal::ParserInputCompletion::Finalized)
    }

    pub(super) fn failure(&self) -> Option<&DocumentLoadFailure> {
        match &self.parse_status {
            DocumentParseStatus::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}

impl std::fmt::Debug for DocumentLoadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentLoadState")
            .field("response_admitted", &self.response.is_some())
            .field("bytes_received", &self.bytes_received)
            .field("body_completion", &self.body_completion)
            .field("parse_status", &self.parse_status)
            .finish()
    }
}

pub(super) struct StylesheetLoadState {
    pub(super) request_id: RequestId,
    pub(super) phase: StylesheetLoadPhase,
}
// Inline acquisition ownership avoids an unqualified Box allocation per request.
#[allow(clippy::large_enum_variant)]
pub(super) enum StylesheetLoadPhase {
    Fetching(crate::stylesheet_fetch::StylesheetFetchAccumulator),
    AwaitingRuntime {
        progress: CssRuntimeProgress,
        abort_url: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CssRuntimeProgress {
    AwaitingBlock,
    BlockReceived,
}
impl StylesheetLoadState {
    #[cfg(any(test, feature = "conformance-observation"))]
    pub(super) fn issued_key(&self) -> &str {
        match &self.phase {
            StylesheetLoadPhase::Fetching(a) => a.issued_key.as_str(),
            StylesheetLoadPhase::AwaitingRuntime { abort_url, .. } => abort_url.as_str(),
        }
    }
}
impl std::fmt::Debug for StylesheetLoadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StylesheetLoadState")
            .field(
                "phase",
                &match &self.phase {
                    StylesheetLoadPhase::Fetching(a) if a.response.is_none() => "awaiting-start",
                    StylesheetLoadPhase::Fetching(_) => "receiving",
                    StylesheetLoadPhase::AwaitingRuntime { .. } => "awaiting-runtime",
                },
            )
            .finish()
    }
}
impl StylesheetLoadPhase {
    pub(super) fn network_complete(&self) -> bool {
        matches!(self, Self::AwaitingRuntime { .. })
    }
    #[cfg(test)]
    pub(super) fn accepted_response(&self) -> Option<&NetworkResponseInfo> {
        match self {
            Self::Fetching(a) => a.response.as_ref(),
            _ => None,
        }
    }
}
pub struct Tab {
    pub tab_id: TabId,

    pub url: String,
    pub(crate) history: Vec<HistoryEntry>,
    pub(crate) history_index: usize,
    last_reserved_navigation_id: RequestId,
    pub(super) navigation_attempt: Option<NavigationAttempt>,

    pub(super) loading: bool,
    // Active-document progress. Rejected submissions never overwrite it.
    pub(super) last_status: Option<String>,
    // One bounded, redacted UI notice; never load/failure/identity authority.
    // Needed to present a rejected submission alongside actual A/B progress.
    pub(super) navigation_notice: Option<String>,
    pub(super) document_load: DocumentLoadState,
    pub(super) stylesheet_loads: HashMap<StylesheetSlotId, StylesheetLoadState>,

    pub(super) page: PageState,
    pub(super) resources: ResourceManager,
    pub(super) repaint: Option<RepaintHandle>,
    pub(super) cmd_tx: Option<mpsc::Sender<CoreCommand>>,
    pub(super) document_input: DocumentInputState,
    pub(super) dom_store: DomStore,
    pub(super) dom_handle: Option<DomHandle>,
    pub(super) dom_version: DomVersion,
    pub(super) pending_render_work: PendingRenderWork,
    pub(super) last_render_trace: Option<RenderFrameExecutionTrace>,
}

impl Tab {
    pub fn new(tab_id: TabId) -> Self {
        Self {
            tab_id,
            url: String::new(),
            history: Vec::new(),
            history_index: 0,
            last_reserved_navigation_id: 0,
            navigation_attempt: None,
            loading: false,
            last_status: None,
            navigation_notice: None,
            document_load: DocumentLoadState::default(),
            stylesheet_loads: HashMap::new(),
            page: PageState::new(),
            resources: ResourceManager::new(),
            repaint: None,
            cmd_tx: None,
            document_input: DocumentInputState::default(),
            dom_store: DomStore::new(),
            dom_handle: None,
            dom_version: DomVersion::INITIAL,
            pending_render_work: PendingRenderWork::default(),
            last_render_trace: None,
        }
    }

    /// Active document identity, never the identity of a pending replacement.
    pub fn active_document_request_id(&self) -> Option<RequestId> {
        self.document_load
            .request
            .as_ref()
            .map(FetchRequest::request_id)
    }

    pub(super) fn active_document_request(&self) -> Option<&FetchRequest> {
        self.document_load.request.as_ref()
    }

    pub(super) fn reserve_navigation_request_id(
        &mut self,
    ) -> Result<RequestId, super::nav::NavigationError> {
        let id = self
            .last_reserved_navigation_id
            .checked_add(1)
            .ok_or(super::nav::NavigationError::RequestIdExhausted)?;
        self.last_reserved_navigation_id = id;
        Ok(id)
    }

    pub(super) fn is_navigation_request(&self, tab_id: TabId, request_id: RequestId) -> bool {
        tab_id == self.tab_id
            && self
                .navigation_attempt
                .as_ref()
                .is_some_and(|attempt| attempt.request.request_id() == request_id)
    }

    pub(super) fn navigation_pending(&self) -> bool {
        self.navigation_attempt
            .as_ref()
            .is_some_and(|attempt| attempt.terminal.is_none())
    }

    pub(super) fn enqueue_html_command(
        &self,
        command: CoreCommand,
    ) -> Result<(), DocumentLoadFailure> {
        self.cmd_tx
            .as_ref()
            .ok_or(DocumentLoadFailure::ParserCommandDeliveryUnavailable)?
            .send(command)
            .map_err(|_| DocumentLoadFailure::ParserCommandDeliveryUnavailable)
    }

    /// The sole active-request installation boundary. Every recoverable check,
    /// including parser-start enqueue, precedes replacement effects.
    pub(super) fn activate_document_response(
        &mut self,
        response: NetworkResponseInfo,
        addresses: crate::page::DocumentAddressState,
        encoding: DocumentEncodingDecision,
    ) -> Result<(), DocumentLoadFailure> {
        let attempt = self
            .navigation_attempt
            .as_ref()
            .ok_or(DocumentLoadFailure::Publication(
                DocumentPublicationFailure::InvariantViolation,
            ))?;
        let request_id = attempt.request.request_id();
        if attempt.terminal.is_some()
            || attempt.non_replacing_response.is_some()
            || request_id > self.last_reserved_navigation_id
            || self
                .active_document_request_id()
                .is_some_and(|active| active >= request_id)
            || matches!(attempt.history_target, HistoryTarget::Existing(index) if index >= self.history.len())
        {
            return Err(DocumentLoadFailure::Publication(
                DocumentPublicationFailure::InvariantViolation,
            ));
        }
        self.enqueue_html_command(CoreCommand::ParseHtmlStart {
            tab_id: self.tab_id,
            request_id,
        })?;
        let attempt = self
            .navigation_attempt
            .take()
            .expect("checked current attempt");
        self.retire_document_stylesheets();
        if let Some(previous) = self.active_document_request_id() {
            self.resources.retire_pending_images(previous);
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id: previous,
            });
        }
        let original = attempt.request.original_url().as_str().to_owned();
        match attempt.history_target {
            HistoryTarget::Append => {
                self.history.truncate(self.history_index + 1);
                self.history.push(HistoryEntry {
                    url: original,
                    document_state: None,
                });
                self.history_index = self.history.len() - 1;
            }
            HistoryTarget::Existing(index) => {
                self.history[index].url = original;
                self.history_index = index;
            }
        }
        self.accept_history_document(addresses.document_url());
        self.document_input.clear_for_navigation();
        self.dom_store.clear();
        self.dom_handle = None;
        self.dom_version = DomVersion::INITIAL;
        self.stylesheet_loads.clear();
        self.page.reset_for_navigation();
        self.page.begin_document(addresses);
        self.clear_render_orchestration_state();
        self.url = response
            .reported_response_url()
            .expect("prepared response URL")
            .to_owned();
        self.last_status = Some(format!(
            "Loading document • {}",
            super::status::response_summary(&response, 0)
        ));
        self.document_load = DocumentLoadState {
            request: Some(attempt.request),
            response: Some(response),
            encoding_decision: Some(encoding),
            ..Default::default()
        };
        self.loading = true;
        self.poke_redraw();
        Ok(())
    }

    pub(super) fn restore_active_toolbar_url(&mut self) {
        self.url = if self.active_document_request_id().is_some() {
            self.history
                .get(self.history_index)
                .map(|entry| entry.url.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
    }

    pub(super) fn fail_navigation_attempt(&mut self, failure: DocumentLoadFailure) {
        let Some(attempt) = self
            .navigation_attempt
            .as_mut()
            .filter(|a| a.terminal.is_none())
        else {
            return;
        };
        let request_id = attempt.request.request_id();
        attempt.terminal = Some(Err(failure));
        self.send_cmd(CoreCommand::CancelRequest {
            tab_id: self.tab_id,
            request_id,
        });
        self.restore_active_toolbar_url();
        self.poke_redraw();
    }

    pub(super) fn cancel_pending_navigation(&mut self) {
        if let Some(attempt) = self
            .navigation_attempt
            .as_ref()
            .filter(|a| a.terminal.is_none())
        {
            let response_context = attempt.non_replacing_response.clone();
            self.fail_navigation_attempt(DocumentLoadFailure::Network {
                kind: NetworkErrorKind::Cancelled,
                response_context,
                // Acquisition alone is not AI-owned transfer progress. A
                // completed attempt is already terminal and cannot enter here.
                body_progress: None,
            });
        }
    }

    pub(crate) fn retire_and_cancel_requests(&mut self) {
        self.retire_document_stylesheets();
        if let Some(request_id) = self.active_document_request_id() {
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id,
            });
        }
        self.cancel_pending_navigation();
    }

    /// History entries in navigation order. Entry URLs and document associations
    /// are updated together by Browser navigation.
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Index of the selected entry; zero when history is empty.
    pub fn history_index(&self) -> usize {
        self.history_index
    }

    pub fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx);
    }

    pub fn set_repaint_handle(&mut self, h: RepaintHandle) {
        self.repaint = Some(h);
    }

    pub(super) fn update_loading(&mut self) -> super::load_observation::StaticInputSettlement {
        use super::load_observation::StaticInputSettlement;
        let settlement = match self.static_input_settlement() {
            Ok(settlement) => settlement,
            Err(failure) => {
                self.fail_document(failure);
                return StaticInputSettlement::Failed;
            }
        };
        self.loading = settlement == StaticInputSettlement::Pending;
        if settlement == StaticInputSettlement::EvidenceUnavailable {
            self.last_status = Some("Document input evidence is incomplete".into());
        }
        settlement
    }

    pub(super) fn fail_document(&mut self, failure: DocumentLoadFailure) {
        if self.document_load.failure().is_some() {
            return;
        }
        let inventory_failure = matches!(failure, DocumentLoadFailure::StylesheetInventory(_));
        self.document_load.parse_status = DocumentParseStatus::Failed(failure);
        self.loading = false;
        self.last_status = None;
        for index in 0..self.page.stylesheet_slot_count() {
            let slot = self
                .page
                .stylesheet_slot_id_at(index)
                .expect("current order");
            if self
                .stylesheet_loads
                .get(&slot)
                .is_some_and(|l| !l.phase.network_complete())
            {
                self.fail_css_acquisition(
                    slot,
                    crate::stylesheet_fetch::StylesheetFetchFailure::DocumentAborted,
                    None,
                    crate::stylesheet_fetch::StylesheetCollectionStop::DocumentAborted,
                );
            }
        }
        if let Some(request_id) = self.active_document_request_id() {
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id,
            });
        }
        if !inventory_failure {
            self.last_status = self
                .document_load
                .failure()
                .map(super::status::document_failure_status);
        }
        self.poke_redraw();
    }

    pub(super) fn is_current(&self, tab_id: TabId, request_id: RequestId) -> bool {
        tab_id == self.tab_id && self.active_document_request_id() == Some(request_id)
    }

    pub(super) fn send_cmd(&self, cmd: CoreCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    pub(super) fn poke_redraw(&self) {
        if let Some(repaint) = &self.repaint {
            repaint.request_now();
        }
    }

    pub(super) fn request_render_work(&mut self, request: RenderInvalidationRequest) -> bool {
        let requests_render_work = self.pending_render_work.push(request);
        if requests_render_work {
            self.poke_redraw();
        }
        requests_render_work
    }

    pub(super) fn request_optional_render_work(
        &mut self,
        request: Option<RenderInvalidationRequest>,
    ) -> bool {
        request.is_some_and(|request| self.request_render_work(request))
    }

    pub(super) fn request_dom_publication_render_work(
        &mut self,
        publication: DomPublicationRenderInvalidation,
    ) -> bool {
        let mut requests_redraw = false;
        for request in publication.requests() {
            requests_redraw |= self.pending_render_work.push(request);
        }
        if requests_redraw {
            self.poke_redraw();
        }
        requests_redraw
    }

    pub(super) fn clear_render_orchestration_state(&mut self) {
        self.pending_render_work = PendingRenderWork::default();
        self.last_render_trace = None;
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::tab::NavigationError;

    #[test]
    fn aj3_reservation_exhaustion_preserves_the_actual_active_request() {
        let mut tab = Tab::new(1);
        let (tx, commands) = mpsc::channel();
        tab.set_bus_sender(tx);
        assert_eq!(tab.active_document_request_id(), None);
        tab.navigate_to_new("https://fixture.invalid/a".into())
            .unwrap();
        let request = match commands.recv().unwrap() {
            CoreCommand::FetchStream { request, .. } => request,
            _ => panic!("fetch"),
        };
        let mut headers = net::HttpHeaders::new();
        headers.append("content-type", b"text/html").unwrap();
        let response = NetworkResponseInfo::http(
            request.address(),
            request.original_url().as_str().into(),
            Some(request.original_url().as_str().into()),
            net::HttpStatus::try_from(200).unwrap(),
            headers,
        )
        .unwrap();
        tab.on_core_event(bus::CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: request.request_id(),
            stylesheet_slot_id: None,
            kind: core_types::ResourceKind::Html,
            response,
        })
        .unwrap();
        assert_eq!(tab.active_document_request_id(), Some(request.request_id()));
        commands.try_iter().for_each(drop);
        // Allocator boundary setup only: no active identity is assigned/synchronized.
        tab.last_reserved_navigation_id = RequestId::MAX - 1;
        tab.navigate_to_new("https://fixture.invalid/b".into())
            .unwrap();
        let pending = match commands.recv().unwrap() {
            CoreCommand::FetchStream { request, .. } => request,
            _ => panic!("fetch"),
        };
        assert_eq!(pending.request_id(), RequestId::MAX);
        assert_eq!(
            tab.navigate_to_new("https://fixture.invalid/c".into()),
            Err(NavigationError::RequestIdExhausted)
        );
        assert_eq!(tab.active_document_request_id(), Some(request.request_id()));
        assert_eq!(
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .request
                .request_id(),
            RequestId::MAX
        );
        assert_eq!(tab.last_reserved_navigation_id, RequestId::MAX);
        assert!(commands.try_recv().is_err());
    }
}

use crate::dom_store::DomStore;
use crate::input_state::DocumentInputState;
use crate::page::{DomPublicationRenderInvalidation, PageState};
use crate::rendering::{PendingRenderWork, RenderFrameExecutionTrace, RenderInvalidationRequest};
use crate::resources::ResourceManager;
use app_api::RepaintHandle;
use bus::CoreCommand;
use core_types::{DomHandle, DomVersion, NetworkResponseInfo, RequestId, StylesheetSlotId, TabId};
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DocumentParseStatus {
    #[default]
    Pending,
    Complete,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub(super) struct DocumentLoadState {
    pub(super) response: Option<NetworkResponseInfo>,
    pub(super) bytes_received: usize,
    pub(super) parse_status: DocumentParseStatus,
}

#[derive(Clone, Debug)]
pub(super) struct StylesheetLoadState {
    pub(super) response: NetworkResponseInfo,
    pub(super) accept_body: bool,
    pub(super) network_done: bool,
}

pub struct Tab {
    pub tab_id: TabId,

    pub url: String,
    pub(crate) history: Vec<HistoryEntry>,
    pub(crate) history_index: usize,
    pub nav_gen: RequestId,

    pub(super) loading: bool,
    pub(super) last_status: Option<String>,
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
            nav_gen: 0,
            loading: false,
            last_status: None,
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

    pub(super) fn update_loading(&mut self) {
        self.loading = match self.document_load.parse_status {
            DocumentParseStatus::Pending => true,
            DocumentParseStatus::Complete => self.page.pending_count() > 0,
            DocumentParseStatus::Failed => false,
        };
    }

    pub(super) fn fail_document(&mut self, status: String) {
        if self.document_load.parse_status != DocumentParseStatus::Failed {
            self.document_load.parse_status = DocumentParseStatus::Failed;
            self.loading = false;
            self.last_status = Some(status);
            let completed = self
                .stylesheet_loads
                .iter()
                .filter_map(|(id, load)| load.network_done.then_some(*id))
                .collect();
            let pending = self.page.abort_pending_external_stylesheets(&completed);
            self.retire_stylesheet_requests(pending, self.nav_gen);
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id: self.nav_gen,
            });
            self.poke_redraw();
        }
    }

    pub(super) fn is_current(&self, tab_id: TabId, request_id: RequestId) -> bool {
        tab_id == self.tab_id && request_id == self.nav_gen
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

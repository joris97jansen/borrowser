use crate::dom_store::DomStore;
use crate::input_state::DocumentInputState;
use crate::page::PageState;
use crate::rendering::{PendingRenderWork, RenderFrameExecutionTrace, RenderInvalidationRequest};
use crate::resources::ResourceManager;
use app_api::RepaintHandle;
use bus::CoreCommand;
use core_types::{DomHandle, NetworkResponseInfo, RequestId, StylesheetSlotId, TabId};
use std::collections::HashMap;
use std::sync::mpsc;

#[derive(Clone, Debug, Default)]
pub(super) struct DocumentLoadState {
    pub(super) response: Option<NetworkResponseInfo>,
    pub(super) bytes_received: usize,
}

#[derive(Clone, Debug)]
pub(super) struct StylesheetLoadState {
    pub(super) response: NetworkResponseInfo,
    pub(super) accept_body: bool,
}

pub struct Tab {
    pub tab_id: TabId,

    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
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
            pending_render_work: PendingRenderWork::default(),
            last_render_trace: None,
        }
    }

    pub fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx);
    }

    pub fn set_repaint_handle(&mut self, h: RepaintHandle) {
        self.repaint = Some(h);
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

    pub(super) fn request_render_work(&mut self, request: RenderInvalidationRequest) {
        debug_assert!(
            request.requested_work.requests_redraw(),
            "render invalidation request must request a frame: {:?}",
            request
        );
        self.pending_render_work.push(request);
        if request.requested_work.requests_redraw() {
            self.poke_redraw();
        }
    }

    pub(super) fn request_optional_render_work(
        &mut self,
        request: Option<RenderInvalidationRequest>,
    ) -> bool {
        if let Some(request) = request {
            self.request_render_work(request);
            true
        } else {
            false
        }
    }

    pub(super) fn clear_render_orchestration_state(&mut self) {
        self.pending_render_work = PendingRenderWork::default();
        self.last_render_trace = None;
    }
}

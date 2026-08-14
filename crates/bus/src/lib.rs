use core_types::{
    DomHandle, DomVersion, NetworkErrorKind, NetworkResponseInfo, ResourceKind, StylesheetSlotId,
    TabId,
};
use html::DomPatch;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub struct DocumentPublication {
    pub handle: DomHandle,
    pub document_mode: html::DocumentMode,
    pub payload: DocumentPublicationPayload,
}

#[derive(Debug)]
pub enum DocumentPublicationPayload {
    Patch {
        from: DomVersion,
        to: DomVersion,
        patches: Vec<DomPatch>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentPublicationFailure {
    DocumentModeUnavailable,
    PreSelectionBudgetExceeded,
    DocumentModeChanged,
    GenerationMismatch,
    InvalidPayload,
    MaterializationFailed,
    InvariantViolation,
}

#[derive(Debug)]
pub enum CoreCommand {
    // Network requests
    FetchStream {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: Option<StylesheetSlotId>,
        url: String,
        kind: ResourceKind,
    },
    CancelRequest {
        tab_id: TabId,
        request_id: u64,
    },
    // HTML Parser
    ParseHtmlStart {
        tab_id: TabId,
        request_id: u64,
    },
    ParseHtmlChunk {
        tab_id: TabId,
        request_id: u64,
        bytes: Vec<u8>,
    },
    ParseHtmlDone {
        tab_id: TabId,
        request_id: u64,
    },
    // CSS stylesheet runtime
    CssChunk {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
        bytes: Vec<u8>,
    },
    CssDone {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
    },
    CssAbort {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
    },
}

#[derive(Debug)]
pub enum CoreEvent {
    // Network -> UI
    NetworkStart {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: Option<StylesheetSlotId>,
        kind: ResourceKind,
        response: NetworkResponseInfo,
    },
    NetworkChunk {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: Option<StylesheetSlotId>,
        kind: ResourceKind,
        url: String,
        bytes: Vec<u8>,
    },
    NetworkDone {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: Option<StylesheetSlotId>,
        kind: ResourceKind,
        response: NetworkResponseInfo,
        bytes_received: usize,
    },
    NetworkError {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: Option<StylesheetSlotId>,
        kind: ResourceKind,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
    },

    // HTML Parser -> UI (patch stream)
    DomPatchUpdate {
        tab_id: TabId,
        request_id: u64,
        publication: DocumentPublication,
    },
    DocumentPublicationFailed {
        tab_id: TabId,
        request_id: u64,
        handle: Option<DomHandle>,
        failure: DocumentPublicationFailure,
    },

    // CSS stylesheet runtime -> UI
    // Carries fully decoded stylesheet text for downstream css::syntax parsing.
    CssDecodedBlock {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
        css_block: String,
    },
    CssSheetDone {
        tab_id: TabId,
        request_id: u64,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
    },
}

pub struct Bus {
    pub cmd_tx: Sender<CoreCommand>,
    pub evt_rx: Receiver<CoreEvent>,
    pub evt_tx: Sender<CoreEvent>, // shareable for runtimes
}

use core_types::{DomHandle, DomVersion, ResourceKind, TabId};
use html::{DomPatch, Node};
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub enum CoreCommand {
    // Network requests
    FetchStream {
        tab_id: TabId,
        request_id: u64,
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
    // CSS Parser
    CssChunk {
        tab_id: TabId,
        request_id: u64,
        url: String,
        bytes: Vec<u8>,
    },
    CssDone {
        tab_id: TabId,
        request_id: u64,
        url: String,
    },
}

#[derive(Debug)]
pub enum CoreEvent {
    // Network -> UI
    NetworkStart {
        tab_id: TabId,
        request_id: u64,
        kind: ResourceKind,
        url: String,
        content_type: Option<String>,
    },
    NetworkChunk {
        tab_id: TabId,
        request_id: u64,
        kind: ResourceKind,
        url: String,
        bytes: Vec<u8>,
    },
    NetworkDone {
        tab_id: TabId,
        request_id: u64,
        kind: ResourceKind,
        url: String,
    },
    NetworkError {
        tab_id: TabId,
        request_id: u64,
        kind: ResourceKind,
        url: String,
        error: String,
    },

    // HTML Parser -> UI (legacy snapshot path)
    DomUpdate {
        tab_id: TabId,
        request_id: u64,
        dom: Box<Node>,
    },
    // HTML Parser -> UI (patch stream)
    DomPatchUpdate {
        tab_id: TabId,
        request_id: u64,
        handle: DomHandle,
        from: DomVersion,
        to: DomVersion,
        patches: Vec<DomPatch>,
    },

    // CSS Parser -> UI
    CssParsedBlock {
        tab_id: TabId,
        request_id: u64,
        url: String,
        css_block: String,
    },
    CssSheetDone {
        tab_id: TabId,
        request_id: u64,
        url: String,
    },
}

pub struct Bus {
    pub cmd_tx: Sender<CoreCommand>,
    pub evt_rx: Receiver<CoreEvent>,
    pub evt_tx: Sender<CoreEvent>, // shareable for runtimes
}

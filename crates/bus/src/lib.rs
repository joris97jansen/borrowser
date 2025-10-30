use std::sync::mpsc::{Sender, Receiver};
use core_types::{
    ResourceKind,
    SessionId,
};
use html::{
    Node,
};

pub enum CoreCommand {
    // Network requests
    FetchStream { session_id: SessionId, request_id: u64, url: String, kind: ResourceKind },
    CancelRequest { session_id: SessionId, request_id: u64 },
    // HTML Parser
    ParseHtmlStart {session_id: SessionId, request_id: u64 },
    ParseHtmlChunk { session_id: SessionId, request_id: u64, bytes: Vec<u8> },
    ParseHtmlDone { session_id: SessionId, request_id: u64 },
    // CSS Parser
    CssChunk { session_id: SessionId, request_id: u64, url: String, bytes: Vec<u8> },
    CssDone { session_id: SessionId, request_id: u64, url: String }
}

pub enum CoreEvent {
    // Network -> UI
    NetworkStart { session_id: SessionId, request_id: u64, kind: ResourceKind, url: String, content_type: Option<String> },
    NetworkChunk { session_id: SessionId, request_id: u64, kind: ResourceKind, url: String, bytes: Vec<u8> },
    NetworkDone { session_id: SessionId, request_id: u64, kind: ResourceKind, url: String },
    NetworkError { session_id: SessionId, request_id: u64, kind: ResourceKind, url: String, error: String },

    // HTML Parser -> UI
    DomUpdate { session_id: SessionId, request_id: u64, dom: Node },

    // CSS Parser -> UI
    CssParsedBlock { session_id: SessionId, request_id: u64, url: String, css_block: String },
    CssSheetDone { session_id: SessionId, request_id: u64, url: String },
}

pub struct Bus {
    pub cmd_tx: Sender<CoreCommand>,
    pub evt_rx: Receiver<CoreEvent>,
    pub evt_tx: Sender<CoreEvent>, // shareable for runtimes
}

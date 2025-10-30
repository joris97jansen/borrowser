use std::sync::mpsc::{Sender, Receiver};
use core_types::ResourceKind;
use html::{
    Node,
};

pub enum CoreCommand {
    // Network requests
    FetchStream { request_id: u64, url: String, kind: ResourceKind },
    CancelRequest { request_id: u64 },
    // HTML Parser
    ParseHtmlStart { request_id: u64 },
    ParseHtmlChunk { request_id: u64, bytes: Vec<u8> },
    ParseHtmlDone { request_id: u64 },
    // CSS Parser
    CssChunk { request_id: u64, url: String, bytes: Vec<u8> },
    CssDone { request_id: u64, url: String }
}

pub enum CoreEvent {
    // Network -> UI
    NetworkStart { request_id: u64, kind: ResourceKind, url: String, content_type: Option<String> },
    NetworkChunk { request_id: u64, kind: ResourceKind, url: String, bytes: Vec<u8> },
    NetworkDone { request_id: u64, kind: ResourceKind, url: String },
    NetworkError { request_id: u64, kind: ResourceKind, url: String, error: String },

    // HTML Parser -> UI
    DomUpdate { request_id: u64, dom: Node },

    // CSS Parser -> UI
    CssParsedBlock { request_id: u64, url: String, css_block: String },
    CssSheetDone { request_id: u64, url: String },
}

pub struct Bus {
    pub cmd_tx: Sender<CoreCommand>,
    pub evt_rx: Receiver<CoreEvent>,
    pub evt_tx: Sender<CoreEvent>, // shareable for runtimes
}

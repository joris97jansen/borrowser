use core_types::{NetworkErrorKind, NetworkResponseInfo};

pub enum NetEvent {
    Start {
        request_id: u64,
        response: NetworkResponseInfo,
    },
    Chunk {
        request_id: u64,
        url: String,
        chunk: Vec<u8>,
    },
    Done {
        request_id: u64,
        response: NetworkResponseInfo,
        bytes_received: usize,
    },
    Error {
        request_id: u64,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
    },
}

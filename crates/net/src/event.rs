use crate::{BodyProgress, NetworkResponseInfo};
use std::fmt;

/// Coarse failures plus current response-admission boundaries. HTTP error
/// statuses are response facts, not members of this taxonomy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkErrorKind {
    Cancelled,
    Transport,
    LocalFile,
    Read,
    ResourceLimit,
    MalformedResponse,
    UnsupportedResponse,
}

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
        body: BodyProgress,
    },
    Error {
        request_id: u64,
        url: String,
        error_kind: NetworkErrorKind,
        response: Option<NetworkResponseInfo>,
        body: Option<BodyProgress>,
        error: String,
    },
}

impl fmt::Debug for NetEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { response, .. } => f
                .debug_struct("Start")
                .field("response", response)
                .finish_non_exhaustive(),
            Self::Chunk { chunk, .. } => f
                .debug_struct("Chunk")
                .field("bytes", &chunk.len())
                .finish_non_exhaustive(),
            Self::Done { response, body, .. } => f
                .debug_struct("Done")
                .field("response", response)
                .field("body", body)
                .finish_non_exhaustive(),
            Self::Error {
                error_kind,
                response,
                body,
                ..
            } => f
                .debug_struct("Error")
                .field("error_kind", error_kind)
                .field("response", response)
                .field("body", body)
                .finish_non_exhaustive(),
        }
    }
}

use crate::{BodyProgress, NetworkResponseInfo};
use std::fmt;

/// The byte/metadata domain whose inclusive resource budget was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceLimitKind {
    EncodedBody,
    DecodedBody,
    ResponseHead,
    FramingMetadata,
}

/// Failures of admitted operations. Synchronous request rejection remains
/// RequestAdmissionError; HTTP error statuses remain immutable response facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkErrorKind {
    Cancelled,
    DnsFailure,
    ConnectionFailure,
    TlsFailure,
    Timeout,
    Backend,
    LocalFile,
    Read,
    ResourceLimit(ResourceLimitKind),
    MalformedResponse,
    ContentDecoding,
    UnsupportedEncoding,
    UnsupportedResponse,
    Redirect(crate::RedirectError),
}

impl NetworkErrorKind {
    /// Fixed diagnostic projection, never an underlying error's Display/Debug.
    pub(crate) fn diagnostic(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::DnsFailure => "DNS resolution failed",
            Self::ConnectionFailure => "connection failed",
            Self::TlsFailure => "TLS operation failed",
            Self::Timeout => "network operation timed out",
            Self::Backend => "HTTP acquisition failed",
            Self::LocalFile => "local file operation failed",
            Self::Read => "response body read failed",
            Self::ResourceLimit(ResourceLimitKind::EncodedBody) => {
                "encoded HTTP body limit exceeded"
            }
            Self::ResourceLimit(ResourceLimitKind::DecodedBody) => "delivered body limit exceeded",
            Self::ResourceLimit(ResourceLimitKind::ResponseHead) => {
                "HTTP response head limit exceeded"
            }
            Self::ResourceLimit(ResourceLimitKind::FramingMetadata) => {
                "HTTP body framing metadata limit exceeded"
            }
            Self::MalformedResponse => "malformed HTTP response",
            Self::ContentDecoding => "invalid or incomplete gzip content",
            Self::UnsupportedEncoding => "unsupported response content encoding",
            Self::UnsupportedResponse => "unsupported HTTP response",
            Self::Redirect(_) => "redirect failed",
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai7_error_debug_is_literal_bounded_and_redacted_in_both_modes() {
        let event = NetEvent::Error {
            request_id: 999,
            url: "https://user:password@PRIVATE/".into(),
            error_kind: NetworkErrorKind::Timeout,
            response: None,
            body: None,
            error: "Authorization: PRIVATE body=PRIVATE".into(),
        };
        assert_eq!(
            format!("{event:?}"),
            "Error { error_kind: Timeout, response: None, body: None, .. }"
        );
        assert_eq!(
            format!("{event:#?}"),
            "Error {\n    error_kind: Timeout,\n    response: None,\n    body: None,\n    ..\n}"
        );
        for kind in [
            NetworkErrorKind::DnsFailure,
            NetworkErrorKind::ConnectionFailure,
            NetworkErrorKind::TlsFailure,
            NetworkErrorKind::Timeout,
            NetworkErrorKind::Backend,
            NetworkErrorKind::UnsupportedEncoding,
            NetworkErrorKind::ContentDecoding,
        ] {
            assert!(format!("{kind:?}").len() < 32);
            assert!(!kind.diagnostic().contains("PRIVATE"));
        }
    }
}

//! Selected HTTP response acquisition, not an observable individual-attempt API.
use std::io::Read;

use core_types::ResourceKind;

use crate::{
    FetchRequest, HttpClientPolicy, HttpHeaders, HttpStatus, NetworkErrorKind, NetworkResponseInfo,
    ResponseAdmissionError, agent::agent_for_policy, log::log_network_error,
};

/// Acquire a selected HTTP response. The current ureq backend may follow redirects
/// internally; AI5/AI6 retain controlled attempts and per-hop admission.
/// Backends never publish events or implement consumer body/terminal policy.
pub(crate) trait HttpResponseBackend: Send + 'static {
    fn acquire(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError>;
}

/// An ownership bundle over existing response facts and an unread body source.
/// Created and consumed in the worker, so the reader needs no Send/Sync bound.
pub(crate) struct AcquiredHttpResponse {
    pub(crate) metadata: NetworkResponseInfo,
    pub(crate) reader: Box<dyn Read + 'static>,
}

pub(crate) enum HttpAcquireError {
    Response(ResponseAdmissionError),
    Network {
        kind: NetworkErrorKind,
        error: String,
    },
}

pub(crate) struct UreqBackend {
    policy: HttpClientPolicy,
}

impl UreqBackend {
    /// Construction stores policy only. Agent/TLS initialization is lazy and
    /// confined to real HTTP acquisition after Fetch's cancellation check.
    pub(crate) fn new(policy: HttpClientPolicy) -> Self {
        Self { policy }
    }
}

impl HttpResponseBackend for UreqBackend {
    fn acquire(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let issued_url = request
            .legacy_issued_url(kind)
            .expect("validated HTTP route");
        let agent = agent_for_policy(&self.policy);
        let response = match build_http_request(&agent, request, kind).call() {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(err) if err.kind() == ureq::ErrorKind::BadHeader => {
                return Err(HttpAcquireError::Network {
                    kind: NetworkErrorKind::MalformedResponse,
                    error: "malformed HTTP response headers".into(),
                });
            }
            Err(err) => {
                let error = err.to_string();
                log_network_error(request.request_id(), kind, &issued_url, "transport", &error);
                return Err(HttpAcquireError::Network {
                    kind: NetworkErrorKind::Transport,
                    error,
                });
            }
        };
        let metadata = response_metadata(request, &issued_url, &response)
            .map_err(HttpAcquireError::Response)?;
        Ok(AcquiredHttpResponse {
            metadata,
            reader: response.into_reader(),
        })
    }
}

// The full request stays owned by the worker. Only admitted fields enter ureq;
// initiator/origin facts never implicitly generate policy or credential headers.
// Keep the complete legacy input URL for response metadata compatibility; ureq
// excludes the fragment from the wire request target. AI5 owns backend evolution.
pub(crate) fn build_http_request(
    agent: &ureq::Agent,
    request: &FetchRequest,
    kind: ResourceKind,
) -> ureq::Request {
    let url = request
        .legacy_issued_url(kind)
        .expect("validated execution route");
    let mut built = agent.get(&url);
    for (name, value) in request.headers().iter() {
        let value = std::str::from_utf8(value).expect("admitted ASCII request field");
        built = built.set(name, value);
    }
    built
}

// This projects ureq's exposed list, not wire bytes: ureq trims values and
// removes encoding/length during gzip processing. The pinned parser correction
// rejects malformed field construction before any backend accessor runs.
// Reconstruct interleaved occurrences without sorting, joining, or omission.
pub(crate) fn response_metadata(
    request: &FetchRequest,
    issued_url: &str,
    response: &ureq::Response,
) -> Result<NetworkResponseInfo, ResponseAdmissionError> {
    let status = HttpStatus::try_from(response.status())?;
    if status.is_informational() {
        return Err(ResponseAdmissionError::InformationalResponseUnsupported);
    }
    let names = response.headers_names();
    let mut fields = HttpHeaders::new();
    for (index, name) in names.iter().enumerate() {
        let occurrence = names[..index]
            .iter()
            .filter(|n| n.eq_ignore_ascii_case(name))
            .count();
        let values = response.all(name);
        let count = names
            .iter()
            .filter(|n| n.eq_ignore_ascii_case(name))
            .count();
        if values.len() != count {
            return Err(ResponseAdmissionError::HeaderValuesUnavailable);
        }
        fields
            .append(name, values[occurrence].as_bytes())
            .map_err(ResponseAdmissionError::Headers)?;
    }
    NetworkResponseInfo::http(
        request.original_url().clone(),
        issued_url.to_owned(),
        Some(response.get_url().to_owned()),
        status,
        fields,
    )
}

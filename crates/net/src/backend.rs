//! Individual HTTP attempt acquisition under Fetch-owned redirect coordination.
use std::io::Read;

use core_types::ResourceKind;

use crate::{
    FetchRequest, HttpClientPolicy, HttpHeaders, HttpStatus, NetworkErrorKind, NetworkResponseInfo,
    ResponseAdmissionError, agent::agent_for_policy,
};

/// Acquire one HTTP response without redirects or content decompression.
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
    // Keep the concrete source until the worker's diagnostic boundary. Neither
    // its URL nor Display/source messages are safe diagnostic input.
    Backend(Box<ureq::Error>),
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
        // Advertise the shared decoder's supported compression explicitly. This
        // is an execution capability, separate from admitted author fields.
        let response = match build_http_request(&agent, request, kind)
            .set("Accept-Encoding", "gzip")
            .call()
        {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(HttpAcquireError::Backend(Box::new(error))),
        };
        let metadata = response_metadata(request, &issued_url, &response)
            .map_err(HttpAcquireError::Response)?;
        Ok(AcquiredHttpResponse {
            metadata,
            reader: response.into_reader(),
        })
    }
}

pub(crate) fn backend_failure(error: &ureq::Error) -> (NetworkErrorKind, &'static str) {
    let kind = typed_failure(error).unwrap_or(NetworkErrorKind::Backend);
    let message = if matches!(
        error.kind(),
        ureq::ErrorKind::BadHeader | ureq::ErrorKind::BadStatus
    ) {
        "malformed HTTP response head"
    } else {
        kind.diagnostic()
    };
    (kind, message)
}

/// Inspect only concrete owned wrappers used by this backend. In particular,
/// io::Error::get_ref() exposes payloads that a generic source walk can skip.
/// Unknown wrappers/messages never authorize a more precise classification.
pub(crate) fn typed_failure(error: &(dyn std::error::Error + 'static)) -> Option<NetworkErrorKind> {
    use crate::ResourceLimitKind;
    if let Some(error) = error.downcast_ref::<std::io::Error>() {
        if error.kind() == std::io::ErrorKind::TimedOut {
            return Some(NetworkErrorKind::Timeout);
        }
        if let Some(kind) = error.get_ref().and_then(|source| typed_failure(source)) {
            return Some(kind);
        }
        return match error.kind() {
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe => Some(NetworkErrorKind::ConnectionFailure),
            _ => None,
        };
    }
    if error.is::<rustls::Error>() {
        return Some(NetworkErrorKind::TlsFailure);
    }
    let error = error.downcast_ref::<ureq::Error>()?;
    let direct = match error.kind() {
        ureq::ErrorKind::ResponseHeadTooLarge => Some(NetworkErrorKind::ResourceLimit(
            ResourceLimitKind::ResponseHead,
        )),
        ureq::ErrorKind::BodyMetadataTooLarge => Some(NetworkErrorKind::ResourceLimit(
            ResourceLimitKind::FramingMetadata,
        )),
        ureq::ErrorKind::BadHeader | ureq::ErrorKind::BadStatus | ureq::ErrorKind::BadBody => {
            Some(NetworkErrorKind::MalformedResponse)
        }
        ureq::ErrorKind::UnsupportedResponse => Some(NetworkErrorKind::UnsupportedResponse),
        _ => None,
    };
    if direct.is_some() {
        return direct;
    }
    if let Some(kind) = std::error::Error::source(error).and_then(typed_failure) {
        return Some(kind);
    }
    match error.kind() {
        ureq::ErrorKind::Dns => Some(NetworkErrorKind::DnsFailure),
        ureq::ErrorKind::ConnectionFailed => Some(NetworkErrorKind::ConnectionFailure),
        ureq::ErrorKind::Tls => Some(NetworkErrorKind::TlsFailure),
        _ => None,
    }
}

// The full request stays owned by the worker. Only admitted fields enter ureq;
// initiator/origin facts never implicitly generate policy or credential headers.
// Execute the complete current admitted target. Ureq preserves its provenance
// but excludes fragments from the wire. Original issued strings are correlation.
pub(crate) fn build_http_request(
    agent: &ureq::Agent,
    request: &FetchRequest,
    kind: ResourceKind,
) -> ureq::Request {
    request
        .validate_route(kind)
        .expect("validated execution route");
    let mut built = agent.get(request.current_url().as_str());
    for (name, value) in request.headers().iter() {
        let value = std::str::from_utf8(value).expect("admitted ASCII request field");
        built = built.set(name, value);
    }
    built
}

// The selected ureq feature set performs no content decompression. Preserve its
// admitted head, including encoded Content-Length and Content-Encoding. The
// pinned parser rejects malformed fields before any backend accessor runs.
// The byte accessor retains interleaved occurrences and obs-text without sorting,
// joining, UTF-8 filtering, or reconstructing fields through string accessors.
pub(crate) fn response_metadata(
    request: &FetchRequest,
    issued_url: &str,
    response: &ureq::Response,
) -> Result<NetworkResponseInfo, ResponseAdmissionError> {
    let status = HttpStatus::try_from(response.status())?;
    if status.is_informational() {
        return Err(ResponseAdmissionError::InformationalResponseUnsupported);
    }
    let mut fields = HttpHeaders::new();
    for (name, value) in response.headers_bytes() {
        fields
            .append(name, value)
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

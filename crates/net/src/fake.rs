//! Deterministic in-memory HTTP fixtures using the production Fetch lifecycle.
//!
//! Registration and acquisition perform no transport, TLS, or filesystem I/O.
//! Routes match AH's normalized fragmentless retrieval URL and the admitted GET
//! method. This identity is not a request identity or an HTTP cache key.
//! See `docs/platform/ai4-network-backend-and-deterministic-fake-network.md`.
//!
//! ```
//! use addressing::BrowserUrl;
//! use net::{HttpHeaders, HttpStatus, RequestMethod};
//! use net::fake::{FakeNetworkBuilder, FakeResponse};
//!
//! let url = BrowserUrl::parse("https://fixture.invalid/page#target")?.url;
//! let mut routes = FakeNetworkBuilder::new();
//! routes.register_response(
//!     &url,
//!     RequestMethod::Get,
//!     FakeResponse::new(
//!         HttpStatus::try_from(200)?, HttpHeaders::new(), vec![0, 255],
//!         Some("https://fixture.invalid/page".into()),
//!     ),
//! )?;
//! let network = routes.build();
//! // network.fetch_stream(request, route, cancellation, callback) uses the
//! // same response admission, reader, limits, and terminal handling as real HTTP.
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use std::{
    fmt,
    io::{self, Read},
    sync::{Arc, atomic::AtomicBool},
};

use addressing::{BrowserUrl, ResourceAddress, ResourceIdentityKey, ResourceKind as Purpose};
use core_types::ResourceKind;

use crate::{
    FetchRequest, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind, NetworkResponseInfo,
    RequestAdmissionError, RequestMethod, ResponseAdmissionError,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend},
    fetch::fetch_http_with_backend,
};

/// Invalid fixture configuration. Diagnostics contain no supplied contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeFixtureError {
    UnsupportedScheme,
    DuplicateRoute,
    InvalidBodyFailureOffset,
    InvalidNetworkErrorKind,
    InvalidBodyReadErrorKind,
}

impl fmt::Display for FakeFixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fake network fixture rejected: {self:?}")
    }
}
impl std::error::Error for FakeFixtureError {}

/// Mutable fixture registration, consumed to freeze a network for execution.
#[derive(Default)]
pub struct FakeNetworkBuilder {
    routes: Vec<(RouteKey, FakeOutcome)>,
}

impl FakeNetworkBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reject an existing normalized URL/method without changing its outcome.
    /// Registration does not admit a request or grant execution capability.
    pub fn register_response(
        &mut self,
        url: &BrowserUrl,
        method: RequestMethod,
        response: FakeResponse,
    ) -> Result<(), FakeFixtureError> {
        self.register(url, method, FakeOutcome::Response(response))
    }

    /// Configure an acquisition failure; this proves handling, not real transport.
    pub fn register_network_error(
        &mut self,
        url: &BrowserUrl,
        method: RequestMethod,
        kind: NetworkErrorKind,
    ) -> Result<(), FakeFixtureError> {
        if !matches!(
            kind,
            NetworkErrorKind::DnsFailure
                | NetworkErrorKind::ConnectionFailure
                | NetworkErrorKind::TlsFailure
                | NetworkErrorKind::Timeout
                | NetworkErrorKind::Backend
        ) {
            return Err(FakeFixtureError::InvalidNetworkErrorKind);
        }
        self.register(url, method, FakeOutcome::NetworkFailure(kind))
    }

    fn register(
        &mut self,
        url: &BrowserUrl,
        method: RequestMethod,
        outcome: FakeOutcome,
    ) -> Result<(), FakeFixtureError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(FakeFixtureError::UnsupportedScheme);
        }
        let key = RouteKey {
            url: ResourceAddress::new(url.clone(), Purpose::Unknown).retrieval_key(),
            method,
        };
        if self.routes.iter().any(|(registered, _)| registered == &key) {
            return Err(FakeFixtureError::DuplicateRoute);
        }
        self.routes.push((key, outcome));
        Ok(())
    }

    /// Empty networks are valid and fail every lookup predictably.
    pub fn build(self) -> FakeNetwork {
        FakeNetwork {
            routes: self.routes.into(),
        }
    }
}

/// Actual entry and return at the fake acquisition boundary, scoped to one call.
/// These records do not prove transmission, body completion or consumer acceptance.
#[derive(Debug)]
pub enum FakeAcquisitionEvent {
    Entered {
        request: Box<FetchRequest>,
        route: ResourceKind,
    },
    Returned {
        result: FakeAcquisitionResult,
    },
}

/// Actual fake lookup/metadata construction result, before shared Fetch admission.
/// Debug delegates to existing redacted types and never includes body contents.
#[derive(Debug)]
pub enum FakeAcquisitionResult {
    Response(NetworkResponseInfo),
    NetworkError(NetworkErrorKind),
    MissingRoute,
    ResponseConstructionRejected(ResponseAdmissionError),
}

type AcquisitionObserver = Arc<dyn Fn(FakeAcquisitionEvent) + Send + Sync>;

// Only this invocation owns the observer; shared route definitions remain immutable.
struct ObservedFakeNetwork {
    network: FakeNetwork,
    observer: AcquisitionObserver,
}
impl HttpResponseBackend for ObservedFakeNetwork {
    fn acquire(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        (self.observer)(FakeAcquisitionEvent::Entered {
            request: Box::new(request.clone()),
            route: kind,
        });
        self.network
            .acquire_recorded(request, kind, Some(&self.observer))
    }
}

/// Immutable routes with independent body readers for every execution.
/// Clones share fixture data, never cursor, request provenance, or call order.
#[derive(Clone)]
pub struct FakeNetwork {
    routes: Arc<[(RouteKey, FakeOutcome)]>,
}

impl FakeNetwork {
    /// Execute an admitted HTTP(S) request through the production Fetch worker.
    /// File requests fail synchronously with UnsupportedScheme; they cannot
    /// reach the real local-file loader. There is no real-backend fallback.
    pub fn fetch_stream(
        &self,
        request: FetchRequest,
        kind: ResourceKind,
        cancel_token: Arc<AtomicBool>,
        callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
    ) -> Result<(), RequestAdmissionError> {
        fetch_http_with_backend(request, kind, self.clone(), cancel_token, callback)
    }

    /// Observe actual individual acquisitions without changing Fetch execution.
    /// Callbacks run synchronously on the Fetch worker. Callers must release
    /// their callback clones to observe producer closure; no worker join is exposed.
    pub fn fetch_stream_observed(
        &self,
        request: FetchRequest,
        kind: ResourceKind,
        cancel_token: Arc<AtomicBool>,
        callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
        acquisitions: Arc<dyn Fn(FakeAcquisitionEvent) + Send + Sync>,
    ) -> Result<(), RequestAdmissionError> {
        fetch_http_with_backend(
            request,
            kind,
            ObservedFakeNetwork {
                network: self.clone(),
                observer: acquisitions,
            },
            cancel_token,
            callback,
        )
    }
}

/// Acquired HTTP facts and finite content-encoded bytes after transfer framing,
/// not a completed Fetch result. gzip fixtures supply gzip bytes; shared Fetch
/// performs the same decoding as real HTTP. Content-Length remains encoded.
/// Reported URL provenance is explicit: missing/empty/invalid values reach the
/// same production response validation as real acquisition. Shared Fetch resolves
/// and follows Location through subsequent acquisitions; this backend only looks
/// up the current admitted route and never falls back to real I/O.
#[derive(Clone)]
pub struct FakeResponse {
    status: HttpStatus,
    headers: HttpHeaders,
    body: Arc<[u8]>,
    reported_response_url: Option<String>,
    read_failure_after: Option<(usize, io::ErrorKind)>,
}

impl FakeResponse {
    pub fn new(
        status: HttpStatus,
        headers: HttpHeaders,
        body: Vec<u8>,
        reported_response_url: Option<String>,
    ) -> Self {
        Self {
            status,
            headers,
            body: body.into(),
            reported_response_url,
            read_failure_after: None,
        }
    }

    /// Fail at this encoded-byte offset, including zero or EOF. An offset beyond
    /// the finite fixture body is rejected. 204/304 bypass this reader; 205 must
    /// verify its framing source contains no content before any decoding.
    pub fn with_read_failure_after(self, encoded_bytes: usize) -> Result<Self, FakeFixtureError> {
        self.with_read_error_after(encoded_bytes, io::ErrorKind::Other)
    }

    /// A fresh fixed error at an encoded offset. Persistent Interrupted faults
    /// would prevent progress forever and are rejected rather than scheduled.
    pub fn with_read_error_after(
        mut self,
        encoded_bytes: usize,
        kind: io::ErrorKind,
    ) -> Result<Self, FakeFixtureError> {
        if encoded_bytes > self.body.len() {
            return Err(FakeFixtureError::InvalidBodyFailureOffset);
        }
        if !matches!(
            kind,
            io::ErrorKind::Other
                | io::ErrorKind::TimedOut
                | io::ErrorKind::ConnectionRefused
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::NotConnected
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::WouldBlock
                | io::ErrorKind::UnexpectedEof
                | io::ErrorKind::InvalidData
        ) {
            return Err(FakeFixtureError::InvalidBodyReadErrorKind);
        }
        self.read_failure_after = Some((encoded_bytes, kind));
        Ok(self)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct RouteKey {
    url: ResourceIdentityKey,
    method: RequestMethod,
}

enum FakeOutcome {
    Response(FakeResponse),
    NetworkFailure(NetworkErrorKind),
}

impl HttpResponseBackend for FakeNetwork {
    fn acquire(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        self.acquire_recorded(request, kind, None)
    }
}

impl FakeNetwork {
    fn acquire_recorded(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
        observer: Option<&AcquisitionObserver>,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let key = RouteKey {
            url: request.transport_target(),
            method: request.method(),
        };
        let outcome = self
            .routes
            .iter()
            .find(|(registered, _)| registered == &key);
        let emit = |result| {
            if let Some(observer) = observer {
                observer(FakeAcquisitionEvent::Returned { result });
            }
        };
        match outcome {
            None => {
                emit(FakeAcquisitionResult::MissingRoute);
                Err(HttpAcquireError::Network {
                    kind: NetworkErrorKind::Backend,
                    error: "fake network route not found".into(),
                })
            }
            Some((_, FakeOutcome::NetworkFailure(kind))) => {
                emit(FakeAcquisitionResult::NetworkError(*kind));
                Err(HttpAcquireError::Network {
                    kind: *kind,
                    error: kind.diagnostic().into(),
                })
            }
            Some((_, FakeOutcome::Response(response))) => {
                let metadata = match NetworkResponseInfo::http(
                    request.address(),
                    request
                        .legacy_issued_url(kind)
                        .expect("validated HTTP route"),
                    response.reported_response_url.clone(),
                    response.status,
                    response.headers.clone(),
                ) {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        emit(FakeAcquisitionResult::ResponseConstructionRejected(e));
                        return Err(HttpAcquireError::Response(e));
                    }
                };
                let acquired = AcquiredHttpResponse {
                    metadata,
                    reader: Box::new(FakeBodyReader {
                        bytes: response.body.clone(),
                        position: 0,
                        fail_after: response.read_failure_after,
                    }),
                };
                if observer.is_some() {
                    emit(FakeAcquisitionResult::Response(acquired.metadata.clone()));
                }
                Ok(acquired)
            }
        }
    }
}

struct FakeBodyReader {
    bytes: Arc<[u8]>,
    position: usize,
    fail_after: Option<(usize, io::ErrorKind)>,
}

impl Read for FakeBodyReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if let Some((offset, kind)) = self.fail_after
            && self.position >= offset
        {
            return Err(io::Error::new(kind, "fake body read failure"));
        }
        let end = self
            .fail_after
            .map_or(self.bytes.len(), |(offset, _)| offset);
        let count = output.len().min(end - self.position);
        output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

impl fmt::Debug for FakeNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeNetwork")
            .field("routes", &self.routes.len())
            .finish()
    }
}
impl fmt::Debug for FakeNetworkBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeNetworkBuilder")
            .field("routes", &self.routes.len())
            .finish()
    }
}
impl fmt::Debug for FakeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body_bytes", &self.body.len())
            .field(
                "reported_url_present",
                &self.reported_response_url.is_some(),
            )
            .field(
                "read_failure_configured",
                &self.read_failure_after.is_some(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai7_reader_empty_reads_and_failure_at_eof_do_not_establish_completion() {
        for offset in [0, 3] {
            let mut reader = FakeBodyReader {
                bytes: Arc::from(b"abc".as_slice()),
                position: 0,
                fail_after: Some((offset, io::ErrorKind::TimedOut)),
            };
            assert_eq!(reader.read(&mut []).unwrap(), 0);
            if offset != 0 {
                assert_eq!(reader.read(&mut [0; 3]).unwrap(), 3);
            }
            for _ in 0..2 {
                assert_eq!(
                    reader.read(&mut [0]).unwrap_err().kind(),
                    io::ErrorKind::TimedOut
                );
                assert_eq!(reader.read(&mut []).unwrap(), 0);
            }
        }
    }
}

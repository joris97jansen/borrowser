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
    RequestAdmissionError, RequestMethod,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend},
    fetch::fetch_http_with_backend,
};

/// Invalid fixture configuration. Diagnostics contain no supplied contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeFixtureError {
    UnsupportedScheme,
    DuplicateRoute,
    InvalidBodyFailureOffset,
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

    /// Configure a pre-response Transport failure with a fixed safe diagnostic.
    pub fn register_network_error(
        &mut self,
        url: &BrowserUrl,
        method: RequestMethod,
    ) -> Result<(), FakeFixtureError> {
        self.register(url, method, FakeOutcome::NetworkFailure)
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
}

/// Acquired HTTP facts and finite content bytes, not a completed Fetch result.
/// Reported URL provenance is explicit: missing/empty/invalid values reach the
/// same production response validation as real acquisition. Location is inert.
#[derive(Clone)]
pub struct FakeResponse {
    status: HttpStatus,
    headers: HttpHeaders,
    body: Arc<[u8]>,
    reported_response_url: Option<String>,
    read_failure_after: Option<usize>,
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

    /// Fail reading at exactly this content-byte offset, including zero or EOF.
    /// An offset beyond the finite fixture body is rejected. Null-body statuses
    /// bypass this reader under Fetch's existing body semantics.
    pub fn with_read_failure_after(
        mut self,
        delivered_bytes: usize,
    ) -> Result<Self, FakeFixtureError> {
        if delivered_bytes > self.body.len() {
            return Err(FakeFixtureError::InvalidBodyFailureOffset);
        }
        self.read_failure_after = Some(delivered_bytes);
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
    NetworkFailure,
}

impl HttpResponseBackend for FakeNetwork {
    fn acquire(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let key = RouteKey {
            url: request.transport_target(),
            method: request.method(),
        };
        let outcome = self
            .routes
            .iter()
            .find(|(registered, _)| registered == &key);
        let response = match outcome {
            None => {
                return Err(HttpAcquireError::Network {
                    kind: NetworkErrorKind::Transport,
                    error: "fake network route not found".into(),
                });
            }
            Some((_, FakeOutcome::NetworkFailure)) => {
                return Err(HttpAcquireError::Network {
                    kind: NetworkErrorKind::Transport,
                    error: "fake network failure".into(),
                });
            }
            Some((_, FakeOutcome::Response(response))) => response,
        };
        let metadata = NetworkResponseInfo::http(
            request.original_url().clone(),
            request
                .legacy_issued_url(kind)
                .expect("validated HTTP route"),
            response.reported_response_url.clone(),
            response.status,
            response.headers.clone(),
        )
        .map_err(HttpAcquireError::Response)?;
        Ok(AcquiredHttpResponse {
            metadata,
            reader: Box::new(FakeBodyReader {
                bytes: response.body.clone(),
                position: 0,
                fail_after: response.read_failure_after,
            }),
        })
    }
}

struct FakeBodyReader {
    bytes: Arc<[u8]>,
    position: usize,
    fail_after: Option<usize>,
}

impl Read for FakeBodyReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.fail_after.is_some_and(|n| self.position >= n) {
            return Err(io::Error::other("fake body read failure"));
        }
        let end = self.fail_after.unwrap_or(self.bytes.len());
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

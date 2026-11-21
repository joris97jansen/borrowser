//! Exact controlled synthetic observations. No production Debug output is parsed.
use super::{
    bounds,
    input::{Header, Route},
};
use crate::{addressing::observations as ah, model::HarnessError};
use net::{FetchRequest, NetworkErrorKind as N, NetworkResponseInfo};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) fn error(detail: impl Into<String>) -> HarnessError {
    let detail = detail.into();
    if detail.len() > bounds::DIAGNOSTIC {
        HarnessError::new(
            "networking.diagnostic-budget",
            "harness diagnostic exceeds byte budget",
        )
    } else {
        HarnessError::new("networking.required-data", detail)
    }
}
fn observation_rows<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<FetchObservation>, D::Error> {
    super::input::sequence::<D, FetchObservation, 32>(d)
}
fn attempts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Attempt>, D::Error> {
    super::input::sequence::<D, Attempt, 64>(d)
}
fn hops<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Hop>, D::Error> {
    super::input::sequence::<D, Hop, 64>(d)
}
fn fields<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Header>, D::Error> {
    super::input::sequence::<D, Header, 64>(d)
}
fn diagnostics<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    super::input::sequence::<D, String, 64>(d)
}
fn small_bytes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    super::input::sequence::<D, u8, 64>(d)
}
fn preview<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    super::input::sequence::<D, u8, 32>(d)
}
fn optional<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Available<T> {
    Present(T),
    Unavailable(String),
    Inapplicable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestObservation {
    pub original_url: String,
    pub current_url: String,
    pub transport_target: String,
    pub purpose: String,
    pub route: Route,
    pub method: String,
    #[serde(deserialize_with = "fields")]
    pub headers: Vec<Header>,
    #[serde(deserialize_with = "optional")]
    pub initiator_url: Option<String>,
    #[serde(deserialize_with = "optional")]
    pub requesting_origin: Option<OriginObservation>,
    pub redirect_policy: String,
    pub has_body: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginObservation {
    pub kind: String,
    pub serialization: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ResponseUrl {
    Missing,
    Parsed {
        url: String,
        #[serde(deserialize_with = "diagnostics")]
        diagnostics: Vec<String>,
    },
    Invalid {
        failure: String,
        #[serde(deserialize_with = "diagnostics")]
        diagnostics: Vec<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Mime {
    Missing,
    Empty,
    Unusable,
    Parsed {
        mime_type: String,
        #[serde(deserialize_with = "optional")]
        charset: Option<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Length {
    Missing,
    Unusable,
    Conflicting,
    OutOfRange,
    Value(u64),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Hop {
    pub request_url: String,
    pub response_url: String,
    pub status: u16,
    pub target_url: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResponseObservation {
    pub original_url: String,
    pub purpose: String,
    pub requested_url: String,
    #[serde(deserialize_with = "optional")]
    pub reported_response_url: Option<String>,
    pub response_url: ResponseUrl,
    pub status: u16,
    pub status_class: String,
    #[serde(deserialize_with = "fields")]
    pub headers: Vec<Header>,
    pub content_type: Mime,
    pub declared_content_length: Length,
    #[serde(deserialize_with = "hops")]
    pub redirect_chain: Vec<Hop>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "reason",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum NetworkError {
    Cancelled,
    DnsFailure,
    ConnectionFailure,
    TlsFailure,
    Timeout,
    Backend,
    LocalFile,
    Read,
    ResourceLimit(Limit),
    MalformedResponse,
    ContentDecoding,
    UnsupportedEncoding,
    UnsupportedResponse,
    Redirect(RedirectReason),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Limit {
    EncodedBody,
    DecodedBody,
    ResponseHead,
    FramingMetadata,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RedirectReason {
    InvalidLocation,
    RepeatedLocation,
    UnsupportedScheme,
    UrlCredentialsUnsupported,
    LimitExceeded,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "reason",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Admission {
    UnsupportedMethod,
    UnsupportedScheme,
    UrlCredentialsUnsupported,
    UnsupportedHeader,
    UnsupportedHeaderValue,
    RepeatedHeaderUnsupported,
    HeadersNotApplicable,
    Headers(HeaderFailure),
    IncompatibleExecutionRoute,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HeaderFailure {
    InvalidName,
    InvalidValue,
    TooManyFields,
    NameTooLong,
    ValueTooLong,
    ListTooLarge,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "reason",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum ResponseFailure {
    InvalidStatus,
    InformationalResponseUnsupported,
    IncompatibleOriginalScheme,
    MissingResponseUrl,
    InvalidResponseUrl,
    UnsupportedResponseScheme,
    HeaderValuesUnavailable,
    Headers(HeaderFailure),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum AcquisitionResult {
    Response(Box<ResponseObservation>),
    NetworkError(NetworkError),
    MissingRoute,
    ResponseConstructionRejected(ResponseFailure),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Attempt {
    pub request: RequestObservation,
    pub result: AcquisitionResult,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Terminal {
    ConstructionRejected {
        error: Admission,
    },
    DispatchRejected {
        error: Admission,
    },
    UrlRejected {
        failure: String,
        #[serde(deserialize_with = "diagnostics")]
        diagnostics: Vec<String>,
    },
    Done {
        response: ResponseObservation,
        progress: Progress,
    },
    Error {
        error: NetworkError,
        response: Available<ResponseObservation>,
        progress: Available<Progress>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Progress {
    pub route: Route,
    pub delivered_bytes: usize,
    pub limit_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Body {
    Bytes {
        #[serde(deserialize_with = "small_bytes")]
        bytes: Vec<u8>,
    },
    Summary {
        length: usize,
        sha256: String,
        #[serde(deserialize_with = "preview")]
        prefix: Vec<u8>,
        #[serde(deserialize_with = "preview")]
        suffix: Vec<u8>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FetchObservation {
    pub request: Available<RequestObservation>,
    #[serde(deserialize_with = "attempts")]
    pub acquisitions: Vec<Attempt>,
    pub start: Available<ResponseObservation>,
    pub terminal: Terminal,
    pub selected_final_url: Available<String>,
    pub body: Body,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NetworkingObservation {
    #[serde(deserialize_with = "observation_rows")]
    pub rows: Vec<FetchObservation>,
}

// Charge all owned capacities (including vector slots) of the closed projection.
// No allocator/RSS claim is made. Strings are checked before copying.
struct Charge(usize);
impl Charge {
    fn add(&mut self, n: usize) -> Result<(), HarnessError> {
        self.0 = self
            .0
            .checked_add(n)
            .ok_or_else(|| error("storage overflow"))?;
        if self.0 > bounds::RECORD {
            return Err(error("record storage budget exceeded"));
        }
        Ok(())
    }
    fn text(&mut self, s: &str) -> Result<String, HarnessError> {
        self.add(s.len() + size_of::<String>())?;
        Ok(s.into())
    }
    fn diagnostics(
        &mut self,
        d: &[addressing::UrlDiagnostic],
    ) -> Result<Vec<String>, HarnessError> {
        if d.len() > 64 {
            return Err(error("URL diagnostic cardinality budget exceeded"));
        }
        self.add(d.len() * (size_of::<String>() + 64))?;
        Ok(ah::diagnostics(d))
    }
    fn headers(&mut self, h: &net::HttpHeaders) -> Result<Vec<Header>, HarnessError> {
        self.add(h.iter().len() * size_of::<Header>())?;
        h.iter()
            .map(|(name, value)| {
                let name = self.text(name)?;
                self.add(value.len())?;
                Ok(Header(name, value.to_vec()))
            })
            .collect()
    }
}
impl RequestObservation {
    pub(crate) fn project(r: &FetchRequest, route: Route) -> Result<Self, HarnessError> {
        let mut c = Charge(size_of::<Self>());
        Ok(Self {
            original_url: c.text(r.original_url().as_str())?,
            current_url: c.text(r.current_url().as_str())?,
            transport_target: c.text(r.transport_target().as_str())?,
            purpose: c.text(ah::kind(r.address().kind()))?,
            route,
            method: c.text(match r.method() {
                net::RequestMethod::Get => "GET",
            })?,
            headers: c.headers(r.headers())?,
            initiator_url: r.initiator_url().map(|u| c.text(u.as_str())).transpose()?,
            requesting_origin: r
                .requesting_origin()
                .map(|o| {
                    Ok::<_, HarnessError>(OriginObservation {
                        kind: c.text(if o.is_opaque() { "opaque" } else { "tuple" })?,
                        serialization: c.text(&o.serialize())?,
                    })
                })
                .transpose()?,
            redirect_policy: c.text(match r.redirect_policy() {
                net::RedirectPolicy::Follow => "follow",
            })?,
            has_body: r.has_body(),
        })
    }
}
impl ResponseObservation {
    pub(crate) fn project(r: &NetworkResponseInfo) -> Result<Self, HarnessError> {
        let mut c = Charge(size_of::<Self>());
        let response_url = match r.response_url() {
            None => ResponseUrl::Missing,
            Some(Ok(p)) => ResponseUrl::Parsed {
                url: c.text(p.url.as_str())?,
                diagnostics: c.diagnostics(&p.diagnostics)?,
            },
            Some(Err(e)) => ResponseUrl::Invalid {
                failure: ah::failure(e)?,
                diagnostics: c.diagnostics(&e.diagnostics)?,
            },
        };
        let content_type = match r.content_type() {
            net::ResponseContentType::Missing => Mime::Missing,
            net::ResponseContentType::Empty => Mime::Empty,
            net::ResponseContentType::Unusable => Mime::Unusable,
            net::ResponseContentType::Parsed(m) => Mime::Parsed {
                mime_type: c.text(m.mime_type())?,
                charset: m.charset().map(|s| c.text(s)).transpose()?,
            },
        };
        let declared_content_length = match r.declared_content_length() {
            net::DeclaredContentLength::Missing => Length::Missing,
            net::DeclaredContentLength::Unusable => Length::Unusable,
            net::DeclaredContentLength::Conflicting => Length::Conflicting,
            net::DeclaredContentLength::OutOfRange => Length::OutOfRange,
            net::DeclaredContentLength::Value(v) => Length::Value(v),
        };
        if r.redirect_chain().len() > 64 {
            return Err(error("redirect observation budget exceeded"));
        }
        c.add(r.redirect_chain().len() * size_of::<Hop>())?;
        let redirect_chain = r
            .redirect_chain()
            .iter()
            .map(|h| {
                Ok(Hop {
                    request_url: c.text(h.request_url().as_str())?,
                    response_url: c.text(h.response_url().as_str())?,
                    status: h.status().code(),
                    target_url: c.text(h.target_url().as_str())?,
                })
            })
            .collect::<Result<_, HarnessError>>()?;
        let status = r
            .status()
            .ok_or_else(|| error("HTTP response status missing"))?;
        let status_class = match status.class() {
            net::HttpStatusClass::Informational => "informational",
            net::HttpStatusClass::Successful => "successful",
            net::HttpStatusClass::Redirection => "redirection",
            net::HttpStatusClass::ClientError => "client_error",
            net::HttpStatusClass::ServerError => "server_error",
        };
        Ok(Self {
            original_url: c.text(r.original_url().as_str())?,
            purpose: c.text(ah::kind(r.resource_kind()))?,
            requested_url: c.text(r.requested_url())?,
            reported_response_url: r.reported_response_url().map(|s| c.text(s)).transpose()?,
            response_url,
            status: status.code(),
            status_class: c.text(status_class)?,
            headers: c.headers(r.headers())?,
            content_type,
            declared_content_length,
            redirect_chain,
        })
    }
}
impl From<N> for NetworkError {
    fn from(e: N) -> Self {
        match e {
            N::Cancelled => Self::Cancelled,
            N::DnsFailure => Self::DnsFailure,
            N::ConnectionFailure => Self::ConnectionFailure,
            N::TlsFailure => Self::TlsFailure,
            N::Timeout => Self::Timeout,
            N::Backend => Self::Backend,
            N::LocalFile => Self::LocalFile,
            N::Read => Self::Read,
            N::MalformedResponse => Self::MalformedResponse,
            N::ContentDecoding => Self::ContentDecoding,
            N::UnsupportedEncoding => Self::UnsupportedEncoding,
            N::UnsupportedResponse => Self::UnsupportedResponse,
            N::ResourceLimit(l) => Self::ResourceLimit(match l {
                net::ResourceLimitKind::EncodedBody => Limit::EncodedBody,
                net::ResourceLimitKind::DecodedBody => Limit::DecodedBody,
                net::ResourceLimitKind::ResponseHead => Limit::ResponseHead,
                net::ResourceLimitKind::FramingMetadata => Limit::FramingMetadata,
            }),
            N::Redirect(r) => Self::Redirect(match r {
                net::RedirectError::InvalidLocation => RedirectReason::InvalidLocation,
                net::RedirectError::RepeatedLocation => RedirectReason::RepeatedLocation,
                net::RedirectError::UnsupportedScheme => RedirectReason::UnsupportedScheme,
                net::RedirectError::UrlCredentialsUnsupported => {
                    RedirectReason::UrlCredentialsUnsupported
                }
                net::RedirectError::LimitExceeded => RedirectReason::LimitExceeded,
            }),
        }
    }
}
impl From<net::HeaderError> for HeaderFailure {
    fn from(e: net::HeaderError) -> Self {
        match e {
            net::HeaderError::InvalidName => Self::InvalidName,
            net::HeaderError::InvalidValue => Self::InvalidValue,
            net::HeaderError::TooManyFields => Self::TooManyFields,
            net::HeaderError::NameTooLong => Self::NameTooLong,
            net::HeaderError::ValueTooLong => Self::ValueTooLong,
            net::HeaderError::ListTooLarge => Self::ListTooLarge,
        }
    }
}
impl From<net::RequestAdmissionError> for Admission {
    fn from(e: net::RequestAdmissionError) -> Self {
        use net::RequestAdmissionError as A;
        match e {
            A::UnsupportedMethod => Self::UnsupportedMethod,
            A::UnsupportedScheme => Self::UnsupportedScheme,
            A::UrlCredentialsUnsupported => Self::UrlCredentialsUnsupported,
            A::UnsupportedHeader => Self::UnsupportedHeader,
            A::UnsupportedHeaderValue => Self::UnsupportedHeaderValue,
            A::RepeatedHeaderUnsupported => Self::RepeatedHeaderUnsupported,
            A::HeadersNotApplicable => Self::HeadersNotApplicable,
            A::Headers(h) => Self::Headers(h.into()),
            A::IncompatibleExecutionRoute => Self::IncompatibleExecutionRoute,
        }
    }
}
impl From<net::ResponseAdmissionError> for ResponseFailure {
    fn from(e: net::ResponseAdmissionError) -> Self {
        use net::ResponseAdmissionError as A;
        match e {
            A::InvalidStatus => Self::InvalidStatus,
            A::InformationalResponseUnsupported => Self::InformationalResponseUnsupported,
            A::IncompatibleOriginalScheme => Self::IncompatibleOriginalScheme,
            A::MissingResponseUrl => Self::MissingResponseUrl,
            A::InvalidResponseUrl => Self::InvalidResponseUrl,
            A::UnsupportedResponseScheme => Self::UnsupportedResponseScheme,
            A::HeaderValuesUnavailable => Self::HeaderValuesUnavailable,
            A::Headers(h) => Self::Headers(h.into()),
        }
    }
}
impl From<net::BodyProgress> for Progress {
    fn from(p: net::BodyProgress) -> Self {
        Self {
            route: Route::from(p.route()),
            delivered_bytes: p.delivered_bytes(),
            limit_bytes: p.limit_bytes(),
        }
    }
}

pub(crate) struct BodyAccumulator {
    length: usize,
    hash: Sha256,
    first: Vec<u8>,
    last: Vec<u8>,
}
impl BodyAccumulator {
    pub(crate) fn new() -> Self {
        Self {
            length: 0,
            hash: Sha256::new(),
            first: Vec::with_capacity(64),
            last: Vec::with_capacity(32),
        }
    }
    pub(crate) fn update(&mut self, bytes: &[u8]) -> Result<(), HarnessError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or_else(|| error("body accounting overflow"))?;
        self.hash.update(bytes);
        self.first
            .extend_from_slice(&bytes[..bytes.len().min(64 - self.first.len())]);
        if bytes.len() >= 32 {
            self.last.clear();
            self.last.extend_from_slice(&bytes[bytes.len() - 32..]);
        } else {
            let excess = (self.last.len() + bytes.len()).saturating_sub(32);
            self.last.drain(..excess);
            self.last.extend_from_slice(bytes);
        }
        Ok(())
    }
    pub(crate) fn len(&self) -> usize {
        self.length
    }
    pub(crate) fn finish(self) -> Body {
        if self.length <= 64 {
            Body::Bytes { bytes: self.first }
        } else {
            Body::Summary {
                length: self.length,
                sha256: format!("{:x}", self.hash.finalize()),
                prefix: self.first[..32].to_vec(),
                suffix: self.last,
            }
        }
    }
}

#[allow(
    clippy::ptr_arg,
    reason = "The harness charges owned vector capacity, not only initialized elements"
)]
fn header_storage(h: &Vec<Header>) -> usize {
    h.capacity() * size_of::<Header>()
        + h.iter()
            .map(|h| h.0.capacity() + h.1.capacity())
            .sum::<usize>()
}
#[allow(clippy::ptr_arg, reason = "The harness charges owned vector capacity")]
fn diagnostic_storage(d: &Vec<String>) -> usize {
    d.capacity() * size_of::<String>() + d.iter().map(String::capacity).sum::<usize>()
}
pub(crate) fn request_storage(r: &RequestObservation) -> usize {
    size_of::<RequestObservation>()
        + r.original_url.capacity()
        + r.current_url.capacity()
        + r.transport_target.capacity()
        + r.purpose.capacity()
        + r.method.capacity()
        + r.redirect_policy.capacity()
        + header_storage(&r.headers)
        + r.initiator_url.as_ref().map_or(0, String::capacity)
        + r.requesting_origin
            .as_ref()
            .map_or(0, |o| o.kind.capacity() + o.serialization.capacity())
}
pub(crate) fn response_storage(r: &ResponseObservation) -> usize {
    size_of::<ResponseObservation>()
        + r.original_url.capacity()
        + r.purpose.capacity()
        + r.requested_url.capacity()
        + r.reported_response_url.as_ref().map_or(0, String::capacity)
        + r.status_class.capacity()
        + header_storage(&r.headers)
        + match &r.response_url {
            ResponseUrl::Missing => 0,
            ResponseUrl::Parsed { url, diagnostics } => {
                url.capacity() + diagnostic_storage(diagnostics)
            }
            ResponseUrl::Invalid {
                failure,
                diagnostics,
            } => failure.capacity() + diagnostic_storage(diagnostics),
        }
        + match &r.content_type {
            Mime::Parsed { mime_type, charset } => {
                mime_type.capacity() + charset.as_ref().map_or(0, String::capacity)
            }
            _ => 0,
        }
        + r.redirect_chain.capacity() * size_of::<Hop>()
        + r.redirect_chain
            .iter()
            .map(|h| h.request_url.capacity() + h.response_url.capacity() + h.target_url.capacity())
            .sum::<usize>()
}
pub(crate) fn available_response_storage(r: &Available<ResponseObservation>) -> usize {
    match r {
        Available::Present(r) => response_storage(r),
        Available::Unavailable(s) => s.capacity(),
        Available::Inapplicable => 0,
    }
}
pub(crate) fn available_progress_storage(p: &Available<Progress>) -> usize {
    match p {
        Available::Unavailable(reason) => reason.capacity(),
        _ => 0,
    }
}
fn terminal_storage(t: &Terminal) -> usize {
    size_of::<Terminal>()
        + match t {
            Terminal::Done { response, .. } => response_storage(response),
            Terminal::Error {
                response, progress, ..
            } => available_response_storage(response) + available_progress_storage(progress),
            Terminal::UrlRejected {
                failure,
                diagnostics,
            } => failure.capacity() + diagnostic_storage(diagnostics),
            _ => 0,
        }
}
impl FetchObservation {
    pub(crate) fn storage(&self) -> usize {
        size_of::<Self>()
            + match &self.request {
                Available::Present(r) => request_storage(r),
                Available::Unavailable(s) => s.capacity(),
                _ => 0,
            }
            + self.acquisitions.capacity() * size_of::<Attempt>()
            + self
                .acquisitions
                .iter()
                .map(|a| {
                    request_storage(&a.request)
                        + match &a.result {
                            AcquisitionResult::Response(r) => response_storage(r),
                            _ => 0,
                        }
                })
                .sum::<usize>()
            + available_response_storage(&self.start)
            + terminal_storage(&self.terminal)
            + match &self.selected_final_url {
                Available::Present(s) | Available::Unavailable(s) => s.capacity(),
                _ => 0,
            }
            + match &self.body {
                Body::Bytes { bytes } => bytes.capacity(),
                Body::Summary {
                    sha256,
                    prefix,
                    suffix,
                    ..
                } => sha256.capacity() + prefix.capacity() + suffix.capacity(),
            }
    }
    pub(crate) fn validate(&self) -> Result<(), HarnessError> {
        if self.acquisitions.len() > 64 {
            return Err(error("acquisition observation budget exceeded"));
        }
        if self.storage() > bounds::ROW_STORAGE {
            return Err(error("row storage budget exceeded"));
        }
        let request_bytes = match &self.request {
            Available::Present(r) => request_storage(r),
            Available::Unavailable(reason) => reason.capacity(),
            Available::Inapplicable => 0,
        };
        if request_bytes > bounds::RECORD || terminal_storage(&self.terminal) > bounds::RECORD {
            return Err(error("request or terminal record budget exceeded"));
        }
        bounds::json(self, bounds::ROW_JSON).map_err(error)?;
        match &self.body {
            Body::Bytes { bytes } if bytes.len() > 64 => {
                return Err(error("large body must use a summary"));
            }
            Body::Summary {
                length,
                sha256,
                prefix,
                suffix,
            } if *length <= 64
                || sha256.len() != 64
                || !sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || prefix.len() != 32
                || suffix.len() != 32 =>
            {
                return Err(error("invalid body summary"));
            }
            _ => {}
        }
        if matches!(
            self.terminal,
            Terminal::ConstructionRejected { .. }
                | Terminal::DispatchRejected { .. }
                | Terminal::UrlRejected { .. }
        ) && (!self.acquisitions.is_empty()
            || self.start != Available::Inapplicable
            || self.body != (Body::Bytes { bytes: vec![] }))
        {
            return Err(error("rejection contains worker observations"));
        }
        for a in &self.acquisitions {
            if request_storage(&a.request) > bounds::RECORD {
                return Err(error("request record budget exceeded"));
            }
            if let AcquisitionResult::Response(r) = &a.result {
                validate_response(r)?;
            }
        }
        if let Available::Present(r) = &self.start {
            validate_response(r)?;
        }
        match &self.terminal {
            Terminal::Done { response, .. } => validate_response(response)?,
            Terminal::Error {
                response: Available::Present(r),
                ..
            } => validate_response(r)?,
            _ => {}
        }
        self.validate_lifecycle()
    }

    /// Evidence consistency only: Fetch still decides admission, redirects,
    /// metadata interpretation and resource policy. Complete semantic differences
    /// are left to independent expectation comparison.
    fn validate_lifecycle(&self) -> Result<(), HarnessError> {
        match &self.terminal {
            Terminal::ConstructionRejected { .. } | Terminal::UrlRejected { .. } => {
                if !matches!(self.request, Available::Unavailable(_))
                    || self.selected_final_url != Available::Inapplicable
                {
                    return Err(error(
                        "construction rejection has admitted request or final URL evidence",
                    ));
                }
                return Ok(());
            }
            Terminal::DispatchRejected { .. } => {
                if !matches!(self.request, Available::Present(_))
                    || self.selected_final_url != Available::Inapplicable
                {
                    return Err(error(
                        "dispatch rejection requires admitted request and no final URL",
                    ));
                }
                return Ok(());
            }
            Terminal::Done { .. } | Terminal::Error { .. } => {}
        }
        let Available::Present(request) = &self.request else {
            return Err(error(
                "accepted execution requires admitted request evidence",
            ));
        };
        // Entries before the final acquisition must have returned metadata:
        // a failed acquisition cannot be followed by another acquisition.
        if self
            .acquisitions
            .iter()
            .rev()
            .skip(1)
            .any(|a| !matches!(a.result, AcquisitionResult::Response(_)))
        {
            return Err(error("acquisition after a failed acquisition"));
        }
        let delivered = match &self.body {
            Body::Bytes { bytes } => bytes.len(),
            Body::Summary { length, .. } => *length,
        };
        let check_progress = |p: &Progress| {
            if p.route != request.route || p.delivered_bytes != delivered {
                Err(error(
                    "body progress contradicts delivered body or request route",
                ))
            } else {
                Ok(())
            }
        };
        match &self.start {
            Available::Present(start) => {
                if !matches!(
                    self.acquisitions.last().map(|a| &a.result),
                    Some(AcquisitionResult::Response(_))
                ) {
                    return Err(error("Start requires acquired response evidence"));
                }
                match &self.terminal {
                    Terminal::Done { response, progress } if response == start => {
                        check_progress(progress)?
                    }
                    Terminal::Error {
                        response: Available::Present(response),
                        progress: Available::Present(progress),
                        ..
                    } if response == start => check_progress(progress)?,
                    _ => {
                        return Err(error(
                            "post-Start terminal requires immutable response and body progress evidence",
                        ));
                    }
                }
                let final_available = matches!(self.selected_final_url, Available::Present(_));
                let final_unavailable =
                    matches!(self.selected_final_url, Available::Unavailable(_));
                if match &start.response_url {
                    ResponseUrl::Parsed { .. } => !final_available,
                    ResponseUrl::Missing | ResponseUrl::Invalid { .. } => !final_unavailable,
                } {
                    return Err(error(
                        "selected final URL evidence contradicts Start availability",
                    ));
                }
            }
            Available::Inapplicable => {
                let Terminal::Error {
                    error: kind,
                    response,
                    progress,
                } = &self.terminal
                else {
                    return Err(error("Done requires Start evidence"));
                };
                if delivered != 0 || !matches!(self.selected_final_url, Available::Unavailable(_)) {
                    return Err(error(
                        "pre-Start failure has delivered body or selected final URL evidence",
                    ));
                }
                if self.acquisitions.is_empty() && *kind != NetworkError::Cancelled {
                    return Err(error(
                        "non-cancellation failure requires acquisition evidence",
                    ));
                }
                let acquired = self
                    .acquisitions
                    .iter()
                    .any(|a| matches!(a.result, AcquisitionResult::Response(_)));
                if !matches!(
                    (acquired, response),
                    (true, Available::Present(_)) | (false, Available::Unavailable(_))
                ) {
                    return Err(error(
                        "pre-Start response availability contradicts acquisition evidence",
                    ));
                }
                match progress {
                    Available::Present(p) => check_progress(p)?,
                    Available::Unavailable(_) => {}
                    Available::Inapplicable => {
                        return Err(error("pre-Start progress availability is unrecorded"));
                    }
                }
            }
            Available::Unavailable(_) => return Err(error("Start evidence is unavailable")),
        }
        Ok(())
    }
}
fn validate_response(r: &ResponseObservation) -> Result<(), HarnessError> {
    if response_storage(r) > bounds::RECORD || r.redirect_chain.len() > 64 || r.headers.len() > 64 {
        return Err(error("response record budget exceeded"));
    }
    Ok(())
}
impl NetworkingObservation {
    pub(crate) fn storage(&self) -> usize {
        size_of::<Self>()
            + self.rows.capacity() * size_of::<FetchObservation>()
            + self
                .rows
                .iter()
                .map(FetchObservation::storage)
                .sum::<usize>()
    }
    pub(crate) fn validate(&self, rows: usize) -> Result<(), HarnessError> {
        if rows == 0 || rows > 32 || self.rows.len() != rows {
            return Err(error("networking observation row correspondence"));
        }
        for row in &self.rows {
            row.validate()?;
        }
        if self.storage() > bounds::FIXTURE_STORAGE {
            return Err(error("fixture observation storage budget exceeded"));
        }
        Ok(())
    }
}

/// Closed networking diagnostic walk. Equality covers the complete typed rows;
/// only differing leaves are rendered. Diagnostic excess is ERROR, never elision.
pub(crate) fn differences(
    left: &NetworkingObservation,
    right: &NetworkingObservation,
) -> Result<Vec<String>, HarnessError> {
    let mut all = Vec::new();
    for (i, (l, r)) in left.rows.iter().zip(&right.rows).enumerate() {
        let p = format!("/rows[{i}]");
        let mut w = Diagnostic {
            lines: vec![],
            bytes: 0,
        };
        w.request_available(&format!("{p}/request"), &l.request, &r.request)?;
        w.field(
            &format!("{p}/acquisitions/length"),
            &l.acquisitions.len(),
            &r.acquisitions.len(),
        )?;
        for (i, (l, r)) in l.acquisitions.iter().zip(&r.acquisitions).enumerate() {
            let p = format!("{p}/acquisitions[{i}]");
            w.request(&format!("{p}/request"), &l.request, &r.request)?;
            match (&l.result, &r.result) {
                (AcquisitionResult::Response(l), AcquisitionResult::Response(r)) => {
                    w.response(&format!("{p}/result/value"), l, r)?
                }
                (AcquisitionResult::NetworkError(l), AcquisitionResult::NetworkError(r)) => {
                    w.field(&format!("{p}/result/value"), l, r)?
                }
                (
                    AcquisitionResult::ResponseConstructionRejected(l),
                    AcquisitionResult::ResponseConstructionRejected(r),
                ) => w.field(&format!("{p}/result/value"), l, r)?,
                _ => w.field(
                    &format!("{p}/result/kind"),
                    &acquisition_kind(&l.result),
                    &acquisition_kind(&r.result),
                )?,
            }
        }
        w.response_available(&format!("{p}/start"), &l.start, &r.start)?;
        w.terminal(&format!("{p}/terminal"), &l.terminal, &r.terminal)?;
        if let (Available::Present(l), Available::Present(r)) =
            (&l.selected_final_url, &r.selected_final_url)
        {
            w.field(&format!("{p}/selected_final_url/value"), l, r)?;
        } else {
            w.availability(
                &format!("{p}/selected_final_url"),
                &l.selected_final_url,
                &r.selected_final_url,
            )?;
        }
        match (&l.body, &r.body) {
            (Body::Bytes { bytes: l }, Body::Bytes { bytes: r }) => {
                w.byte_values(&format!("{p}/body/bytes"), l, r)?
            }
            (
                Body::Summary {
                    length: l,
                    sha256: lh,
                    prefix: lp,
                    suffix: ls,
                },
                Body::Summary {
                    length: r,
                    sha256: rh,
                    prefix: rp,
                    suffix: rs,
                },
            ) => {
                w.field(&format!("{p}/body/length"), l, r)?;
                w.field(&format!("{p}/body/sha256"), lh, rh)?;
                w.byte_values(&format!("{p}/body/prefix"), lp, rp)?;
                w.byte_values(&format!("{p}/body/suffix"), ls, rs)?;
            }
            _ => w.field(
                &format!("{p}/body/kind"),
                &matches!(l.body, Body::Bytes { .. }),
                &matches!(r.body, Body::Bytes { .. }),
            )?,
        }
        all.extend(w.lines);
    }
    Ok(all)
}
fn acquisition_kind(v: &AcquisitionResult) -> &'static str {
    match v {
        AcquisitionResult::Response(_) => "response",
        AcquisitionResult::NetworkError(_) => "network_error",
        AcquisitionResult::MissingRoute => "missing_route",
        AcquisitionResult::ResponseConstructionRejected(_) => "response_construction_rejected",
    }
}
fn terminal_kind(v: &Terminal) -> &'static str {
    match v {
        Terminal::Done { .. } => "done",
        Terminal::Error { .. } => "error",
        Terminal::ConstructionRejected { .. } => "construction_rejected",
        Terminal::DispatchRejected { .. } => "dispatch_rejected",
        Terminal::UrlRejected { .. } => "url_rejected",
    }
}
struct Diagnostic {
    lines: Vec<String>,
    bytes: usize,
}
macro_rules! fields {($w:expr,$p:expr,$l:expr,$r:expr,$($f:ident),+)=>{$($w.field(&format!("{}/{}",$p,stringify!($f)),&$l.$f,&$r.$f)?;)+};}
impl Diagnostic {
    fn field<T: Serialize + PartialEq>(
        &mut self,
        path: &str,
        l: &T,
        r: &T,
    ) -> Result<(), HarnessError> {
        use std::io::Write;
        if l == r {
            return Ok(());
        }
        if self.lines.len() == 32 {
            return Err(error("difference count budget exceeded"));
        }
        let mut out = bounds::Buffer::new(bounds::DIAGNOSTIC.saturating_sub(self.bytes));
        write!(out, "difference: {path} expected ")
            .map_err(|_| error("diagnostic budget exceeded"))?;
        serde_json::to_writer(&mut out, l).map_err(|_| error("diagnostic budget exceeded"))?;
        write!(out, ", actual ").map_err(|_| error("diagnostic budget exceeded"))?;
        serde_json::to_writer(&mut out, r).map_err(|_| error("diagnostic budget exceeded"))?;
        let bytes = out.finish();
        self.bytes += bytes.len();
        self.lines
            .push(String::from_utf8(bytes).map_err(|_| error("invalid diagnostic UTF-8"))?);
        Ok(())
    }
    fn availability<T>(
        &mut self,
        p: &str,
        l: &Available<T>,
        r: &Available<T>,
    ) -> Result<(), HarnessError> {
        let state = |v: &Available<T>| match v {
            Available::Present(_) => "present",
            Available::Unavailable(_) => "unavailable",
            Available::Inapplicable => "inapplicable",
        };
        self.field(&format!("{p}/state"), &state(l), &state(r))?;
        if let (Available::Unavailable(l), Available::Unavailable(r)) = (l, r) {
            self.field(&format!("{p}/value"), l, r)?;
        }
        Ok(())
    }
    fn byte_values(&mut self, p: &str, l: &[u8], r: &[u8]) -> Result<(), HarnessError> {
        self.field(&format!("{p}/length"), &l.len(), &r.len())?;
        for (i, (l, r)) in l.iter().zip(r).enumerate() {
            self.field(&format!("{p}[{i}]"), l, r)?;
        }
        Ok(())
    }
    fn headers(&mut self, p: &str, l: &[Header], r: &[Header]) -> Result<(), HarnessError> {
        self.field(&format!("{p}/length"), &l.len(), &r.len())?;
        for (i, (l, r)) in l.iter().zip(r).enumerate() {
            self.field(&format!("{p}[{i}]/name"), &l.0, &r.0)?;
            self.byte_values(&format!("{p}[{i}]/value"), &l.1, &r.1)?;
        }
        Ok(())
    }
    fn request_available(
        &mut self,
        p: &str,
        l: &Available<RequestObservation>,
        r: &Available<RequestObservation>,
    ) -> Result<(), HarnessError> {
        if let (Available::Present(l), Available::Present(r)) = (l, r) {
            self.request(&format!("{p}/value"), l, r)
        } else {
            self.availability(p, l, r)
        }
    }
    fn response_available(
        &mut self,
        p: &str,
        l: &Available<ResponseObservation>,
        r: &Available<ResponseObservation>,
    ) -> Result<(), HarnessError> {
        if let (Available::Present(l), Available::Present(r)) = (l, r) {
            self.response(&format!("{p}/value"), l, r)
        } else {
            self.availability(p, l, r)
        }
    }
    fn request(
        &mut self,
        p: &str,
        l: &RequestObservation,
        r: &RequestObservation,
    ) -> Result<(), HarnessError> {
        fields!(
            self,
            p,
            l,
            r,
            original_url,
            current_url,
            transport_target,
            purpose,
            route,
            method,
            initiator_url,
            requesting_origin,
            redirect_policy,
            has_body
        );
        self.headers(&format!("{p}/headers"), &l.headers, &r.headers)
    }
    fn response(
        &mut self,
        p: &str,
        l: &ResponseObservation,
        r: &ResponseObservation,
    ) -> Result<(), HarnessError> {
        fields!(
            self,
            p,
            l,
            r,
            original_url,
            purpose,
            requested_url,
            reported_response_url,
            response_url,
            status,
            status_class,
            content_type,
            declared_content_length
        );
        self.headers(&format!("{p}/headers"), &l.headers, &r.headers)?;
        self.field(
            &format!("{p}/redirect_chain/length"),
            &l.redirect_chain.len(),
            &r.redirect_chain.len(),
        )?;
        for (i, (l, r)) in l.redirect_chain.iter().zip(&r.redirect_chain).enumerate() {
            fields!(
                self,
                format!("{p}/redirect_chain[{i}]"),
                l,
                r,
                request_url,
                response_url,
                status,
                target_url
            );
        }
        Ok(())
    }
    fn terminal(&mut self, p: &str, l: &Terminal, r: &Terminal) -> Result<(), HarnessError> {
        self.field(&format!("{p}/kind"), &terminal_kind(l), &terminal_kind(r))?;
        match (l, r) {
            (
                Terminal::Done {
                    response: l,
                    progress: lp,
                },
                Terminal::Done {
                    response: r,
                    progress: rp,
                },
            ) => {
                self.response(&format!("{p}/response"), l, r)?;
                self.field(&format!("{p}/progress"), lp, rp)?;
            }
            (
                Terminal::Error {
                    error: l,
                    response: lr,
                    progress: lp,
                },
                Terminal::Error {
                    error: r,
                    response: rr,
                    progress: rp,
                },
            ) => {
                self.field(&format!("{p}/error"), l, r)?;
                self.response_available(&format!("{p}/response"), lr, rr)?;
                self.field(&format!("{p}/progress"), lp, rp)?;
            }
            (
                Terminal::ConstructionRejected { error: l },
                Terminal::ConstructionRejected { error: r },
            )
            | (Terminal::DispatchRejected { error: l }, Terminal::DispatchRejected { error: r }) => {
                self.field(&format!("{p}/error"), l, r)?
            }
            (
                Terminal::UrlRejected {
                    failure: l,
                    diagnostics: ld,
                },
                Terminal::UrlRejected {
                    failure: r,
                    diagnostics: rd,
                },
            ) => {
                self.field(&format!("{p}/failure"), l, r)?;
                self.field(&format!("{p}/diagnostics"), ld, rd)?;
            }
            _ => {}
        }
        Ok(())
    }
}

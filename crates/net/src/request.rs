//! Fetch-owned request admission, independent of I/O and Browser private state.
use addressing::{BrowserUrl, Origin, ResourceAddress, ResourceIdentityKey, ResourceKind};
use core_types::{RequestId, ResourceKind as ExecutionRoute};
use std::fmt;

use crate::{HeaderError, HttpHeaders};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestMethod {
    Get,
}

impl TryFrom<&str> for RequestMethod {
    type Error = RequestAdmissionError;
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        match token {
            "GET" => Ok(Self::Get),
            _ => Err(RequestAdmissionError::UnsupportedMethod),
        }
    }
}

/// Follow intent only. The AI2 adapter still uses legacy backend redirects;
/// AI5/AI6 own controlled attempts and per-hop admission. No policy override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectPolicy {
    Follow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestAdmissionError {
    UnsupportedMethod,
    UnsupportedScheme,
    UrlCredentialsUnsupported,
    UnsupportedHeader,
    UnsupportedHeaderValue,
    RepeatedHeaderUnsupported,
    HeadersNotApplicable,
    Headers(HeaderError),
    IncompatibleExecutionRoute,
}

impl fmt::Display for RequestAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fetch request rejected: {self:?}")
    }
}
impl std::error::Error for RequestAdmissionError {}

/// Immutable admitted request metadata. Producer purpose is not an execution
/// route. Browser command admission additionally validates route and attachment.
/// RequestId retains its existing navigation-generation meaning, not a globally
/// unique subresource identity. Missing source context is never inferred.
#[derive(Clone)]
pub struct FetchRequest {
    address: ResourceAddress,
    current_url: BrowserUrl,
    request_id: RequestId,
    method: RequestMethod,
    headers: HttpHeaders,
    initiator_url: Option<BrowserUrl>,
    requesting_origin: Option<Origin>,
    redirect_policy: RedirectPolicy,
}

impl FetchRequest {
    pub fn try_new(
        request_id: RequestId,
        address: ResourceAddress,
        method: RequestMethod,
        mut headers: HttpHeaders,
        initiator_url: Option<BrowserUrl>,
        requesting_origin: Option<Origin>,
    ) -> Result<Self, RequestAdmissionError> {
        match address.url().scheme() {
            "http" | "https" => {
                if !address.url().username().is_empty() || address.url().password().is_some() {
                    return Err(RequestAdmissionError::UrlCredentialsUnsupported);
                }
                let mut accept = false;
                let mut language = false;
                for (name, value) in headers.iter() {
                    let seen = match name {
                        "accept" => &mut accept,
                        "accept-language" => &mut language,
                        _ => return Err(RequestAdmissionError::UnsupportedHeader),
                    };
                    if *seen {
                        return Err(RequestAdmissionError::RepeatedHeaderUnsupported);
                    }
                    *seen = true;
                    // ureq's request API uses strings and trims boundary OWS.
                    // Admit only values it can faithfully carry; preserve the
                    // broader byte representation for AI3, without conversion.
                    if !value.is_ascii()
                        || value.first().is_some_and(|b| matches!(b, b' ' | b'\t'))
                        || value.last().is_some_and(|b| matches!(b, b' ' | b'\t'))
                    {
                        return Err(RequestAdmissionError::UnsupportedHeaderValue);
                    }
                }
                if !accept {
                    headers
                        .append("accept", b"*/*")
                        .map_err(RequestAdmissionError::Headers)?;
                }
            }
            "file" => {
                if !headers.is_empty() {
                    return Err(RequestAdmissionError::HeadersNotApplicable);
                }
            }
            _ => return Err(RequestAdmissionError::UnsupportedScheme),
        }
        Ok(Self {
            current_url: address.url().clone(),
            address,
            request_id,
            method,
            headers,
            initiator_url,
            requesting_origin,
            redirect_policy: RedirectPolicy::Follow,
        })
    }

    pub fn address(&self) -> &ResourceAddress {
        &self.address
    }
    pub fn original_url(&self) -> &BrowserUrl {
        self.address.url()
    }
    /// Initial attempt only until AI5/AI6 expose backend hops.
    pub fn current_url(&self) -> &BrowserUrl {
        &self.current_url
    }
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }
    pub fn method(&self) -> RequestMethod {
        self.method
    }
    pub fn headers(&self) -> &HttpHeaders {
        &self.headers
    }
    pub fn initiator_url(&self) -> Option<&BrowserUrl> {
        self.initiator_url.as_ref()
    }
    pub fn requesting_origin(&self) -> Option<&Origin> {
        self.requesting_origin.as_ref()
    }
    pub fn redirect_policy(&self) -> RedirectPolicy {
        self.redirect_policy
    }
    /// GET has no body; there is no body input or mutable body state in AI2.
    pub fn has_body(&self) -> bool {
        false
    }

    pub fn transport_target(&self) -> ResourceIdentityKey {
        ResourceAddress::new(self.current_url.clone(), self.address.kind()).retrieval_key()
    }

    /// Existing execution/limit routes, not a general Fetch destination model.
    pub fn validate_route(&self, route: ExecutionRoute) -> Result<(), RequestAdmissionError> {
        match (self.address.kind(), route) {
            (ResourceKind::Document | ResourceKind::Hyperlink, ExecutionRoute::Html)
            | (ResourceKind::Stylesheet, ExecutionRoute::Css)
            | (ResourceKind::Image, ExecutionRoute::Image) => Ok(()),
            _ => Err(RequestAdmissionError::IncompatibleExecutionRoute),
        }
    }

    /// Temporary event-correlation projection, not transport targeting. AI3/AI5
    /// evolve the string response protocol. Keep CSS's exact fragmentless issued
    /// provenance and complete document/image lookup strings unchanged.
    pub fn legacy_issued_url(
        &self,
        route: ExecutionRoute,
    ) -> Result<String, RequestAdmissionError> {
        self.validate_route(route)?;
        Ok(match route {
            ExecutionRoute::Css => self.address.retrieval_key().as_str().to_owned(),
            ExecutionRoute::Html | ExecutionRoute::Image => self.original_url().as_str().to_owned(),
        })
    }
}

impl fmt::Debug for FetchRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FetchRequest")
            .field("request_id", &self.request_id)
            .field("address", &"<redacted>")
            .field("purpose", &self.address.kind())
            .field("current_url", &"<redacted>")
            .field("method", &self.method)
            .field("headers", &self.headers)
            .field(
                "initiator_url",
                &self.initiator_url.as_ref().map(|_| "<redacted>"),
            )
            .field("requesting_origin", &self.requesting_origin)
            .field("redirect_policy", &self.redirect_policy)
            .field("body", &"absent")
            .finish()
    }
}

use std::{fmt, sync::Arc};

use addressing::{BrowserUrl, UrlFailure, UrlParseOutcome};

use crate::{ContentType, HeaderError, HttpHeaders, InvalidContentType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseAdmissionError {
    InvalidStatus,
    InformationalResponseUnsupported,
    IncompatibleOriginalScheme,
    MissingResponseUrl,
    InvalidResponseUrl,
    UnsupportedResponseScheme,
    HeaderValuesUnavailable,
    Headers(HeaderError),
}

impl fmt::Display for ResponseAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Response admission failed: {self:?}")
    }
}
impl std::error::Error for ResponseAdmissionError {}

/// Valid HTTP status, including unknown codes within a recognized class.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HttpStatus(u16);

impl TryFrom<u16> for HttpStatus {
    type Error = ResponseAdmissionError;
    fn try_from(code: u16) -> Result<Self, Self::Error> {
        if !(100..=599).contains(&code) {
            return Err(ResponseAdmissionError::InvalidStatus);
        }
        Ok(Self(code))
    }
}
impl HttpStatus {
    pub const fn code(self) -> u16 {
        self.0
    }
    pub const fn is_informational(self) -> bool {
        self.0 < 200
    }
    pub const fn is_success(self) -> bool {
        self.0 >= 200 && self.0 < 300
    }
    pub const fn is_http_error(self) -> bool {
        self.0 >= 400
    }
    pub const fn is_redirection(self) -> bool {
        self.0 >= 300 && self.0 < 400
    }
    pub const fn is_redirect(self) -> bool {
        matches!(self.0, 301 | 302 | 303 | 307 | 308)
    }
    pub const fn has_null_body(self) -> bool {
        self.is_informational() || matches!(self.0, 204 | 205 | 304)
    }
}
impl fmt::Debug for HttpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HttpStatus({})", self.0)
    }
}

/// Immutable acquired response facts. Construction does not imply body completion,
/// backend admission, or suitability for a resource consumer.
#[derive(Clone, PartialEq, Eq)]
pub struct NetworkResponseInfo {
    facts: Arc<ResponseFacts>,
}

// Start, completion, failure context and retained consumers share immutable
// facts. This also keeps the existing bus/platform event envelopes compact.
#[derive(PartialEq, Eq)]
struct ResponseFacts {
    original_url: BrowserUrl,
    requested_url: String,
    reported_response_url: Option<String>,
    response_url: Option<Result<UrlParseOutcome, UrlFailure>>,
    status: Option<HttpStatus>,
    headers: HttpHeaders,
    local_content_type: Option<String>,
}

impl NetworkResponseInfo {
    pub fn http(
        original_url: BrowserUrl,
        requested_url: String,
        reported_response_url: Option<String>,
        status: HttpStatus,
        headers: HttpHeaders,
    ) -> Result<Self, ResponseAdmissionError> {
        if !matches!(original_url.scheme(), "http" | "https") {
            return Err(ResponseAdmissionError::IncompatibleOriginalScheme);
        }
        if status.is_informational() {
            return Err(ResponseAdmissionError::InformationalResponseUnsupported);
        }
        Ok(Self::new(
            original_url,
            requested_url,
            reported_response_url,
            Some(status),
            headers,
            None,
        ))
    }

    pub fn local_file(
        original_url: BrowserUrl,
        requested_url: String,
        reported_response_url: Option<String>,
        content_type: Option<String>,
    ) -> Result<Self, ResponseAdmissionError> {
        if original_url.scheme() != "file" {
            return Err(ResponseAdmissionError::IncompatibleOriginalScheme);
        }
        // Local MIME is a loader fact, never a fabricated HTTP header.
        if let Some(value) = &content_type {
            let mut bounded = HttpHeaders::new();
            bounded
                .append("content-type", value.as_bytes())
                .map_err(ResponseAdmissionError::Headers)?;
        }
        Ok(Self::new(
            original_url,
            requested_url,
            reported_response_url,
            None,
            HttpHeaders::new(),
            content_type,
        ))
    }

    fn new(
        original_url: BrowserUrl,
        requested_url: String,
        reported_response_url: Option<String>,
        status: Option<HttpStatus>,
        headers: HttpHeaders,
        local_content_type: Option<String>,
    ) -> Self {
        let response_url = reported_response_url.as_deref().map(BrowserUrl::parse);
        Self {
            facts: Arc::new(ResponseFacts {
                original_url,
                requested_url,
                reported_response_url,
                response_url,
                status,
                headers,
                local_content_type,
            }),
        }
    }

    pub fn original_url(&self) -> &BrowserUrl {
        &self.facts.original_url
    }
    pub fn requested_url(&self) -> &str {
        &self.facts.requested_url
    }
    pub fn reported_response_url(&self) -> Option<&str> {
        self.facts.reported_response_url.as_deref()
    }
    pub fn response_url(&self) -> Option<&Result<UrlParseOutcome, UrlFailure>> {
        self.facts.response_url.as_ref()
    }
    pub fn status(&self) -> Option<HttpStatus> {
        self.facts.status
    }
    pub fn status_code(&self) -> Option<u16> {
        self.facts.status.map(HttpStatus::code)
    }
    /// Admitted producer fields, not a claim of a byte-preserving wire capture.
    pub fn headers(&self) -> &HttpHeaders {
        &self.facts.headers
    }
    pub fn declared_content_types(
        &self,
    ) -> impl Iterator<Item = Result<ContentType, InvalidContentType>> + '_ {
        self.facts
            .headers
            .values("content-type")
            .map(ContentType::parse)
    }
    /// Existing first-field compatibility selection. AI8 owns aggregate extraction.
    pub fn legacy_content_type(&self) -> Option<&str> {
        if self.facts.status.is_none() {
            return self.facts.local_content_type.as_deref();
        }
        self.facts
            .headers
            .values("content-type")
            .next()
            .and_then(|v| std::str::from_utf8(v).ok())
    }
    pub(crate) fn validate_backend_http(&self) -> Result<(), ResponseAdmissionError> {
        match self.response_url() {
            None => Err(ResponseAdmissionError::MissingResponseUrl),
            Some(Err(_)) => Err(ResponseAdmissionError::InvalidResponseUrl),
            Some(Ok(parsed)) if !matches!(parsed.url.scheme(), "http" | "https") => {
                Err(ResponseAdmissionError::UnsupportedResponseScheme)
            }
            Some(Ok(_)) => Ok(()),
        }
    }
}

impl fmt::Debug for NetworkResponseInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let provenance = match self.response_url() {
            None => "missing",
            Some(Err(_)) => "invalid",
            Some(Ok(_)) => "parsed",
        };
        f.debug_struct("NetworkResponseInfo")
            .field("original_url", &"<redacted>")
            .field("requested_url", &"<redacted>")
            .field("response_url", &provenance)
            .field("status", &self.facts.status)
            .field("headers", &self.facts.headers)
            .field(
                "local_content_type_present",
                &self.facts.local_content_type.is_some(),
            )
            .finish()
    }
}

//! Fetch-owned redirect decisions. Addressing remains the only URL parser.
use addressing::BrowserUrl;
use std::fmt;

use crate::{FetchRequest, HttpStatus, NetworkResponseInfo};

pub(crate) const MAX_REDIRECTS: usize = 20;

/// Concrete failures of the supported GET redirect operation, not HTTP statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectError {
    InvalidLocation,
    RepeatedLocation,
    UnsupportedScheme,
    UrlCredentialsUnsupported,
    LimitExceeded,
}

impl fmt::Display for RedirectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fetch redirect failed: {self:?}")
    }
}
impl std::error::Error for RedirectError {}

/// A transition whose target backend acquisition was entered. This is not proof
/// of DNS success, transmission, or server receipt. No body/field contents live
/// here. Construction and the twenty-entry budget belong to Fetch.
#[derive(Clone, PartialEq, Eq)]
pub struct RedirectHop {
    request_url: BrowserUrl,
    response_url: BrowserUrl,
    status: HttpStatus,
    target_url: BrowserUrl,
}

impl RedirectHop {
    pub fn request_url(&self) -> &BrowserUrl {
        &self.request_url
    }
    pub fn response_url(&self) -> &BrowserUrl {
        &self.response_url
    }
    pub fn status(&self) -> HttpStatus {
        self.status
    }
    pub fn target_url(&self) -> &BrowserUrl {
        &self.target_url
    }
}

impl fmt::Debug for RedirectHop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedirectHop")
            .field("request_url", &"<redacted>")
            .field("response_url", &"<redacted>")
            .field("status", &self.status)
            .field("target_url", &"<redacted>")
            .finish()
    }
}

/// Only admitted HttpHeaders values enter this helper (at most 8 KiB). Escape
/// original non-ASCII bytes, without Unicode decoding or touching ASCII syntax
/// and existing escapes. Expansion is at most 3x. Fetch issues #883/#814 leave
/// this boundary unsettled; this explicit adaptation follows the tentative WPT
/// evidence, not a claim of an unambiguous normative decoding rule.
fn location_reference(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut reference = String::with_capacity(value.len() * 3);
    for &byte in value {
        if byte.is_ascii() {
            reference.push(char::from(byte));
        } else {
            reference.push('%');
            reference.push(char::from(HEX[usize::from(byte >> 4)]));
            reference.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    reference
}

/// A returned hop is still pending: only entry into the next acquisition commits
/// it. Missing Location and non-followed statuses select the current response.
pub(crate) fn next_target(
    request: &FetchRequest,
    response: &NetworkResponseInfo,
    followed: usize,
) -> Result<Option<(BrowserUrl, RedirectHop)>, RedirectError> {
    let Some(status) = response.status().filter(|status| status.is_redirect()) else {
        return Ok(None);
    };
    let mut locations = response.headers().values("location");
    let Some(value) = locations.next() else {
        return Ok(None);
    };
    if locations.next().is_some() {
        return Err(RedirectError::RepeatedLocation);
    }
    // The caller has admitted this response. Never substitute the original URL
    // for absent/invalid acquired provenance.
    let base = &response
        .response_url()
        .and_then(|url| url.as_ref().ok())
        .ok_or(RedirectError::InvalidLocation)?
        .url;
    let mut target = base
        .resolve(&location_reference(value))
        .map_err(|_| RedirectError::InvalidLocation)?
        .url;
    if target.fragment().is_none()
        && let Some(fragment) = request.current_url().fragment()
    {
        target = target
            .resolve(&format!("#{fragment}"))
            .map_err(|_| RedirectError::InvalidLocation)?
            .url;
    }
    if !matches!(target.scheme(), "http" | "https") {
        return Err(RedirectError::UnsupportedScheme);
    }
    if !target.username().is_empty() || target.password().is_some() {
        return Err(RedirectError::UrlCredentialsUnsupported);
    }
    if followed >= MAX_REDIRECTS {
        return Err(RedirectError::LimitExceeded);
    }
    let hop = RedirectHop {
        request_url: request.current_url().clone(),
        response_url: base.clone(),
        status,
        target_url: target.clone(),
    };
    Ok(Some((target, hop)))
}

#[cfg(test)]
mod tests {
    use super::location_reference;

    #[test]
    fn ai6_location_adaptation_preserves_original_bytes_and_ascii_spelling() {
        for (raw, expected) in [
            (b"../next?\xff".as_slice(), "../next?%FF"),
            (b"../next?\xe2\x98\x83", "../next?%E2%98%83"),
            (
                b"../next?%ff%E2%98%83%e2%98%83",
                "../next?%ff%E2%98%83%e2%98%83",
            ),
            (b"../next?\xff%ff", "../next?%FF%ff"),
            (b"../next?%\xff", "../next?%%FF"),
            (b"../next?%\xe2\x98\x83", "../next?%%E2%98%83"),
            (b"../p,a?x=%2f&y=a,b#", "../p,a?x=%2f&y=a,b#"),
            (b"", ""),
        ] {
            assert_eq!(location_reference(raw), expected);
        }
        let raw = vec![0xff; 8192];
        assert_eq!(location_reference(&raw), "%FF".repeat(8192));
    }
}

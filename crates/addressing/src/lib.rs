//! Immutable URL values and explicit-base resolution owned by Borrowser.
//!
//! Parsing wraps the revision-pinned maintained `url` fork. It does not grant permission to
//! fetch a scheme, compute an origin, or select a document base. See the AH2
//! contract and AH-URL-001 in the feature-gap tracker for dependency limitations.

mod diagnostics;

pub use diagnostics::{UrlDiagnostic, UrlFailure, UrlFailureKind, UrlParseError};

use std::cell::RefCell;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

/// A normalized, immutable URL. No backend types or mutation are exposed.
///
/// Equality and hashing use the complete normalized serialization, including
/// credentials, query spelling/order, and fragment. They are neither same-origin
/// comparison nor fetch/cache identity. Diagnostics do not belong to this value.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BrowserUrl {
    inner: url::Url,
}

/// A borrowed host component. Empty hosts are represented by `None` together
/// with [`BrowserUrl::has_authority`]; they are not a domain name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlHost<'a> {
    /// An ASCII domain for a special scheme (including IDNA normalization).
    Domain(&'a str),
    /// A non-special host, preserving its case and percent-encoded spelling.
    Opaque(&'a str),
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
}

/// Successful parsing, possibly with recoverable diagnostics. Events retain
/// callback order and repetitions. No events does not prove input conformance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlParseOutcome {
    pub url: BrowserUrl,
    pub diagnostics: Vec<UrlDiagnostic>,
}

impl BrowserUrl {
    /// Parse without a base, address-bar guessing, or caller-specific trimming.
    pub fn parse(input: &str) -> Result<UrlParseOutcome, UrlFailure> {
        Self::parse_with_base(input, None)
    }

    /// Resolve a reference against this explicit base. Absolute references are
    /// also accepted. The base is never mutated, and its earlier parse diagnostics
    /// are not replayed. Opaque bases can resolve fragments but not relative paths.
    pub fn resolve(&self, reference: &str) -> Result<UrlParseOutcome, UrlFailure> {
        Self::parse_with_base(reference, Some(self))
    }

    fn parse_with_base(input: &str, base: Option<&Self>) -> Result<UrlParseOutcome, UrlFailure> {
        // The backend callback is Fn rather than FnMut. This cell is local to
        // one synchronous parse; there is no shared/global diagnostic state.
        let diagnostics = RefCell::new(Vec::new());
        let callback = |event| {
            diagnostics
                .borrow_mut()
                .push(UrlDiagnostic::from_backend(event))
        };
        let result = url::Url::options()
            .base_url(base.map(|base| &base.inner))
            .syntax_violation_callback(Some(&callback))
            .parse(input);
        let diagnostics = diagnostics.into_inner();
        match result {
            Ok(inner) => Ok(UrlParseOutcome {
                url: Self { inner },
                diagnostics,
            }),
            Err(error) => Err(UrlFailure {
                kind: UrlFailureKind::from_backend(error),
                diagnostics,
                base: base.cloned(),
            }),
        }
    }

    /// Complete normalized serialization, including credentials. Use `Debug`
    /// for credential-redacted diagnostics; this is not a presentation label.
    /// Repeated serialization of this immutable value is always stable.
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Lowercase scheme without the colon. Parsing does not imply loader support.
    pub fn scheme(&self) -> &str {
        self.inner.scheme()
    }

    /// Normalized, percent-encoded username. An empty string cannot distinguish
    /// absent userinfo from an authored empty username.
    pub fn username(&self) -> &str {
        self.inner.username()
    }

    /// Normalized, percent-encoded password. Authored empty passwords normalize
    /// to `None`; this is not a test for whether a colon appeared in the input.
    pub fn password(&self) -> Option<&str> {
        self.inner.password()
    }

    pub fn host(&self) -> Option<UrlHost<'_>> {
        self.inner.host().map(|host| match host {
            url::Host::Domain(domain) => {
                // WHATWG's special-scheme set determines domain vs opaque host
                // semantics. This classification grants no transport capability.
                match self.scheme() {
                    "ftp" | "file" | "http" | "https" | "ws" | "wss" => UrlHost::Domain(domain),
                    _ => UrlHost::Opaque(domain),
                }
            }
            url::Host::Ipv4(address) => UrlHost::Ipv4(address),
            url::Host::Ipv6(address) => UrlHost::Ipv6(address),
        })
    }

    /// Nondefault port, if present. A scheme's default port normalizes to `None`.
    pub fn port(&self) -> Option<u16> {
        self.inner.port()
    }

    /// Serialized path, preserving percent encoding rather than decoding it.
    pub fn path(&self) -> &str {
        self.inner.path()
    }

    /// Query without `?`; `None` and `Some("")` remain distinct.
    pub fn query(&self) -> Option<&str> {
        self.inner.query()
    }

    /// Fragment without `#`; `None` and `Some("")` remain distinct.
    pub fn fragment(&self) -> Option<&str> {
        self.inner.fragment()
    }

    pub fn has_authority(&self) -> bool {
        self.inner.has_authority()
    }

    pub fn has_opaque_path(&self) -> bool {
        self.inner.cannot_be_a_base()
    }
}

impl fmt::Debug for BrowserUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never delegate to the backend Debug or include its serialization.
        f.debug_struct("BrowserUrl")
            .field("scheme", &self.scheme())
            .field(
                "username",
                &if self.username().is_empty() {
                    ""
                } else {
                    "<redacted>"
                },
            )
            .field("password", &self.password().map(|_| "<redacted>"))
            .field("host", &self.host())
            .field("port", &self.port())
            .field("path", &self.path())
            .field("query", &self.query())
            .field("fragment", &self.fragment())
            .field("authority", &self.has_authority())
            .field("opaque_path", &self.has_opaque_path())
            .finish()
    }
}

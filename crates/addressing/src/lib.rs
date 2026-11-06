//! Immutable URL values, explicit-base resolution, and origins owned by Borrowser.
//!
//! Parsing wraps the revision-pinned maintained `url` fork. It does not grant permission to
//! fetch a scheme or select a document base. URL-origin determination is a
//! separate operation, not a replacement for document creation rules. See the AH2
//! contract and AH-URL-001 in the feature-gap tracker for dependency limitations.

mod diagnostics;
mod origin;
mod resource;

pub use origin::Origin;
pub use resource::{
    ResourceAddress, ResourceCapability, ResourceIdentityKey, ResourceKind, ResourceOperation,
};

pub use diagnostics::{
    UrlDiagnostic, UrlFailure, UrlFailureKind, UrlParseError, format_resolution_debug,
};

use std::cell::RefCell;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

/// A URL has no supported native file-path interpretation. No URL or path
/// contents escape through this fixed error category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePathConversionError;

impl fmt::Display for FilePathConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("URL cannot be converted to a native file path")
    }
}

impl std::error::Error for FilePathConversionError {}

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
/// Its derived Debug exposes host contents; use the URL Debug for redacted output.
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

    /// Parse the original reference only if the parser proves that its result,
    /// failure and diagnostics are independent of an unavailable base.
    /// `Ok(None)` is explicitly unresolved, never a hypothetical null-base
    /// verdict. The consumer retains missing/invalid source provenance.
    pub fn parse_if_base_independent(input: &str) -> Result<Option<UrlParseOutcome>, UrlFailure> {
        let diagnostics = RefCell::new(Vec::new());
        let callback = |event| {
            diagnostics
                .borrow_mut()
                .push(UrlDiagnostic::from_backend(event))
        };
        let result = url::Url::options()
            .syntax_violation_callback(Some(&callback))
            .parse_if_base_independent(input);
        let diagnostics = diagnostics.into_inner();
        match result {
            Ok(Some(inner)) => Ok(Some(UrlParseOutcome {
                url: Self { inner },
                diagnostics,
            })),
            Ok(None) => Ok(None),
            Err(error) => Err(UrlFailure {
                kind: UrlFailureKind::from_backend(error),
                diagnostics,
                base: None,
            }),
        }
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
    /// for content-redacted structural diagnostics; this is not a presentation label.
    /// Repeated serialization of this immutable value is always stable.
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Interpret a file URL using the pinned URL implementation's native path
    /// conversion. Query and fragment remain URL metadata, not filename bytes.
    /// Nonlocal authorities are unsupported on Unix; Windows uses drive/UNC
    /// rules. This performs no I/O and grants no filesystem access permission.
    pub fn to_file_path(&self) -> Result<PathBuf, FilePathConversionError> {
        if self.scheme() != "file" {
            return Err(FilePathConversionError);
        }
        #[cfg(any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        ))]
        {
            let path = self
                .inner
                .to_file_path()
                .map_err(|()| FilePathConversionError)?;
            // The pinned implementation does not reject decoded NUL despite
            // its documented contract. Never hand an invalid native path to I/O.
            if path.as_os_str().as_encoded_bytes().contains(&0) {
                return Err(FilePathConversionError);
            }
            Ok(path)
        }
        #[cfg(not(any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        )))]
        Err(FilePathConversionError)
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
        // Only fixed labels and component presence leave this formatter. Never
        // delegate to backend Debug, serialize the URL, or echo arbitrary text.
        let scheme = match self.scheme() {
            "http" => "http",
            "https" => "https",
            "ftp" => "ftp",
            "file" => "file",
            "ws" => "ws",
            "wss" => "wss",
            "about" => "about",
            "data" => "data",
            "blob" => "blob",
            "javascript" => "javascript",
            _ => "<redacted>",
        };
        let host_kind = self.host().map(|host| match host {
            UrlHost::Domain(_) => "domain",
            UrlHost::Opaque(_) => "opaque",
            UrlHost::Ipv4(_) => "ipv4",
            UrlHost::Ipv6(_) => "ipv6",
        });
        // Preserve absent/empty/nonempty distinctions without content or lengths.
        let redact = |value: &str| if value.is_empty() { "" } else { "<redacted>" };
        f.debug_struct("BrowserUrl")
            .field("scheme", &scheme)
            .field("username", &redact(self.username()))
            .field("password", &self.password().map(redact))
            .field("host_kind", &host_kind)
            .field("port", &self.port().map(|_| "<redacted>"))
            .field("path", &redact(self.path()))
            .field("query", &self.query().map(redact))
            .field("fragment", &self.fragment().map(redact))
            .field("authority", &self.has_authority())
            .field("opaque_path", &self.has_opaque_path())
            .finish()
    }
}

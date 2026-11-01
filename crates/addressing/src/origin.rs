use crate::BrowserUrl;
use std::fmt;
use std::sync::Arc;

/// An immutable tuple or opaque origin. Equality means same-origin, not URL
/// equality or permission to access a resource. The HTML domain field is always
/// null; document.domain and same-origin-domain comparison are not supported.
///
/// Cloning preserves opaque identity. Semantic serialization cannot recreate an
/// opaque origin. Debug is content-redacted and never exposes identity.
#[derive(Clone)]
pub struct Origin {
    kind: OriginKind,
}

#[derive(Clone)]
enum OriginKind {
    Tuple {
        scheme: String,
        host: url::Host<String>,
        port: Option<u16>,
    },
    Opaque(Arc<()>),
}

impl BrowserUrl {
    /// Determine this URL's origin, without document creation context.
    ///
    /// BrowserUrl has no associated blob URL entry. For blob URLs this implements
    /// the URL Standard's no-entry embedded-URL fallback only. An existing entry
    /// would require its environment's retained origin, which is unsupported.
    ///
    /// Every opaque result is fresh, including for file URLs under Borrowser's
    /// file-origin policy. Retain the returned value rather than calling again to
    /// access a document's origin. About-document inheritance and sandboxing are
    /// separate creation rules; this operation does not implement them.
    pub fn url_origin(&self) -> Origin {
        match self.scheme() {
            "ftp" | "http" | "https" | "ws" | "wss" => Origin {
                kind: OriginKind::Tuple {
                    scheme: self.scheme().to_owned(),
                    // The parser requires a host for these special schemes.
                    host: self.inner.host().expect("tuple URL has a host").to_owned(),
                    port: self.port(),
                },
            },
            "blob" => {
                // Use the existing parser, not percent decoding, prefix slicing,
                // or the backend's broader recursive origin accessor.
                if let Ok(parsed) = Self::parse(self.path())
                    && matches!(parsed.url.scheme(), "http" | "https" | "file")
                {
                    parsed.url.url_origin()
                } else {
                    Origin::new_opaque()
                }
            }
            // File origins are deliberately opaque. No filesystem permission
            // or local-resource access policy follows from this choice.
            _ => Origin::new_opaque(),
        }
    }
}

impl Origin {
    fn new_opaque() -> Self {
        Self {
            kind: OriginKind::Opaque(Arc::new(())),
        }
    }

    pub fn is_opaque(&self) -> bool {
        matches!(self.kind, OriginKind::Opaque(_))
    }

    /// Compare normalized tuple components or shared opaque identity.
    pub fn is_same_origin(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (
                OriginKind::Tuple { scheme, host, port },
                OriginKind::Tuple {
                    scheme: other_scheme,
                    host: other_host,
                    port: other_port,
                },
            ) => scheme == other_scheme && host == other_host && port == other_port,
            (OriginKind::Opaque(identity), OriginKind::Opaque(other)) => {
                Arc::ptr_eq(identity, other)
            }
            _ => false,
        }
    }

    /// HTML semantic origin serialization. This exposes the tuple's host and
    /// nondefault port; use Debug for production diagnostics. Opaque origins all
    /// serialize as `null`, even when their identities differ.
    pub fn serialize(&self) -> String {
        match &self.kind {
            OriginKind::Opaque(_) => "null".to_owned(),
            OriginKind::Tuple { scheme, host, port } => {
                // The existing host serializer includes canonical IPv6 brackets.
                // Components are already normalized; do not parse or normalize
                // again, or maintain a second default-port table.
                let mut output = format!("{scheme}://{host}");
                if let Some(port) = port {
                    use std::fmt::Write;
                    write!(output, ":{port}").expect("writing to String cannot fail");
                }
                output
            }
        }
    }
}

impl PartialEq for Origin {
    fn eq(&self, other: &Self) -> bool {
        self.is_same_origin(other)
    }
}

impl Eq for Origin {}

impl fmt::Debug for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = f.debug_struct("Origin");
        match &self.kind {
            OriginKind::Opaque(_) => output.field("kind", &"opaque"),
            OriginKind::Tuple { scheme, host, port } => {
                let host_kind = match host {
                    url::Host::Domain(_) => "domain",
                    url::Host::Ipv4(_) => "ipv4",
                    url::Host::Ipv6(_) => "ipv6",
                };
                // Tuple schemes come exclusively from the five fixed URL rules.
                output
                    .field("kind", &"tuple")
                    .field("scheme", scheme)
                    .field("host_kind", &host_kind)
                    .field("port", &port.map(|_| "<redacted>"))
            }
        };
        output.finish()
    }
}

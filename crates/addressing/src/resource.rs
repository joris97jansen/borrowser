use crate::BrowserUrl;
use std::fmt;

/// Producer-known reference purpose. This is neither a transport route nor a
/// Fetch destination; a hyperlink/form action is not a separate destination,
/// and Media does not distinguish audio from video. Unknown never guesses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Document,
    Stylesheet,
    Image,
    /// Authored classic/module JavaScript reference; does not enable execution.
    Script,
    Font,
    Media,
    FormAction,
    Hyperlink,
    Unknown,
}

/// Addressing operation, not Fetch mode or a request to perform I/O.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOperation {
    Retrieval,
    Navigation,
}

/// AH addressing support, independent of parsing and security authorization.
/// Only Supported passes current consumers' operation checks. Reserved and
/// Deferred do not imply an available handler, transport, or creation context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCapability {
    Supported,
    Reserved,
    Deferred,
    Unsupported,
}

/// A complete normalized reference and its producer-known purpose.
///
/// Construction performs no parsing, origin creation, capability admission, or
/// I/O. Unsupported references remain representable. The requesting origin,
/// base selection, initiator/attachment and request lifetime belong to the
/// consumer, not this reusable value. Choose URL equality or retrieval identity
/// explicitly: this descriptor intentionally implements neither Eq nor Hash.
#[derive(Clone, Debug)]
pub struct ResourceAddress {
    url: BrowserUrl,
    kind: ResourceKind,
}

/// Normalized URL serialization with the fragment excluded, for address
/// equality and hashing. Credentials, query spelling/order/repetitions and an
/// empty query remain significant. Kind and requesting origin do not participate.
///
/// This is not request, response, decoded-resource, attachment, navigation, or
/// complete cache identity. Hash outputs are not persistent serialized IDs.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ResourceIdentityKey {
    serialized_url: String,
}

impl ResourceAddress {
    pub fn new(url: BrowserUrl, kind: ResourceKind) -> Self {
        Self { url, kind }
    }

    /// Complete reference, retaining resource-specific fragment information.
    pub fn url(&self) -> &BrowserUrl {
        &self.url
    }

    pub fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Classify AH's operation boundary. Supported does not promise permission,
    /// successful transport, decoder support, or document-creation admission.
    pub fn capability(&self, operation: ResourceOperation) -> ResourceCapability {
        use ResourceCapability::*;
        match (self.url.scheme(), operation) {
            ("http" | "https" | "file", _) => Supported,
            ("about", ResourceOperation::Navigation) => Reserved,
            ("data" | "blob", _) => Deferred,
            // Reserved for WebSocket integration, not either operation here.
            ("ws" | "wss", _) => Reserved,
            _ => Unsupported,
        }
    }

    /// Derive address identity without mutating or reparsing the complete URL.
    /// Also available for unsupported addresses; equality grants no operation.
    pub fn retrieval_key(&self) -> ResourceIdentityKey {
        ResourceIdentityKey {
            // Backend component boundaries implement URL serialization excluding
            // the fragment, including its delimiter. Setters could additionally
            // alter opaque paths; splitting/reparsing would invent URL semantics.
            serialized_url: self.url.inner[..url::Position::AfterQuery].to_owned(),
        }
    }
}

impl ResourceIdentityKey {
    /// Semantic serialization, including credentials and private URL contents.
    /// Use Debug for diagnostics. This is not a logging/presentation interface.
    pub fn as_str(&self) -> &str {
        &self.serialized_url
    }
}

impl fmt::Debug for ResourceIdentityKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceIdentityKey")
            .field("url", &"<redacted>")
            .finish()
    }
}

use core_types::ResourceKind;
use tools::common::{MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, MAX_STYLESHEET_BYTES};

pub(crate) fn resource_byte_limit(kind: ResourceKind) -> usize {
    match kind {
        ResourceKind::Html => MAX_DOCUMENT_BYTES,
        ResourceKind::Css => MAX_STYLESHEET_BYTES,
        ResourceKind::Image => MAX_IMAGE_BYTES,
    }
}

/// Content-encoded HTTP bytes, after transfer framing and before decoding.
pub(crate) fn encoded_byte_limit(kind: ResourceKind) -> usize {
    match kind {
        ResourceKind::Html => 20 * 1024 * 1024,
        ResourceKind::Css => 4 * 1024 * 1024,
        ResourceKind::Image => 40 * 1024 * 1024,
    }
}

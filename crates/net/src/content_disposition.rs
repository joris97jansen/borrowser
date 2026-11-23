//! Bounded disposition-type extraction, not filename or download processing.
use crate::{HttpHeaders, headers::is_token_byte};

/// The declared disposition type. Parameters remain opaque response facts.
/// `Unusable` describes an ambiguous/malformed type, not a transport failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseContentDisposition {
    Missing,
    Inline,
    Attachment,
    Other,
    Unusable,
}

impl ResponseContentDisposition {
    pub(crate) fn extract(headers: &HttpHeaders) -> Self {
        let mut fields = headers.values("content-disposition");
        let Some(value) = fields.next() else {
            return Self::Missing;
        };
        // This is not a list field. Do not choose an occurrence or comma-join it.
        if fields.next().is_some() {
            return Self::Unusable;
        }
        let mut value = value;
        while matches!(value.first(), Some(b' ' | b'\t')) {
            value = &value[1..];
        }
        // Only HTTP OWS is permitted, rather than general Unicode whitespace.
        if value.first().is_some_and(|b| !is_token_byte(*b)) {
            return Self::Unusable;
        }
        let end = value
            .iter()
            .position(|b| !is_token_byte(*b))
            .unwrap_or(value.len());
        if end == 0 {
            return Self::Unusable;
        }
        let mut suffix = &value[end..];
        while matches!(suffix.first(), Some(b' ' | b'\t')) {
            suffix = &suffix[1..];
        }
        if !suffix.is_empty() && suffix[0] != b';' {
            return Self::Unusable;
        }
        // No parameter parsing/recovery or filename decoding is performed here.
        let token = &value[..end];
        if token.eq_ignore_ascii_case(b"inline") {
            Self::Inline
        } else if token.eq_ignore_ascii_case(b"attachment") {
            Self::Attachment
        } else {
            Self::Other
        }
    }
}

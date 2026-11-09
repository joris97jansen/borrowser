use core_types::ResourceKind;

use crate::{HttpHeaders, NetworkErrorKind, limits::resource_byte_limit};

/// The two content-decoding operations with current static-resource consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContentCoding {
    Identity,
    Gzip,
}

impl ContentCoding {
    pub(crate) fn admit(headers: &HttpHeaders) -> Result<Self, NetworkErrorKind> {
        let mut selected = None;
        let mut count = 0;
        let mut present = false;
        for field in headers.values("content-encoding") {
            present = true;
            for member in field.split(|byte| *byte == b',') {
                let mut member = member;
                while matches!(member.first(), Some(b' ' | b'\t')) {
                    member = &member[1..];
                }
                while matches!(member.last(), Some(b' ' | b'\t')) {
                    member = &member[..member.len() - 1];
                }
                // RFC 9110 recipient list rules: empty positions are ignored;
                // a present logical list with no actual coding is rejected.
                if member.is_empty() {
                    continue;
                }
                if !member.iter().copied().all(crate::headers::is_token_byte) {
                    return Err(NetworkErrorKind::MalformedResponse);
                }
                count += 1;
                selected = if member.eq_ignore_ascii_case(b"identity") {
                    Some(Self::Identity)
                } else if member.eq_ignore_ascii_case(b"gzip")
                    || member.eq_ignore_ascii_case(b"x-gzip")
                {
                    Some(Self::Gzip)
                } else {
                    None
                };
            }
        }
        if !present {
            return Ok(Self::Identity);
        }
        if count == 0 {
            return Err(NetworkErrorKind::MalformedResponse);
        }
        if count != 1 {
            return Err(NetworkErrorKind::UnsupportedResponse);
        }
        selected.ok_or(NetworkErrorKind::UnsupportedResponse)
    }
}

/// Delivered content bytes, not encoded length or proof of a complete representation.
/// The route determines the cap; callers cannot supply an independent limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyProgress {
    route: ResourceKind,
    delivered_bytes: usize,
}

impl BodyProgress {
    pub fn try_new(route: ResourceKind, delivered_bytes: usize) -> Result<Self, NetworkErrorKind> {
        if delivered_bytes > resource_byte_limit(route) {
            return Err(NetworkErrorKind::ResourceLimit);
        }
        Ok(Self {
            route,
            delivered_bytes,
        })
    }

    pub fn route(self) -> ResourceKind {
        self.route
    }
    pub fn delivered_bytes(self) -> usize {
        self.delivered_bytes
    }
    pub fn limit_bytes(self) -> usize {
        resource_byte_limit(self.route)
    }
    pub fn matches_route(self, route: ResourceKind) -> bool {
        self.route == route
    }
}

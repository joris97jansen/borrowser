use std::fmt;

use crate::{HttpHeaders, headers::split_header_value};

/// Fetch's declared-length projection. This is never a framing decision,
/// allocation request, observed-byte count or proof of body completion.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeclaredContentLength {
    /// No HTTP declaration, including local-loader results.
    Missing,
    /// Equal candidates do not form a nonempty ASCII decimal declaration.
    Unusable,
    /// Textual candidates disagree, even if their numeric values would agree.
    Conflicting,
    /// A numeric declaration outside Borrowser's supported u64 projection.
    OutOfRange,
    Value(u64),
}

impl DeclaredContentLength {
    pub(crate) fn extract(headers: &HttpHeaders) -> Self {
        let Some(combined) = headers.combined_value("content-length") else {
            return Self::Missing;
        };
        let mut values = split_header_value(&combined);
        let candidate = values.next().expect("split retains an empty member");
        // Compare all textual candidates before syntax/range interpretation.
        // AI5's backend separately admits numerically agreeing framing values.
        if values.any(|value| value != candidate) {
            return Self::Conflicting;
        }
        if candidate.is_empty() || !candidate.iter().all(u8::is_ascii_digit) {
            return Self::Unusable;
        }
        candidate
            .iter()
            .try_fold(0u64, |n, digit| {
                n.checked_mul(10)?.checked_add(u64::from(*digit - b'0'))
            })
            .map_or(Self::OutOfRange, Self::Value)
    }
}

impl fmt::Debug for DeclaredContentLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("Missing"),
            Self::Unusable => f.write_str("Unusable"),
            Self::Conflicting => f.write_str("Conflicting"),
            Self::OutOfRange => f.write_str("OutOfRange"),
            Self::Value(_) => f.debug_tuple("Value").field(&"<redacted>").finish(),
        }
    }
}

use std::fmt;

/// Borrowser categories for observed recoverable backend callbacks. These are
/// not an exhaustive WHATWG validation taxonomy and carry no source positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlDiagnostic {
    Backslash,
    BoundaryControlOrSpaceIgnored,
    /// Strict domain-to-ASCII validation failed; non-strict parsing may still succeed.
    DomainToAscii,
    EmbeddedCredentials,
    ExpectedAuthoritySlashes,
    ExpectedFileSlashes,
    FileHostWithDriveLetter,
    NonUrlCodePoint,
    NullInFragment,
    InvalidPercentEncoding,
    TabOrNewlineIgnored,
    UnencodedAtSign,
    /// A newer backend emitted an event without a Borrowser category yet.
    Unclassified,
}

impl UrlDiagnostic {
    pub(crate) fn from_backend(event: url::SyntaxViolation) -> Self {
        use url::SyntaxViolation as V;
        match event {
            V::Backslash => Self::Backslash,
            V::C0SpaceIgnored => Self::BoundaryControlOrSpaceIgnored,
            V::DomainToAscii => Self::DomainToAscii,
            V::EmbeddedCredentials => Self::EmbeddedCredentials,
            V::ExpectedDoubleSlash => Self::ExpectedAuthoritySlashes,
            V::ExpectedFileDoubleSlash => Self::ExpectedFileSlashes,
            V::FileWithHostAndWindowsDrive => Self::FileHostWithDriveLetter,
            V::NonUrlCodePoint => Self::NonUrlCodePoint,
            V::NullInFragment => Self::NullInFragment,
            V::PercentDecode => Self::InvalidPercentEncoding,
            V::TabOrNewlineIgnored => Self::TabOrNewlineIgnored,
            V::UnencodedAtSign => Self::UnencodedAtSign,
            _ => Self::Unclassified,
        }
    }
}

/// Fatal input or base errors. Unsupported resource operations are not parsing
/// failures and are deliberately outside this API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlParseError {
    EmptyHost,
    InvalidDomain,
    InvalidDomainCharacter,
    InvalidPort,
    InvalidIpv4Address,
    InvalidIpv6Address,
    RelativeWithoutBase,
    RelativeAgainstOpaqueBase,
}

/// Distinguishes invalid input/base context from parser implementation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlFailureKind {
    InvalidInput(UrlParseError),
    /// The backend cannot represent the URL's size. This is not invalid syntax.
    CapacityExceeded,
    /// An unexpected or newly introduced backend error needs adapter review.
    BackendFailure,
}

impl UrlFailureKind {
    pub(crate) fn from_backend(error: url::ParseError) -> Self {
        use url::ParseError as E;
        let error = match error {
            E::EmptyHost => UrlParseError::EmptyHost,
            E::IdnaError => UrlParseError::InvalidDomain,
            E::InvalidDomainCharacter => UrlParseError::InvalidDomainCharacter,
            E::InvalidPort => UrlParseError::InvalidPort,
            E::InvalidIpv4Address => UrlParseError::InvalidIpv4Address,
            E::InvalidIpv6Address => UrlParseError::InvalidIpv6Address,
            E::RelativeUrlWithoutBase => UrlParseError::RelativeWithoutBase,
            E::RelativeUrlWithCannotBeABaseBase => UrlParseError::RelativeAgainstOpaqueBase,
            E::Overflow => return Self::CapacityExceeded,
            // A setter-only error cannot be an input verdict from our immutable
            // parse API. Unknown non-exhaustive variants are conservative too.
            E::SetHostOnCannotBeABaseUrl => return Self::BackendFailure,
            _ => return Self::BackendFailure,
        };
        Self::InvalidInput(error)
    }
}

/// A failed operation, retaining events observed before failure. Neither input
/// strings nor backend error messages are stored. A resolution failure retains
/// its explicit base, whose Debug redacts credentials. Document/base-selection
/// provenance remains the source producer's responsibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlFailure {
    pub kind: UrlFailureKind,
    pub diagnostics: Vec<UrlDiagnostic>,
    /// `None` for absolute parsing, `Some` for resolution against this base.
    pub base: Option<crate::BrowserUrl>,
}

impl fmt::Display for UrlFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "URL operation failed: {:?}", self.kind)
    }
}

impl std::error::Error for UrlFailure {}

#[cfg(test)]
mod tests {
    use super::UrlFailureKind;

    #[test]
    fn backend_limits_and_unexpected_setter_errors_are_not_input_errors() {
        // Exercise unreachable-in-small-fixtures categories without allocating
        // a multi-gigabyte URL or exposing mutation in the engine API.
        assert_eq!(
            UrlFailureKind::from_backend(url::ParseError::Overflow),
            UrlFailureKind::CapacityExceeded
        );
        assert_eq!(
            UrlFailureKind::from_backend(url::ParseError::SetHostOnCannotBeABaseUrl),
            UrlFailureKind::BackendFailure
        );
    }
}

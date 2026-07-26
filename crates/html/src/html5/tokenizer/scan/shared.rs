#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AsciiPrefixMatch {
    Matched,
    NeedMoreInput,
    NoMatch,
    InvariantFailure,
}

pub(crate) fn match_ascii_prefix_ci_at(
    bytes: &[u8],
    at: usize,
    pattern: &[u8],
) -> AsciiPrefixMatch {
    let Some(end) = at.checked_add(pattern.len()) else {
        return AsciiPrefixMatch::InvariantFailure;
    };
    if end > bytes.len() {
        let Some(available) = bytes.len().checked_sub(at) else {
            return AsciiPrefixMatch::InvariantFailure;
        };
        if bytes
            .get(at..)
            .is_some_and(|tail| pattern[..available].eq_ignore_ascii_case(tail))
        {
            return AsciiPrefixMatch::NeedMoreInput;
        }
        return AsciiPrefixMatch::NoMatch;
    }

    if bytes[at..end].eq_ignore_ascii_case(pattern) {
        AsciiPrefixMatch::Matched
    } else {
        AsciiPrefixMatch::NoMatch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DoctypeKeywordKind {
    Public,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum QuotedParse<'a> {
    Complete {
        value: &'a str,
        value_start: usize,
        cursor_after: usize,
    },
    InvariantFailure,
    LimitExceeded,
    NeedMoreInput,
    Malformed,
}

#[cfg(test)]
mod tests {
    use super::{AsciiPrefixMatch, match_ascii_prefix_ci_at};

    #[test]
    fn ascii_prefix_scan_distinguishes_invalid_ranges_from_input_mismatch() {
        assert_eq!(
            match_ascii_prefix_ci_at(b"PUBLIC", 0, b"public"),
            AsciiPrefixMatch::Matched
        );
        assert_eq!(
            match_ascii_prefix_ci_at(b"PUB", 0, b"public"),
            AsciiPrefixMatch::NeedMoreInput
        );
        assert_eq!(
            match_ascii_prefix_ci_at(b"PRIVATE", 0, b"public"),
            AsciiPrefixMatch::NoMatch
        );
        assert_eq!(
            match_ascii_prefix_ci_at(b"PUBLIC", 7, b"public"),
            AsciiPrefixMatch::InvariantFailure
        );
        assert_eq!(
            match_ascii_prefix_ci_at(b"", usize::MAX, b"public"),
            AsciiPrefixMatch::InvariantFailure
        );
    }
}

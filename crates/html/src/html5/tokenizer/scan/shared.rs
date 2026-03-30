use super::super::input::MatchResult;

pub(crate) fn match_ascii_prefix_ci_at(bytes: &[u8], at: usize, pattern: &[u8]) -> MatchResult {
    if at + pattern.len() > bytes.len() {
        let available = bytes.len().saturating_sub(at);
        if bytes
            .get(at..)
            .is_some_and(|tail| pattern[..available].eq_ignore_ascii_case(tail))
        {
            return MatchResult::NeedMoreInput;
        }
        return MatchResult::NoMatch;
    }

    if bytes[at..at + pattern.len()].eq_ignore_ascii_case(pattern) {
        MatchResult::Matched
    } else {
        MatchResult::NoMatch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DoctypeKeywordKind {
    Public,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum QuotedParse<'a> {
    Complete((&'a str, usize)),
    LimitExceeded,
    NeedMoreInput,
    Malformed,
}

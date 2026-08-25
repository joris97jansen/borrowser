use std::cmp::Ordering;

use crate::selectors::AttributeMatcher;

/// Case policy for selector value comparison.
///
/// This type is intentionally limited to symmetric value comparison. HTML
/// element and attribute names use the asymmetric host-language name matcher
/// in `host_language` instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextCaseSensitivity {
    Sensitive,
    AsciiInsensitive,
}

impl TextCaseSensitivity {
    pub(super) fn equals(self, actual: &str, expected: &str) -> bool {
        match self {
            Self::Sensitive => actual == expected,
            Self::AsciiInsensitive => actual.eq_ignore_ascii_case(expected),
        }
    }

    /// Compares selector values with the same ASCII-only case policy used by
    /// equality matching, without materializing a folded owned string.
    pub(super) fn compare(self, left: &str, right: &str) -> Ordering {
        match self {
            Self::Sensitive => left.cmp(right),
            Self::AsciiInsensitive => left
                .bytes()
                .map(|byte| byte.to_ascii_lowercase())
                .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase())),
        }
    }

    pub(super) fn has_prefix(self, actual: &str, expected: &str) -> bool {
        match self {
            Self::Sensitive => actual.starts_with(expected),
            Self::AsciiInsensitive => actual
                .as_bytes()
                .get(..expected.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(expected.as_bytes())),
        }
    }

    pub(super) fn has_suffix(self, actual: &str, expected: &str) -> bool {
        match self {
            Self::Sensitive => actual.ends_with(expected),
            Self::AsciiInsensitive => actual
                .len()
                .checked_sub(expected.len())
                .and_then(|start| actual.as_bytes().get(start..))
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(expected.as_bytes())),
        }
    }

    pub(super) fn contains(self, actual: &str, expected: &str) -> bool {
        match self {
            Self::Sensitive => actual.contains(expected),
            Self::AsciiInsensitive => {
                if expected.is_empty() {
                    return true;
                }

                actual
                    .as_bytes()
                    .windows(expected.len())
                    .any(|window| window.eq_ignore_ascii_case(expected.as_bytes()))
            }
        }
    }
}

pub(super) fn contains_css_whitespace_token(
    actual: &str,
    expected: &str,
    sensitivity: TextCaseSensitivity,
) -> bool {
    split_css_whitespace_separated_tokens(actual).any(|token| sensitivity.equals(token, expected))
}

pub(super) fn matches_attribute_value(
    matcher: AttributeMatcher,
    actual: &str,
    expected: &str,
    sensitivity: TextCaseSensitivity,
) -> bool {
    match matcher {
        AttributeMatcher::Exact => sensitivity.equals(actual, expected),
        AttributeMatcher::Includes => {
            !expected.is_empty()
                && !contains_css_whitespace(expected)
                && contains_css_whitespace_token(actual, expected, sensitivity)
        }
        AttributeMatcher::DashMatch => {
            sensitivity.equals(actual, expected)
                || (sensitivity.has_prefix(actual, expected)
                    && actual.as_bytes().get(expected.len()) == Some(&b'-'))
        }
        AttributeMatcher::Prefix => {
            !expected.is_empty() && sensitivity.has_prefix(actual, expected)
        }
        AttributeMatcher::Suffix => {
            !expected.is_empty() && sensitivity.has_suffix(actual, expected)
        }
        AttributeMatcher::Substring => {
            !expected.is_empty() && sensitivity.contains(actual, expected)
        }
    }
}

pub(crate) fn split_css_whitespace_separated_tokens(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(is_css_selector_whitespace)
        .filter(|token| !token.is_empty())
}

fn contains_css_whitespace(value: &str) -> bool {
    value.chars().any(is_css_selector_whitespace)
}

fn is_css_selector_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUE_OPERATORS: [AttributeMatcher; 6] = [
        AttributeMatcher::Exact,
        AttributeMatcher::Includes,
        AttributeMatcher::DashMatch,
        AttributeMatcher::Prefix,
        AttributeMatcher::Suffix,
        AttributeMatcher::Substring,
    ];

    #[test]
    fn value_comparison_primitives_fold_ascii_without_unicode_case_folding() {
        let insensitive = TextCaseSensitivity::AsciiInsensitive;
        let sensitive = TextCaseSensitivity::Sensitive;

        assert!(insensitive.equals("FOO-é-BAR", "foo-é-bar"));
        assert!(insensitive.has_prefix("FOO-é-BAR-tail", "foo-é-bar"));
        assert!(insensitive.has_suffix("head-FOO-é-BAR", "foo-é-bar"));
        assert!(insensitive.contains("head-FOO-é-BAR-tail", "foo-é-bar"));

        assert!(!sensitive.equals("FOO-é-BAR", "foo-é-bar"));
        assert!(!insensitive.equals("FOO-É-BAR", "foo-é-bar"));
        assert!(!insensitive.has_prefix("FOO-É-BAR-tail", "foo-é-bar"));
        assert!(!insensitive.has_suffix("head-FOO-É-BAR", "foo-é-bar"));
        assert!(!insensitive.contains("head-FOO-É-BAR-tail", "foo-é-bar"));

        assert_eq!(insensitive.compare("FOO-é", "foo-é"), Ordering::Equal);
        assert_eq!(insensitive.compare("FOO-a", "foo-b"), Ordering::Less);
        assert_eq!(sensitive.compare("FOO", "foo"), Ordering::Less);
    }

    #[test]
    fn all_attribute_value_operators_share_the_selected_ascii_case_policy() {
        let cases = [
            (AttributeMatcher::Exact, "VaLuE", "value"),
            (AttributeMatcher::Includes, "left VaLuE right", "value"),
            (AttributeMatcher::DashMatch, "VaLuE-tail", "value"),
            (AttributeMatcher::Prefix, "VaLuE-tail", "value"),
            (AttributeMatcher::Suffix, "head-VaLuE", "value"),
            (AttributeMatcher::Substring, "head-VaLuE-tail", "value"),
        ];

        for (matcher, actual, expected) in cases {
            assert!(matches_attribute_value(
                matcher,
                actual,
                expected,
                TextCaseSensitivity::AsciiInsensitive,
            ));
            assert!(!matches_attribute_value(
                matcher,
                actual,
                expected,
                TextCaseSensitivity::Sensitive,
            ));
        }
    }

    #[test]
    fn attribute_operator_empty_needles_keep_independent_semantics() {
        for sensitivity in [
            TextCaseSensitivity::Sensitive,
            TextCaseSensitivity::AsciiInsensitive,
        ] {
            assert!(matches_attribute_value(
                AttributeMatcher::Exact,
                "",
                "",
                sensitivity
            ));
            assert!(!matches_attribute_value(
                AttributeMatcher::Exact,
                "x",
                "",
                sensitivity
            ));
            assert!(!matches_attribute_value(
                AttributeMatcher::Includes,
                "",
                "",
                sensitivity
            ));
            assert!(!matches_attribute_value(
                AttributeMatcher::Includes,
                "x",
                "",
                sensitivity
            ));
            assert!(matches_attribute_value(
                AttributeMatcher::DashMatch,
                "",
                "",
                sensitivity
            ));
            assert!(matches_attribute_value(
                AttributeMatcher::DashMatch,
                "-x",
                "",
                sensitivity
            ));
            assert!(!matches_attribute_value(
                AttributeMatcher::DashMatch,
                "x",
                "",
                sensitivity
            ));

            for matcher in [
                AttributeMatcher::Prefix,
                AttributeMatcher::Suffix,
                AttributeMatcher::Substring,
            ] {
                assert!(!matches_attribute_value(matcher, "", "", sensitivity));
                assert!(!matches_attribute_value(matcher, "x", "", sensitivity));
            }
        }
    }

    #[test]
    fn includes_uses_only_css_selector_whitespace() {
        for whitespace in ['\t', '\n', '\u{000C}', '\r', ' '] {
            let actual = format!("left{whitespace}target{whitespace}right");
            assert!(matches_attribute_value(
                AttributeMatcher::Includes,
                &actual,
                "target",
                TextCaseSensitivity::Sensitive,
            ));

            let expected = format!("left{whitespace}target");
            assert!(!matches_attribute_value(
                AttributeMatcher::Includes,
                &actual,
                &expected,
                TextCaseSensitivity::Sensitive,
            ));
        }

        assert!(matches_attribute_value(
            AttributeMatcher::Includes,
            "left\u{00A0}target tail",
            "left\u{00A0}target",
            TextCaseSensitivity::Sensitive,
        ));
        assert!(!matches_attribute_value(
            AttributeMatcher::Includes,
            "left\u{00A0}target tail",
            "target",
            TextCaseSensitivity::Sensitive,
        ));
    }

    #[test]
    fn every_operator_preserves_non_ascii_code_points_exactly() {
        let matching_cases = [
            "FOO-é-BAR".to_string(),
            "left FOO-é-BAR right".to_string(),
            "FOO-é-BAR-tail".to_string(),
            "FOO-é-BAR-tail".to_string(),
            "head-FOO-é-BAR".to_string(),
            "head-FOO-é-BAR-tail".to_string(),
        ];
        let nonmatching_cases = [
            "FOO-É-BAR".to_string(),
            "left FOO-É-BAR right".to_string(),
            "FOO-É-BAR-tail".to_string(),
            "FOO-É-BAR-tail".to_string(),
            "head-FOO-É-BAR".to_string(),
            "head-FOO-É-BAR-tail".to_string(),
        ];

        for ((matcher, matching), nonmatching) in VALUE_OPERATORS
            .into_iter()
            .zip(matching_cases)
            .zip(nonmatching_cases)
        {
            assert!(matches_attribute_value(
                matcher,
                &matching,
                "foo-é-bar",
                TextCaseSensitivity::AsciiInsensitive,
            ));
            assert!(!matches_attribute_value(
                matcher,
                &nonmatching,
                "foo-é-bar",
                TextCaseSensitivity::AsciiInsensitive,
            ));
        }
    }
}

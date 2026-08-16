use html::{AttributeNamespace, DocumentMode, ElementNamespace};

use super::comparison::TextCaseSensitivity;
use super::context::SelectorDomAttribute;

const HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES: [&str; 46] = [
    "accept",
    "accept-charset",
    "align",
    "alink",
    "axis",
    "bgcolor",
    "charset",
    "checked",
    "clear",
    "codetype",
    "color",
    "compact",
    "declare",
    "defer",
    "dir",
    "direction",
    "disabled",
    "enctype",
    "face",
    "frame",
    "hreflang",
    "http-equiv",
    "lang",
    "language",
    "link",
    "media",
    "method",
    "multiple",
    "nohref",
    "noresize",
    "noshade",
    "nowrap",
    "readonly",
    "rel",
    "rev",
    "rules",
    "scope",
    "scrolling",
    "selected",
    "shape",
    "target",
    "text",
    "type",
    "valign",
    "valuetype",
    "vlink",
];

/// Host-language rule for comparing a selector/requested name with one exact
/// DOM local name.
///
/// This is deliberately distinct from selector value comparison. HTML name
/// matching lowercases ASCII on the selector/request side only; it does not
/// reinterpret a noncanonical actual DOM name through symmetric folding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostLanguageNameMatch {
    Exact,
    AsciiLowercaseSelector,
}

impl HostLanguageNameMatch {
    fn matches(self, actual_name: &str, selector_name: &str) -> bool {
        match self {
            Self::Exact => actual_name == selector_name,
            Self::AsciiLowercaseSelector => actual_name
                .bytes()
                .eq(selector_name.bytes().map(ascii_lowercase_byte)),
        }
    }
}

const fn ascii_lowercase_byte(byte: u8) -> u8 {
    if byte >= b'A' && byte <= b'Z' {
        byte + (b'a' - b'A')
    } else {
        byte
    }
}

const fn type_selector_name_rule(namespace: ElementNamespace) -> HostLanguageNameMatch {
    match namespace {
        ElementNamespace::Html => HostLanguageNameMatch::AsciiLowercaseSelector,
        ElementNamespace::Svg | ElementNamespace::MathMl => HostLanguageNameMatch::Exact,
    }
}

const fn unqualified_attribute_name_rule(namespace: ElementNamespace) -> HostLanguageNameMatch {
    match namespace {
        ElementNamespace::Html => HostLanguageNameMatch::AsciiLowercaseSelector,
        ElementNamespace::Svg | ElementNamespace::MathMl => HostLanguageNameMatch::Exact,
    }
}

pub(super) fn matches_type_selector_name(
    element_namespace: ElementNamespace,
    actual_local_name: &str,
    selector_name: &str,
) -> bool {
    type_selector_name_rule(element_namespace).matches(actual_local_name, selector_name)
}

/// Applies the host-language name rule used by the shared effective
/// unqualified-attribute lookup.
///
/// `requested_name` is the selector/query side. Selector matching supplies an
/// authored selector identifier, while inline-style discovery supplies the
/// canonical internal request `style`. The actual DOM name is never folded.
pub(crate) fn matches_unqualified_attribute_name(
    element_namespace: ElementNamespace,
    actual_local_name: &str,
    requested_name: &str,
) -> bool {
    unqualified_attribute_name_rule(element_namespace).matches(actual_local_name, requested_name)
}

pub(super) const fn id_and_class_selector_value_case(
    document_mode: DocumentMode,
) -> TextCaseSensitivity {
    match document_mode {
        DocumentMode::NoQuirks => TextCaseSensitivity::Sensitive,
        DocumentMode::LimitedQuirks => TextCaseSensitivity::Sensitive,
        DocumentMode::Quirks => TextCaseSensitivity::AsciiInsensitive,
    }
}

pub(super) fn attribute_selector_value_case(
    element_namespace: ElementNamespace,
    effective_attribute: SelectorDomAttribute<'_>,
) -> TextCaseSensitivity {
    if element_namespace == ElementNamespace::Html
        && effective_attribute.namespace() == AttributeNamespace::None
        && HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES
            .binary_search(&effective_attribute.local_name())
            .is_ok()
    {
        TextCaseSensitivity::AsciiInsensitive
    } else {
        TextCaseSensitivity::Sensitive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selectors::AttributeMatcher;
    use crate::selectors::matching::comparison::matches_attribute_value;

    const EXPECTED_HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES: [&str; 46] = [
        "accept",
        "accept-charset",
        "align",
        "alink",
        "axis",
        "bgcolor",
        "charset",
        "checked",
        "clear",
        "codetype",
        "color",
        "compact",
        "declare",
        "defer",
        "dir",
        "direction",
        "disabled",
        "enctype",
        "face",
        "frame",
        "hreflang",
        "http-equiv",
        "lang",
        "language",
        "link",
        "media",
        "method",
        "multiple",
        "nohref",
        "noresize",
        "noshade",
        "nowrap",
        "readonly",
        "rel",
        "rev",
        "rules",
        "scope",
        "scrolling",
        "selected",
        "shape",
        "target",
        "text",
        "type",
        "valign",
        "valuetype",
        "vlink",
    ];

    const VALUE_OPERATOR_CASES: [(AttributeMatcher, &str, &str); 6] = [
        (AttributeMatcher::Exact, "VaLuE", "value"),
        (AttributeMatcher::Includes, "left VaLuE right", "value"),
        (AttributeMatcher::DashMatch, "VaLuE-tail", "value"),
        (AttributeMatcher::Prefix, "VaLuE-tail", "value"),
        (AttributeMatcher::Suffix, "head-VaLuE", "value"),
        (AttributeMatcher::Substring, "head-VaLuE-tail", "value"),
    ];

    #[test]
    fn html_name_matching_ascii_lowercases_only_the_selector_side() {
        assert!(matches_type_selector_name(
            ElementNamespace::Html,
            "type",
            "TYPE"
        ));
        assert!(!matches_type_selector_name(
            ElementNamespace::Html,
            "TYPE",
            "TYPE"
        ));
        assert!(!matches_type_selector_name(
            ElementNamespace::Html,
            "TYPE",
            "type"
        ));
    }

    #[test]
    fn foreign_name_matching_remains_exact() {
        assert!(matches_type_selector_name(
            ElementNamespace::Svg,
            "foreignObject",
            "foreignObject"
        ));
        assert!(!matches_type_selector_name(
            ElementNamespace::Svg,
            "foreignObject",
            "FOREIGNOBJECT"
        ));
        assert!(matches_unqualified_attribute_name(
            ElementNamespace::MathMl,
            "TYPE",
            "TYPE"
        ));
        assert!(!matches_unqualified_attribute_name(
            ElementNamespace::MathMl,
            "type",
            "TYPE"
        ));
    }

    #[test]
    fn selector_side_name_normalization_is_ascii_only() {
        assert!(matches_type_selector_name(
            ElementNamespace::Html,
            "foo-é-bar",
            "FOO-é-BAR"
        ));
        assert!(!matches_type_selector_name(
            ElementNamespace::Html,
            "foo-é-bar",
            "FOO-É-BAR"
        ));
    }

    #[test]
    fn id_and_class_value_policy_distinguishes_all_document_modes() {
        assert_eq!(
            id_and_class_selector_value_case(DocumentMode::NoQuirks),
            TextCaseSensitivity::Sensitive
        );
        assert_eq!(
            id_and_class_selector_value_case(DocumentMode::LimitedQuirks),
            TextCaseSensitivity::Sensitive
        );
        assert_eq!(
            id_and_class_selector_value_case(DocumentMode::Quirks),
            TextCaseSensitivity::AsciiInsensitive
        );
    }

    #[test]
    fn html_insensitive_value_inventory_is_exact_complete_sorted_and_unique() {
        assert_eq!(HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES.len(), 46);
        assert!(
            HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "the production inventory must remain strictly sorted and duplicate-free"
        );
        assert_eq!(
            HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES,
            EXPECTED_HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES
        );
    }

    #[test]
    fn html_insensitive_value_inventory_covers_every_name_and_operator() {
        let mut cases = 0usize;

        for local_name in EXPECTED_HTML_ASCII_CASE_INSENSITIVE_VALUE_ATTRIBUTE_NAMES {
            for (matcher, actual, expected) in VALUE_OPERATOR_CASES {
                let attribute =
                    SelectorDomAttribute::new(AttributeNamespace::None, local_name, actual);
                let sensitivity = attribute_selector_value_case(ElementNamespace::Html, attribute);

                assert_eq!(sensitivity, TextCaseSensitivity::AsciiInsensitive);
                assert!(matches_attribute_value(
                    matcher,
                    attribute.value(),
                    expected,
                    sensitivity,
                ));
                assert!(
                    !matches_attribute_value(
                        matcher,
                        attribute.value(),
                        expected,
                        TextCaseSensitivity::Sensitive,
                    ),
                    "fixture for {local_name:?} and {matcher:?} must require ASCII folding"
                );
                cases += 1;
            }
        }

        assert_eq!(cases, 46 * 6);
    }

    #[test]
    fn attribute_value_policy_requires_exact_effective_html_identity() {
        for ordinary_name in ["id", "class", "style", "data-kind", "types"] {
            let attribute =
                SelectorDomAttribute::new(AttributeNamespace::None, ordinary_name, "VaLuE");
            assert_eq!(
                attribute_selector_value_case(ElementNamespace::Html, attribute),
                TextCaseSensitivity::Sensitive
            );
        }

        let noncanonical = SelectorDomAttribute::new(AttributeNamespace::None, "TYPE", "BuTtOn");
        assert_eq!(
            attribute_selector_value_case(ElementNamespace::Html, noncanonical),
            TextCaseSensitivity::Sensitive,
            "the inventory must not fold a noncanonical actual DOM local name"
        );
    }

    #[test]
    fn html_value_policy_excludes_foreign_elements_and_qualified_attributes() {
        let unqualified = SelectorDomAttribute::new(AttributeNamespace::None, "type", "BuTtOn");
        assert_eq!(
            attribute_selector_value_case(ElementNamespace::Svg, unqualified),
            TextCaseSensitivity::Sensitive
        );
        assert_eq!(
            attribute_selector_value_case(ElementNamespace::MathMl, unqualified),
            TextCaseSensitivity::Sensitive
        );

        for namespace in [
            AttributeNamespace::Xml,
            AttributeNamespace::Xmlns,
            AttributeNamespace::XLink,
        ] {
            let qualified = SelectorDomAttribute::new(namespace, "type", "BuTtOn");
            assert_eq!(
                attribute_selector_value_case(ElementNamespace::Html, qualified),
                TextCaseSensitivity::Sensitive
            );
        }
    }

    #[test]
    fn ordinary_html_attribute_values_remain_sensitive_for_every_operator() {
        for (matcher, actual, expected) in VALUE_OPERATOR_CASES {
            let attribute =
                SelectorDomAttribute::new(AttributeNamespace::None, "data-kind", actual);
            let sensitivity = attribute_selector_value_case(ElementNamespace::Html, attribute);

            assert_eq!(sensitivity, TextCaseSensitivity::Sensitive);
            assert!(!matches_attribute_value(
                matcher,
                attribute.value(),
                expected,
                sensitivity,
            ));
        }
    }
}

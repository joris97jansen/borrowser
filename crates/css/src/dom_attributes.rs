use crate::selectors::SelectorDomAttribute;
use html::{AttributeNamespace, ElementNamespace};

/// Returns the first unqualified DOM attribute whose effective local name
/// matches `requested_local_name`.
///
/// This helper is shared CSS-side policy over neutral DOM facts. It preserves
/// provider order, applies the current HTML-versus-foreign local-name policy,
/// and deliberately does not implement selector value operators, ID matching,
/// class tokenization, or pseudo-class semantics.
pub(crate) fn first_effective_unqualified_attribute<'a>(
    element_namespace: ElementNamespace,
    attributes: impl IntoIterator<Item = SelectorDomAttribute<'a>>,
    requested_local_name: &str,
) -> Option<SelectorDomAttribute<'a>> {
    attributes.into_iter().find(|attribute| {
        attribute.namespace() == AttributeNamespace::None
            && crate::selectors::matching::matches_unqualified_attribute_name(
                element_namespace,
                attribute.local_name(),
                requested_local_name,
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_lookup_preserves_order_and_keeps_policy_css_owned() {
        let attributes = [
            SelectorDomAttribute::new(AttributeNamespace::None, "data-kind", "first"),
            SelectorDomAttribute::new(AttributeNamespace::None, "data-kind", "second"),
        ];

        assert_eq!(
            first_effective_unqualified_attribute(ElementNamespace::Html, attributes, "DATA-KIND",)
                .map(SelectorDomAttribute::value),
            Some("first")
        );
        assert_eq!(
            first_effective_unqualified_attribute(ElementNamespace::Svg, attributes, "DATA-KIND")
                .map(SelectorDomAttribute::value),
            None
        );
        assert_eq!(
            first_effective_unqualified_attribute(ElementNamespace::Svg, attributes, "data-kind")
                .map(SelectorDomAttribute::value),
            Some("first")
        );
    }

    #[test]
    fn effective_lookup_ignores_qualified_attributes() {
        let attributes = [
            SelectorDomAttribute::new(AttributeNamespace::XLink, "href", "qualified"),
            SelectorDomAttribute::new(AttributeNamespace::None, "href", "ordinary"),
        ];

        assert_eq!(
            first_effective_unqualified_attribute(ElementNamespace::Svg, attributes, "href")
                .map(SelectorDomAttribute::value),
            Some("ordinary")
        );
    }

    #[test]
    fn effective_lookup_ascii_lowercases_only_the_html_request_side() {
        let attributes = [
            SelectorDomAttribute::new(AttributeNamespace::None, "TYPE", "noncanonical"),
            SelectorDomAttribute::new(AttributeNamespace::None, "type", "canonical"),
        ];

        assert_eq!(
            first_effective_unqualified_attribute(ElementNamespace::Html, attributes, "TYPE")
                .map(SelectorDomAttribute::value),
            Some("canonical"),
            "a noncanonical actual HTML name must not match or shadow the canonical name"
        );
        assert_eq!(
            first_effective_unqualified_attribute(
                ElementNamespace::Html,
                [SelectorDomAttribute::new(
                    AttributeNamespace::None,
                    "TYPE",
                    "noncanonical",
                )],
                "TYPE",
            ),
            None
        );
    }

    #[test]
    fn effective_lookup_keeps_foreign_attribute_names_exact() {
        let attributes = [SelectorDomAttribute::new(
            AttributeNamespace::None,
            "viewBox",
            "0 0 10 10",
        )];

        assert!(
            first_effective_unqualified_attribute(ElementNamespace::Svg, attributes, "viewBox")
                .is_some()
        );
        assert_eq!(
            first_effective_unqualified_attribute(ElementNamespace::Svg, attributes, "VIEWBOX"),
            None
        );
    }
}

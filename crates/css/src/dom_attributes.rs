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
            && if element_namespace == ElementNamespace::Html {
                attribute
                    .local_name()
                    .eq_ignore_ascii_case(requested_local_name)
            } else {
                attribute.local_name() == requested_local_name
            }
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
}

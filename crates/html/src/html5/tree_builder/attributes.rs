use crate::attributes::ParserCreatedAttribute;
use crate::html5::shared::{AtomTable, Attribute, EngineInvariantError};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::resolve::resolve_attribute_value;

pub(in crate::html5::tree_builder) type ParserCreatedAttributes = Vec<ParserCreatedAttribute>;

pub(in crate::html5::tree_builder) fn resolve_token_attributes_first_wins(
    attrs: &[Attribute],
    atoms: &AtomTable,
    text: &dyn TextResolver,
) -> Result<ParserCreatedAttributes, TreeBuilderError> {
    let mut seen = Vec::new();
    let mut attributes = Vec::with_capacity(attrs.len());
    for attr in attrs {
        if seen.contains(&attr.name) {
            continue;
        }
        seen.push(attr.name);
        let local_name = atoms
            .resolve_local_name(attr.name)
            .ok_or(EngineInvariantError)?;
        attributes.push(ParserCreatedAttribute::new(
            crate::attributes::QualifiedAttributeName::unqualified(local_name),
            resolve_attribute_value(attr, text)?,
        ));
    }
    Ok(attributes)
}

pub(in crate::html5::tree_builder) fn snapshot_token_attributes_first_wins(
    attrs: &[Attribute],
    atoms: &AtomTable,
    text: &dyn TextResolver,
) -> Result<ParserCreatedAttributes, TreeBuilderError> {
    resolve_token_attributes_first_wins(attrs, atoms, text)
}

pub(in crate::html5::tree_builder) fn resolve_afe_attributes_first_wins(
    attrs: &[ParserCreatedAttribute],
) -> ParserCreatedAttributes {
    attrs.to_vec()
}

/// HTML tree-construction "same attributes" comparison.
///
/// Encounter order and prefix do not participate. Parser-created lists contain
/// no duplicate expanded names, so deterministic one-to-one matching is
/// unambiguous.
pub(in crate::html5::tree_builder) fn same_attributes_for_html_parser(
    left: &[ParserCreatedAttribute],
    right: &[ParserCreatedAttribute],
) -> bool {
    left.len() == right.len()
        && left.iter().all(|left_attribute| {
            right.iter().any(|right_attribute| {
                left_attribute.namespace() == right_attribute.namespace()
                    && left_attribute.local_name() == right_attribute.local_name()
                    && left_attribute.value() == right_attribute.value()
            })
        })
}

#[cfg(test)]
mod tests {
    use super::same_attributes_for_html_parser;
    use crate::attributes::{ParserCreatedAttribute, QualifiedAttributeName};
    use crate::names::NameInterner;

    #[test]
    fn noahs_ark_attribute_equality_includes_namespace_but_not_order() {
        let mut names = NameInterner::new();
        let href = names.intern_exact("href").expect("href atom");
        let local = names.resolve_local_name(href).expect("href local name");
        let ordinary = ParserCreatedAttribute::new(
            QualifiedAttributeName::unqualified(local.clone()),
            "#x".to_string(),
        );
        let xlink =
            ParserCreatedAttribute::new(QualifiedAttributeName::xlink(local), "#x".to_string());

        assert!(!same_attributes_for_html_parser(
            std::slice::from_ref(&ordinary),
            std::slice::from_ref(&xlink),
        ));
        assert_ne!(
            ordinary, xlink,
            "exact DOM equality retains qualified shape"
        );

        let a = names.intern_exact("a").expect("a atom");
        let b = names.intern_exact("b").expect("b atom");
        let first = vec![
            ParserCreatedAttribute::new(
                QualifiedAttributeName::unqualified(
                    names.resolve_local_name(a).expect("a local name"),
                ),
                "1".to_string(),
            ),
            ParserCreatedAttribute::new(
                QualifiedAttributeName::unqualified(
                    names.resolve_local_name(b).expect("b local name"),
                ),
                "2".to_string(),
            ),
        ];
        let mut reversed = first.clone();
        reversed.reverse();
        assert!(same_attributes_for_html_parser(&first, &reversed));
        assert_ne!(first, reversed, "stored DOM order remains observable");

        let mut changed_value = reversed.clone();
        changed_value[0] = ParserCreatedAttribute::new(
            QualifiedAttributeName::unqualified(names.resolve_local_name(b).expect("b local name")),
            "different".to_string(),
        );
        assert!(!same_attributes_for_html_parser(&first, &changed_value));
    }
}

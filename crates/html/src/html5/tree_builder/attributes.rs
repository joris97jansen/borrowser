use crate::attributes::ParserCreatedAttribute;
use crate::html5::shared::{AtomTable, Attribute, UnsupportedFeatureObservationFailure};
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
            .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?;
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

/// Returns whether the applicable repeated-html/body rule would need to add
/// at least one unqualified token attribute to the authoritative live element.
///
/// This is observation-only: it neither resolves values nor mutates the
/// parser-created element. The quadratic first-wins scan avoids observer-owned
/// allocation and matches the canonical token attribute semantics.
pub(in crate::html5::tree_builder) fn has_missing_unqualified_token_attribute_first_wins(
    attrs: &[Attribute],
    existing: &[ParserCreatedAttribute],
    atoms: &AtomTable,
) -> Result<bool, UnsupportedFeatureObservationFailure> {
    for (index, attr) in attrs.iter().enumerate() {
        if attrs[..index]
            .iter()
            .any(|earlier| earlier.name == attr.name)
        {
            continue;
        }
        let local_name = atoms
            .resolve(attr.name)
            .ok_or(UnsupportedFeatureObservationFailure::TokenAttributeNameUnavailable)?;
        let already_present = existing.iter().any(|existing| {
            existing.namespace() == crate::attributes::AttributeNamespace::None
                && existing.local_name() == local_name
        });
        if !already_present {
            return Ok(true);
        }
    }
    Ok(false)
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
    use super::{
        has_missing_unqualified_token_attribute_first_wins, same_attributes_for_html_parser,
    };
    use crate::attributes::{ParserCreatedAttribute, QualifiedAttributeName};
    use crate::html5::shared::{AtomTable, Attribute, AttributeValue};
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

    #[test]
    fn repeated_element_merge_eligibility_is_first_wins_and_expanded_name_only() {
        let mut atoms = AtomTable::default();
        let a = atoms.intern_ascii_folded("a").unwrap();
        let b = atoms.intern_ascii_folded("b").unwrap();
        let a_local = atoms.resolve_local_name(a).unwrap();
        let existing = vec![ParserCreatedAttribute::new(
            QualifiedAttributeName::unqualified(a_local.clone()),
            "old".to_string(),
        )];
        let token_attribute = |name, value: &str| Attribute {
            name,
            value: AttributeValue::Owned(value.to_string()),
        };

        assert!(
            !has_missing_unqualified_token_attribute_first_wins(&[], &existing, &atoms).unwrap()
        );
        assert!(
            !has_missing_unqualified_token_attribute_first_wins(
                &[token_attribute(a, "different")],
                &existing,
                &atoms
            )
            .unwrap()
        );
        assert!(
            !has_missing_unqualified_token_attribute_first_wins(
                &[token_attribute(a, "first"), token_attribute(a, "duplicate")],
                &existing,
                &atoms
            )
            .unwrap()
        );
        assert!(
            has_missing_unqualified_token_attribute_first_wins(
                &[
                    token_attribute(a, "existing"),
                    token_attribute(b, "missing")
                ],
                &existing,
                &atoms
            )
            .unwrap()
        );

        let namespaced_only = vec![ParserCreatedAttribute::new(
            QualifiedAttributeName::xlink(a_local),
            "old".to_string(),
        )];
        assert!(
            has_missing_unqualified_token_attribute_first_wins(
                &[token_attribute(a, "unqualified")],
                &namespaced_only,
                &atoms
            )
            .unwrap()
        );
    }
}

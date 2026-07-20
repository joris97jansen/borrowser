use crate::attributes::{ParserCreatedAttribute, QualifiedAttributeName};
use crate::html5::shared::{AtomTable, Attribute, EngineInvariantError};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::resolve::{resolve_atom, resolve_attribute_value};
use crate::names::ElementNamespace;

pub(in crate::html5::tree_builder) struct AdjustedAttributes {
    pub(in crate::html5::tree_builder) attributes: Vec<ParserCreatedAttribute>,
    pub(in crate::html5::tree_builder) post_adjustment_collisions: usize,
}

pub(in crate::html5::tree_builder) fn adjust_foreign_attributes(
    namespace: ElementNamespace,
    attrs: &[Attribute],
    atoms: &AtomTable,
    text: &dyn TextResolver,
) -> Result<AdjustedAttributes, TreeBuilderError> {
    let mut adjusted = Vec::with_capacity(attrs.len());
    let mut collisions = 0usize;
    for attr in attrs {
        let raw = resolve_atom(atoms, attr.name)?;
        let ordinary_local = match namespace {
            ElementNamespace::Svg => super::svg_adjusted_attribute_name(raw),
            ElementNamespace::MathMl if raw == "definitionurl" => "definitionURL",
            _ => raw,
        };
        let interned = |local: &str| {
            atoms
                .lookup_exact(local)
                .and_then(|id| atoms.resolve_local_name(id))
                .ok_or(EngineInvariantError)
        };
        let name = match super::qualified_foreign_attribute_adjustment(raw) {
            Some(super::QualifiedForeignAttributeAdjustment::Xml(local)) => {
                QualifiedAttributeName::xml(interned(local)?)
            }
            Some(super::QualifiedForeignAttributeAdjustment::XLink(local)) => {
                QualifiedAttributeName::xlink(interned(local)?)
            }
            Some(super::QualifiedForeignAttributeAdjustment::XmlnsDefault) => {
                QualifiedAttributeName::xmlns_default()
            }
            Some(super::QualifiedForeignAttributeAdjustment::XmlnsPrefixed(local)) => {
                QualifiedAttributeName::xmlns_prefixed(interned(local)?)
            }
            None => QualifiedAttributeName::unqualified(interned(ordinary_local)?),
        };
        let candidate = ParserCreatedAttribute::new(name, resolve_attribute_value(attr, text)?);
        if adjusted.iter().any(|existing: &ParserCreatedAttribute| {
            existing.name().same_expanded_name(candidate.name())
        }) {
            collisions = collisions.saturating_add(1);
            continue;
        }
        adjusted.push(candidate);
    }
    Ok(AdjustedAttributes {
        attributes: adjusted,
        post_adjustment_collisions: collisions,
    })
}

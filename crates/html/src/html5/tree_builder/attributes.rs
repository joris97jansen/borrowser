use crate::html5::shared::{AtomId, AtomTable, Attribute};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::formatting::AfeAttributeSnapshot;
use crate::html5::tree_builder::resolve::{resolve_atom_arc, resolve_attribute_value};
use std::sync::Arc;

pub(in crate::html5::tree_builder) type ParserCreatedAttribute = (Arc<str>, Option<String>);
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
        attributes.push((
            resolve_atom_arc(atoms, attr.name)?,
            resolve_attribute_value(attr, text)?,
        ));
    }
    Ok(attributes)
}

pub(in crate::html5::tree_builder) fn snapshot_token_attributes_first_wins(
    attrs: &[Attribute],
    text: &dyn TextResolver,
) -> Result<Vec<AfeAttributeSnapshot>, TreeBuilderError> {
    let mut seen = Vec::new();
    let mut snapshot = Vec::with_capacity(attrs.len());
    for attr in attrs {
        if seen.contains(&attr.name) {
            continue;
        }
        seen.push(attr.name);
        snapshot.push(AfeAttributeSnapshot::new(
            attr.name,
            resolve_attribute_value(attr, text)?,
        ));
    }
    Ok(snapshot)
}

pub(in crate::html5::tree_builder) fn resolve_afe_attributes_first_wins(
    attrs: &[AfeAttributeSnapshot],
    atoms: &AtomTable,
) -> Result<ParserCreatedAttributes, TreeBuilderError> {
    let mut seen: Vec<AtomId> = Vec::new();
    let mut attributes = Vec::with_capacity(attrs.len());
    for attr in attrs {
        if seen.contains(&attr.name) {
            continue;
        }
        seen.push(attr.name);
        attributes.push((resolve_atom_arc(atoms, attr.name)?, attr.value.clone()));
    }
    Ok(attributes)
}

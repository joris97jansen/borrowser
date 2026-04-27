use super::super::contract::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeImportance, CascadeSpecifiedValue,
    StylesheetDeclarationRef,
};
use crate::model::{self, PropertyNameKind};
use crate::{PropertyInvalidValuePolicy, property_registry};

pub(super) fn stylesheet_declaration_inputs(
    stylesheet_index: u32,
    rule_index: u32,
    declarations: &[model::Declaration],
) -> Vec<CascadeDeclarationInput> {
    declarations
        .iter()
        .enumerate()
        .map(|(declaration_index, declaration)| {
            let declaration_index = u32_index(declaration_index);
            declaration_input_from_model(
                CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
                    stylesheet_index,
                    rule_index,
                    declaration_index,
                }),
                declaration_index,
                declaration,
            )
        })
        .collect()
}

pub(super) fn declaration_input_from_model(
    source: CascadeDeclarationSource,
    declaration_order: u32,
    declaration: &model::Declaration,
) -> CascadeDeclarationInput {
    let importance = if declaration.important.is_some() {
        CascadeImportance::Important
    } else {
        CascadeImportance::Normal
    };

    match declaration.name.kind {
        PropertyNameKind::Standard => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                );
            };

            if let Some(property) = property_registry().lookup_id(property_name) {
                match CascadeSpecifiedValue::parse(property, &declaration.value) {
                    Ok(value) => CascadeDeclarationInput::supported(
                        source,
                        declaration_order,
                        importance,
                        property,
                        value,
                    ),
                    Err(error) => {
                        // Current supported properties only define strict
                        // declaration rejection. Keep policy dispatch here so
                        // any future invalid-value policy is added at the
                        // property/cascade boundary, not as computed-style or
                        // layout recovery.
                        match property.metadata().invalid_value_policy {
                            PropertyInvalidValuePolicy::RejectDeclaration => {
                                CascadeDeclarationInput::invalid_value(
                                    source,
                                    declaration_order,
                                    importance,
                                    property,
                                    error,
                                    CascadeSpecifiedValue::preserved(&declaration.value),
                                )
                            }
                        }
                    }
                }
            } else {
                CascadeDeclarationInput::unsupported_property(
                    source,
                    declaration_order,
                    importance,
                    property_name,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                )
            }
        }
        PropertyNameKind::Custom => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                );
            };

            CascadeDeclarationInput::custom_property(
                source,
                declaration_order,
                importance,
                property_name,
                CascadeSpecifiedValue::preserved(&declaration.value),
            )
        }
        PropertyNameKind::Invalid => CascadeDeclarationInput::invalid_property_name(
            source,
            declaration_order,
            importance,
            CascadeSpecifiedValue::preserved(&declaration.value),
        ),
    }
}

pub(super) fn u32_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

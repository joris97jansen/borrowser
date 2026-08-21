use super::super::contract::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeImportance, CascadeSpecifiedValue,
    InlineStyleDeclarationRef, InlineStyleRuleRef,
};
use super::super::contract::{DeclarationOrder, DeclarationSourceIndex, SourceCoordinateError};
use crate::model::{self, PropertyNameKind};
use crate::specified::{ShorthandExpansionError, ShorthandExpansionErrorKind};
use crate::{
    PropertyId, PropertyInvalidValuePolicy, ShorthandId, expand_shorthand_declaration,
    property_registry, shorthand_registry,
};

#[cfg(test)]
thread_local! {
    static DECLARATION_CLASSIFICATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_declaration_classification_count() {
    DECLARATION_CLASSIFICATION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn declaration_classification_count() -> usize {
    DECLARATION_CLASSIFICATION_COUNT.get()
}

pub(super) fn inline_style_declaration_inputs_from_model(
    inline_style: InlineStyleRuleRef,
    declarations: &[model::Declaration],
) -> Result<Vec<CascadeDeclarationInput>, SourceCoordinateError> {
    let mut inputs = Vec::new();
    for (index, declaration) in declarations.iter().enumerate() {
        let declaration_index = DeclarationSourceIndex::from_usize(index)?;
        let declaration_order = DeclarationOrder::from_usize(index)?;
        inputs.extend(declaration_inputs_from_model(
            CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef::new(
                inline_style,
                declaration_index,
            )),
            declaration_order,
            declaration,
        ));
    }
    Ok(inputs)
}

pub(super) fn declaration_inputs_from_model(
    source: CascadeDeclarationSource,
    declaration_order: DeclarationOrder,
    declaration: &model::Declaration,
) -> Vec<CascadeDeclarationInput> {
    #[cfg(test)]
    DECLARATION_CLASSIFICATION_COUNT.with(|count| count.set(count.get() + 1));
    let importance = if declaration.important.is_some() {
        CascadeImportance::Important
    } else {
        CascadeImportance::Normal
    };

    match declaration.name.kind {
        PropertyNameKind::Standard => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return vec![CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                )];
            };

            if let Some(property) = property_registry().lookup_id(property_name) {
                vec![longhand_declaration_input(
                    source,
                    declaration_order,
                    importance,
                    property,
                    &declaration.value,
                )]
            } else if let Some(shorthand) = shorthand_registry().lookup_id(property_name) {
                shorthand_declaration_inputs(
                    source,
                    declaration_order,
                    importance,
                    shorthand,
                    &declaration.value,
                )
            } else {
                vec![CascadeDeclarationInput::unsupported_property(
                    source,
                    declaration_order,
                    importance,
                    property_name,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                )]
            }
        }
        PropertyNameKind::Custom => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return vec![CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                )];
            };

            vec![CascadeDeclarationInput::custom_property(
                source,
                declaration_order,
                importance,
                property_name,
                CascadeSpecifiedValue::preserved(&declaration.value),
            )]
        }
        PropertyNameKind::Invalid => vec![CascadeDeclarationInput::invalid_property_name(
            source,
            declaration_order,
            importance,
            CascadeSpecifiedValue::preserved(&declaration.value),
        )],
    }
}

fn longhand_declaration_input(
    source: CascadeDeclarationSource,
    declaration_order: DeclarationOrder,
    importance: CascadeImportance,
    property: PropertyId,
    value: &model::DeclarationValue,
) -> CascadeDeclarationInput {
    match CascadeSpecifiedValue::parse(property, value) {
        Ok(value) => CascadeDeclarationInput::supported(
            source,
            declaration_order,
            importance,
            property,
            value,
        ),
        Err(error) => {
            // Current supported properties only define strict declaration
            // rejection. Keep policy dispatch here so any future invalid-value
            // policy is added at the property/cascade boundary, not as
            // computed-style or layout recovery.
            match property.metadata().invalid_value_policy {
                PropertyInvalidValuePolicy::RejectDeclaration => {
                    CascadeDeclarationInput::invalid_value(
                        source,
                        declaration_order,
                        importance,
                        property,
                        error,
                        CascadeSpecifiedValue::preserved(value),
                    )
                }
            }
        }
    }
}

fn shorthand_declaration_inputs(
    source: CascadeDeclarationSource,
    declaration_order: DeclarationOrder,
    importance: CascadeImportance,
    shorthand: ShorthandId,
    value: &model::DeclarationValue,
) -> Vec<CascadeDeclarationInput> {
    let expansion = match expand_shorthand_declaration(shorthand, value) {
        Ok(expansion) => expansion,
        Err(error) => {
            return vec![CascadeDeclarationInput::invalid_shorthand_value(
                source,
                declaration_order,
                importance,
                shorthand,
                error,
                CascadeSpecifiedValue::preserved(value),
            )];
        }
    };

    let mut parsed_longhands = Vec::new();
    for expanded in expansion.longhands() {
        match CascadeSpecifiedValue::parse(expanded.property(), expanded.value()) {
            Ok(value) => {
                parsed_longhands.push((expanded.property(), expanded.expansion_order(), value))
            }
            Err(error) => {
                let shorthand_error = ShorthandExpansionError::new(
                    shorthand,
                    ShorthandExpansionErrorKind::LonghandValueRejected {
                        property: expanded.property(),
                        kind: error.kind(),
                    },
                );
                return vec![CascadeDeclarationInput::invalid_shorthand_value(
                    source,
                    declaration_order,
                    importance,
                    shorthand,
                    shorthand_error,
                    CascadeSpecifiedValue::preserved(value),
                )];
            }
        }
    }

    parsed_longhands
        .into_iter()
        .map(|(property, expansion_order, parsed_value)| {
            CascadeDeclarationInput::supported_with_expansion_order(
                source,
                declaration_order,
                expansion_order,
                importance,
                property,
                parsed_value,
            )
        })
        .collect()
}

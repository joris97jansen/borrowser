use crate::{
    model::{DeclarationValue, ValueComponent, ValueText, ValueToken},
    properties::{PropertyId, ShorthandId},
    values::CssWideKeyword,
};

use super::{
    SpecifiedValueLimits, error::SpecifiedValueParseErrorKind, parse::parse_specified_value,
};

const DEFAULT_EXPANSION_ORDER: u16 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShorthandExpansion {
    shorthand: ShorthandId,
    longhands: Vec<ExpandedLonghandDeclaration>,
}

impl ShorthandExpansion {
    fn new(shorthand: ShorthandId, longhands: Vec<ExpandedLonghandDeclaration>) -> Self {
        Self {
            shorthand,
            longhands,
        }
    }

    pub fn shorthand(&self) -> ShorthandId {
        self.shorthand
    }

    pub fn longhands(&self) -> &[ExpandedLonghandDeclaration] {
        &self.longhands
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpandedLonghandDeclaration {
    property: PropertyId,
    value: DeclarationValue,
    expansion_order: u16,
}

impl ExpandedLonghandDeclaration {
    fn new(property: PropertyId, value: DeclarationValue, expansion_order: u16) -> Self {
        Self {
            property,
            value,
            expansion_order,
        }
    }

    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn value(&self) -> &DeclarationValue {
        &self.value
    }

    pub fn expansion_order(&self) -> u16 {
        self.expansion_order
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShorthandExpansionError {
    shorthand: ShorthandId,
    kind: ShorthandExpansionErrorKind,
}

impl ShorthandExpansionError {
    pub(crate) fn new(shorthand: ShorthandId, kind: ShorthandExpansionErrorKind) -> Self {
        Self { shorthand, kind }
    }

    pub fn shorthand(&self) -> ShorthandId {
        self.shorthand
    }

    pub fn kind(&self) -> &ShorthandExpansionErrorKind {
        &self.kind
    }
}

impl std::fmt::Display for ShorthandExpansionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "shorthand '{}' value rejected: {}",
            self.shorthand.name(),
            self.kind.as_debug_label()
        )
    }
}

impl std::error::Error for ShorthandExpansionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShorthandExpansionErrorKind {
    ResourceLimitExceeded,
    EmptyValue,
    UnsupportedComponent,
    DuplicateComponent {
        property: PropertyId,
    },
    AmbiguousComponent,
    UnsupportedCssWideKeyword,
    LonghandValueRejected {
        property: PropertyId,
        kind: SpecifiedValueParseErrorKind,
    },
}

impl ShorthandExpansionErrorKind {
    pub fn as_debug_label(&self) -> &'static str {
        match self {
            Self::ResourceLimitExceeded => "resource-limit-exceeded",
            Self::EmptyValue => "empty-value",
            Self::UnsupportedComponent => "unsupported-component",
            Self::DuplicateComponent { .. } => "duplicate-component",
            Self::AmbiguousComponent => "ambiguous-component",
            Self::UnsupportedCssWideKeyword => "unsupported-css-wide-keyword",
            Self::LonghandValueRejected { .. } => "longhand-value-rejected",
        }
    }
}

pub fn expand_shorthand_declaration(
    shorthand: ShorthandId,
    value: &DeclarationValue,
) -> Result<ShorthandExpansion, ShorthandExpansionError> {
    match shorthand {
        ShorthandId::Outline => expand_outline(value),
    }
}

fn expand_outline(value: &DeclarationValue) -> Result<ShorthandExpansion, ShorthandExpansionError> {
    let components = non_trivia_components(ShorthandId::Outline, value)?;
    if let Some(css_wide) = css_wide_shorthand_value(&components)? {
        return Ok(outline_expansion(
            value,
            css_wide.clone(),
            css_wide.clone(),
            css_wide,
        ));
    }

    let mut outline = OutlineExpansionParts::default();
    for component in components {
        let slot = classify_outline_component(component)?;
        outline.record(slot, component_value(component))?;
    }

    Ok(outline_expansion(
        value,
        outline
            .color
            .unwrap_or_else(|| initial_reset_value(value.span())),
        outline
            .style
            .unwrap_or_else(|| initial_reset_value(value.span())),
        outline
            .width
            .unwrap_or_else(|| initial_reset_value(value.span())),
    ))
}

fn outline_expansion(
    value: &DeclarationValue,
    color: DeclarationValue,
    style: DeclarationValue,
    width: DeclarationValue,
) -> ShorthandExpansion {
    let _ = value;
    ShorthandExpansion::new(
        ShorthandId::Outline,
        vec![
            ExpandedLonghandDeclaration::new(
                PropertyId::OutlineColor,
                color,
                DEFAULT_EXPANSION_ORDER,
            ),
            ExpandedLonghandDeclaration::new(PropertyId::OutlineStyle, style, 1),
            ExpandedLonghandDeclaration::new(PropertyId::OutlineWidth, width, 2),
        ],
    )
}

#[derive(Default)]
struct OutlineExpansionParts {
    color: Option<DeclarationValue>,
    style: Option<DeclarationValue>,
    width: Option<DeclarationValue>,
}

impl OutlineExpansionParts {
    fn record(
        &mut self,
        slot: OutlineExpansionSlot,
        value: DeclarationValue,
    ) -> Result<(), ShorthandExpansionError> {
        let target = match slot {
            OutlineExpansionSlot::Color => &mut self.color,
            OutlineExpansionSlot::Style => &mut self.style,
            OutlineExpansionSlot::Width => &mut self.width,
        };
        if target.is_some() {
            return Err(ShorthandExpansionError::new(
                ShorthandId::Outline,
                ShorthandExpansionErrorKind::DuplicateComponent {
                    property: slot.property(),
                },
            ));
        }

        *target = Some(value);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutlineExpansionSlot {
    Color,
    Style,
    Width,
}

impl OutlineExpansionSlot {
    fn property(self) -> PropertyId {
        match self {
            Self::Color => PropertyId::OutlineColor,
            Self::Style => PropertyId::OutlineStyle,
            Self::Width => PropertyId::OutlineWidth,
        }
    }
}

fn classify_outline_component(
    component: &ValueComponent,
) -> Result<OutlineExpansionSlot, ShorthandExpansionError> {
    let candidates = [
        (OutlineExpansionSlot::Color, PropertyId::OutlineColor),
        (OutlineExpansionSlot::Style, PropertyId::OutlineStyle),
        (OutlineExpansionSlot::Width, PropertyId::OutlineWidth),
    ];
    let value = component_value(component);
    let mut matches = Vec::new();
    let mut errors = Vec::new();

    for (slot, property) in candidates {
        match parse_specified_value(property, &value) {
            Ok(_) => matches.push(slot),
            Err(error) => errors.push((property, error.kind())),
        }
    }

    match matches.as_slice() {
        [slot] => Ok(*slot),
        [] => Err(shorthand_component_error(component, &errors)),
        _ => Err(ShorthandExpansionError::new(
            ShorthandId::Outline,
            ShorthandExpansionErrorKind::AmbiguousComponent,
        )),
    }
}

fn shorthand_component_error(
    component: &ValueComponent,
    errors: &[(PropertyId, SpecifiedValueParseErrorKind)],
) -> ShorthandExpansionError {
    let preferred = match component {
        ValueComponent::Token(ValueToken::Hash { .. }) => PropertyId::OutlineColor,
        ValueComponent::Token(ValueToken::Dimension { .. } | ValueToken::Number { .. }) => {
            PropertyId::OutlineWidth
        }
        ValueComponent::Function(_) => PropertyId::OutlineColor,
        _ => {
            return ShorthandExpansionError::new(
                ShorthandId::Outline,
                ShorthandExpansionErrorKind::UnsupportedComponent,
            );
        }
    };

    if let Some((property, kind)) = errors
        .iter()
        .find(|(property, _)| *property == preferred)
        .copied()
    {
        return ShorthandExpansionError::new(
            ShorthandId::Outline,
            ShorthandExpansionErrorKind::LonghandValueRejected { property, kind },
        );
    }

    ShorthandExpansionError::new(
        ShorthandId::Outline,
        ShorthandExpansionErrorKind::UnsupportedComponent,
    )
}

fn non_trivia_components(
    shorthand: ShorthandId,
    value: &DeclarationValue,
) -> Result<Vec<&ValueComponent>, ShorthandExpansionError> {
    let limits = SpecifiedValueLimits::default();
    if value.components.len() > limits.max_components_per_value {
        return Err(ShorthandExpansionError::new(
            shorthand,
            ShorthandExpansionErrorKind::ResourceLimitExceeded,
        ));
    }

    let components = value
        .components
        .iter()
        .filter(|component| !is_trivia(component))
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Err(ShorthandExpansionError::new(
            shorthand,
            ShorthandExpansionErrorKind::EmptyValue,
        ));
    }

    Ok(components)
}

fn css_wide_shorthand_value(
    components: &[&ValueComponent],
) -> Result<Option<DeclarationValue>, ShorthandExpansionError> {
    let [component] = components else {
        return Ok(None);
    };
    let ValueComponent::Token(ValueToken::Ident { span, text }) = component else {
        return Ok(None);
    };
    let Some(text) = text.text.as_deref() else {
        return Ok(None);
    };
    let Some(keyword) = CssWideKeyword::from_canonical(&text.to_ascii_lowercase()) else {
        return Ok(None);
    };

    if !keyword.is_supported_for_current_cascade() {
        return Err(ShorthandExpansionError::new(
            ShorthandId::Outline,
            ShorthandExpansionErrorKind::UnsupportedCssWideKeyword,
        ));
    }

    Ok(Some(DeclarationValue {
        span: *span,
        components: vec![(*component).clone()],
    }))
}

fn component_value(component: &ValueComponent) -> DeclarationValue {
    DeclarationValue {
        span: component.span(),
        components: vec![component.clone()],
    }
}

fn initial_reset_value(span: crate::syntax::CssSpan) -> DeclarationValue {
    DeclarationValue {
        span,
        components: vec![ValueComponent::Token(ValueToken::Ident {
            span,
            text: ValueText {
                span: Some(span),
                text: Some("initial".to_string()),
            },
        })],
    }
}

fn is_trivia(component: &ValueComponent) -> bool {
    matches!(
        component,
        ValueComponent::Token(ValueToken::Whitespace { .. } | ValueToken::Comment { .. })
    )
}

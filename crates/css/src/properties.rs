//! Engine-owned CSS property system contract for the current supported subset.
//!
//! This module is the shared property table for the cascade and computed-style
//! layers. It owns:
//! - the supported property identifier universe
//! - the registry of supported properties and their canonical CSS names
//! - inheritance and initial/default metadata
//! - the boundary between typed specified-value parsing and typed computed
//!   values
//! - property-owned value-range metadata for specified-value validation
//! - the current-scope invalid-value handling rule
//!
//! `PropertyId` is the stable identity for one supported property.
//! `PropertyId::metadata()` is the normative source for inheritance,
//! initial/default, specified-value-shape, computed-value-shape, and
//! invalid-value and value-range facts. Downstream code must not re-encode
//! those facts in separate match tables.
//!
//! This module deliberately does not own cascade precedence, selector
//! matching, property-specific parsers, or layout-facing interpretation.

/// Engine-owned identifier for one supported CSS property.
///
/// Ordering is canonical and stable. Both cascade and computed-style assembly
/// rely on `ALL` remaining deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum PropertyId {
    BackgroundColor,
    Color,
    Display,
    FontSize,
    Height,
    MarginBottom,
    MarginLeft,
    MarginRight,
    MarginTop,
    MaxWidth,
    MinWidth,
    PaddingBottom,
    PaddingLeft,
    PaddingRight,
    PaddingTop,
    Width,
}

impl PropertyId {
    pub const ALL: [Self; 16] = [
        Self::BackgroundColor,
        Self::Color,
        Self::Display,
        Self::FontSize,
        Self::Height,
        Self::MarginBottom,
        Self::MarginLeft,
        Self::MarginRight,
        Self::MarginTop,
        Self::MaxWidth,
        Self::MinWidth,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::PaddingRight,
        Self::PaddingTop,
        Self::Width,
    ];

    pub const fn as_index(self) -> usize {
        match self {
            Self::BackgroundColor => 0,
            Self::Color => 1,
            Self::Display => 2,
            Self::FontSize => 3,
            Self::Height => 4,
            Self::MarginBottom => 5,
            Self::MarginLeft => 6,
            Self::MarginRight => 7,
            Self::MarginTop => 8,
            Self::MaxWidth => 9,
            Self::MinWidth => 10,
            Self::PaddingBottom => 11,
            Self::PaddingLeft => 12,
            Self::PaddingRight => 13,
            Self::PaddingTop => 14,
            Self::Width => 15,
        }
    }

    pub fn name(self) -> &'static str {
        property_registry().get(self).name()
    }

    /// Maps a canonical property name from the model layer into the supported
    /// property subset.
    pub fn from_name(name: &str) -> Option<Self> {
        property_registry().lookup_id(name)
    }

    /// Returns the normative shared metadata for this property.
    ///
    /// Contributors should extend the registry rather than restating
    /// inheritance/default/value-kind facts in downstream subsystems.
    pub fn metadata(self) -> PropertyMetadata {
        property_registry().get(self).metadata()
    }

    /// Returns the cascade-owned initial/default value for this property.
    ///
    /// Cascade owns source selection for initial/default fill. The computed
    /// layer later interprets the chosen initial/default token into a typed
    /// computed value and must not invent missing-property defaults
    /// independently.
    pub fn initial_value(self) -> InitialStyleValue {
        self.metadata().initial
    }
}

/// One registry entry describing a supported property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyRegistration {
    id: PropertyId,
    name: &'static str,
    metadata: PropertyMetadata,
}

impl PropertyRegistration {
    pub const fn new(id: PropertyId, name: &'static str, metadata: PropertyMetadata) -> Self {
        Self { id, name, metadata }
    }

    pub fn id(&self) -> PropertyId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn metadata(&self) -> PropertyMetadata {
        self.metadata
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PropertyNameLookupEntry {
    name: &'static str,
    id: PropertyId,
}

impl PropertyNameLookupEntry {
    const fn new(name: &'static str, id: PropertyId) -> Self {
        Self { name, id }
    }
}

/// Deterministic registry for Borrowser's supported property subset.
///
/// Entries are stored in canonical property order. Identifier lookup
/// intentionally depends on `PropertyId::as_index()` matching the registry
/// entry order. Name lookup uses a separately indexed canonical-name table, so
/// binary-search behavior does not depend on the canonical entry sequence.
#[derive(Clone, Copy, Debug)]
pub struct PropertyRegistry {
    entries: &'static [PropertyRegistration],
    lookup_by_name: &'static [PropertyNameLookupEntry],
}

impl PropertyRegistry {
    const fn new(
        entries: &'static [PropertyRegistration],
        lookup_by_name: &'static [PropertyNameLookupEntry],
    ) -> Self {
        Self {
            entries,
            lookup_by_name,
        }
    }

    /// Returns supported properties in canonical property order.
    pub fn entries(&self) -> &'static [PropertyRegistration] {
        self.entries
    }

    /// Iterates property identifiers in canonical property order.
    pub fn ids(&self) -> impl Iterator<Item = PropertyId> + '_ {
        self.entries.iter().map(PropertyRegistration::id)
    }

    /// Returns the registry entry for one supported property identifier.
    pub fn get(&self, id: PropertyId) -> &'static PropertyRegistration {
        let registration = &self.entries[id.as_index()];
        debug_assert_eq!(
            registration.id(),
            id,
            "property registry entry order must align with PropertyId::as_index()"
        );
        registration
    }

    /// Resolves a canonical parsed property name into a supported registry
    /// entry.
    ///
    /// Lookup is deterministic and exact over canonical lowercase CSS property
    /// names. Case folding belongs upstream in the model layer.
    pub fn lookup(&self, name: &str) -> Option<&'static PropertyRegistration> {
        let lookup_index = self
            .lookup_by_name
            .binary_search_by_key(&name, |entry| entry.name)
            .ok()?;

        Some(self.get(self.lookup_by_name[lookup_index].id))
    }

    /// Resolves a canonical parsed property name directly to its property id.
    pub fn lookup_id(&self, name: &str) -> Option<PropertyId> {
        self.lookup(name).map(|entry| entry.id())
    }
}

/// Returns the shared supported-property registry.
pub fn property_registry() -> &'static PropertyRegistry {
    &PROPERTY_REGISTRY
}

static PROPERTY_REGISTRY: PropertyRegistry =
    PropertyRegistry::new(&PROPERTY_REGISTRATION_DATA, &PROPERTY_LOOKUP_BY_NAME);

const PROPERTY_REGISTRATION_DATA: [PropertyRegistration; 16] = [
    PropertyRegistration::new(
        PropertyId::BackgroundColor,
        "background-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Color,
        "color",
        PropertyMetadata::inherited(
            InitialStyleValue::ColorBlack,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Display,
        "display",
        PropertyMetadata::not_inherited(
            InitialStyleValue::DisplayInline,
            PropertySpecifiedValueKind::DisplayKeyword,
            PropertyComputedValueKind::DisplayKeyword,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::FontSize,
        "font-size",
        PropertyMetadata::inherited(
            InitialStyleValue::FontSizePx16,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Height,
        "height",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            PropertyComputedValueKind::AbsoluteLengthOrAuto,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::MarginBottom,
        "margin-bottom",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MarginLeft,
        "margin-left",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MarginRight,
        "margin-right",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MarginTop,
        "margin-top",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MaxWidth,
        "max-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::NoneKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrNone,
            PropertyComputedValueKind::AbsoluteLengthOrNone,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::MinWidth,
        "min-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            PropertyComputedValueKind::AbsoluteLengthOrAuto,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingBottom,
        "padding-bottom",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingLeft,
        "padding-left",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingRight,
        "padding-right",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingTop,
        "padding-top",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Width,
        "width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            PropertyComputedValueKind::AbsoluteLengthOrAuto,
        ),
    ),
];

const PROPERTY_LOOKUP_BY_NAME: [PropertyNameLookupEntry; 16] = [
    PropertyNameLookupEntry::new("background-color", PropertyId::BackgroundColor),
    PropertyNameLookupEntry::new("color", PropertyId::Color),
    PropertyNameLookupEntry::new("display", PropertyId::Display),
    PropertyNameLookupEntry::new("font-size", PropertyId::FontSize),
    PropertyNameLookupEntry::new("height", PropertyId::Height),
    PropertyNameLookupEntry::new("margin-bottom", PropertyId::MarginBottom),
    PropertyNameLookupEntry::new("margin-left", PropertyId::MarginLeft),
    PropertyNameLookupEntry::new("margin-right", PropertyId::MarginRight),
    PropertyNameLookupEntry::new("margin-top", PropertyId::MarginTop),
    PropertyNameLookupEntry::new("max-width", PropertyId::MaxWidth),
    PropertyNameLookupEntry::new("min-width", PropertyId::MinWidth),
    PropertyNameLookupEntry::new("padding-bottom", PropertyId::PaddingBottom),
    PropertyNameLookupEntry::new("padding-left", PropertyId::PaddingLeft),
    PropertyNameLookupEntry::new("padding-right", PropertyId::PaddingRight),
    PropertyNameLookupEntry::new("padding-top", PropertyId::PaddingTop),
    PropertyNameLookupEntry::new("width", PropertyId::Width),
];

/// Shared property metadata consumed by cascade and computed-style code.
///
/// `PropertyId` is the stable identity; `PropertyMetadata` is the normative
/// fact table attached to that identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyMetadata {
    pub inheritance: PropertyInheritance,
    pub initial: InitialStyleValue,
    pub specified_value: PropertySpecifiedValueKind,
    pub computed_value: PropertyComputedValueKind,
    pub invalid_value_policy: PropertyInvalidValuePolicy,
    pub length_sign: PropertyLengthSignPolicy,
}

impl PropertyMetadata {
    pub const fn inherited(
        initial: InitialStyleValue,
        specified_value: PropertySpecifiedValueKind,
        computed_value: PropertyComputedValueKind,
    ) -> Self {
        Self {
            inheritance: PropertyInheritance::Inherited,
            initial,
            specified_value,
            computed_value,
            invalid_value_policy: PropertyInvalidValuePolicy::RejectDeclaration,
            length_sign: default_length_sign_policy(specified_value),
        }
    }

    pub const fn not_inherited(
        initial: InitialStyleValue,
        specified_value: PropertySpecifiedValueKind,
        computed_value: PropertyComputedValueKind,
    ) -> Self {
        Self {
            inheritance: PropertyInheritance::NotInherited,
            initial,
            specified_value,
            computed_value,
            invalid_value_policy: PropertyInvalidValuePolicy::RejectDeclaration,
            length_sign: default_length_sign_policy(specified_value),
        }
    }

    pub const fn with_length_sign(mut self, length_sign: PropertyLengthSignPolicy) -> Self {
        self.length_sign = length_sign;
        self
    }
}

const fn default_length_sign_policy(
    specified_value: PropertySpecifiedValueKind,
) -> PropertyLengthSignPolicy {
    match specified_value {
        PropertySpecifiedValueKind::Color | PropertySpecifiedValueKind::DisplayKeyword => {
            PropertyLengthSignPolicy::NotLength
        }
        PropertySpecifiedValueKind::AbsoluteLength
        | PropertySpecifiedValueKind::AbsoluteLengthOrAuto
        | PropertySpecifiedValueKind::AbsoluteLengthOrNone => PropertyLengthSignPolicy::NonNegative,
    }
}

/// Whether a property inherits when no local winning declaration exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInheritance {
    Inherited,
    NotInherited,
}

/// Typed specified-value shape the property parser is expected to emit.
///
/// The current supported subset keeps specified-value parsing layout
/// independent. Relative units, percentages, and other layout-dependent forms
/// remain out of scope until a later milestone extends the value model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertySpecifiedValueKind {
    Color,
    DisplayKeyword,
    AbsoluteLength,
    AbsoluteLengthOrAuto,
    AbsoluteLengthOrNone,
}

/// Typed computed-value shape exposed to runtime consumers through
/// `ComputedStyle`.
///
/// "Absolute" here means normalized to the engine's current CSS-px-only
/// contract for the supported subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyComputedValueKind {
    AbsoluteColor,
    DisplayKeyword,
    AbsoluteLength,
    AbsoluteLengthOrAuto,
    AbsoluteLengthOrNone,
}

/// Invalid-value handling rule for the current supported subset.
///
/// Current policy is intentionally strict: if a declaration cannot be parsed
/// into the property's specified-value representation, the declaration is
/// rejected at the property pipeline layer. Layout, painting, and other
/// runtime consumers must not attempt post-hoc recovery for supported
/// properties. The cascade then falls back to another winner, inheritance, or
/// the initial/default contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInvalidValuePolicy {
    RejectDeclaration,
}

/// Sign policy for length branches accepted by a supported property.
///
/// This lives in property metadata so specified-value parsers do not keep a
/// second property rule table for value-range behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyLengthSignPolicy {
    NotLength,
    AllowNegative,
    NonNegative,
}

/// Cascade-owned initial/default values for the current property subset.
///
/// These are not typed computed values. The computed-value layer remains
/// responsible for converting these tokens into normalized runtime data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitialStyleValue {
    ColorBlack,
    TransparentColor,
    DisplayInline,
    FontSizePx16,
    ZeroPx,
    AutoKeyword,
    NoneKeyword,
}

impl InitialStyleValue {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::ColorBlack => "black",
            Self::TransparentColor => "transparent",
            Self::DisplayInline => "inline",
            Self::FontSizePx16 => "16px",
            Self::ZeroPx => "0px",
            Self::AutoKeyword => "auto",
            Self::NoneKeyword => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InitialStyleValue, PROPERTY_LOOKUP_BY_NAME, PropertyComputedValueKind, PropertyId,
        PropertyInheritance, PropertyInvalidValuePolicy, PropertyLengthSignPolicy,
        PropertySpecifiedValueKind, property_registry,
    };

    #[test]
    fn property_registry_entries_are_total_canonical_and_metadata_backed() {
        let expected = [
            (
                PropertyId::BackgroundColor,
                PropertyInheritance::NotInherited,
                InitialStyleValue::TransparentColor,
                PropertySpecifiedValueKind::Color,
                PropertyComputedValueKind::AbsoluteColor,
                PropertyLengthSignPolicy::NotLength,
            ),
            (
                PropertyId::Color,
                PropertyInheritance::Inherited,
                InitialStyleValue::ColorBlack,
                PropertySpecifiedValueKind::Color,
                PropertyComputedValueKind::AbsoluteColor,
                PropertyLengthSignPolicy::NotLength,
            ),
            (
                PropertyId::Display,
                PropertyInheritance::NotInherited,
                InitialStyleValue::DisplayInline,
                PropertySpecifiedValueKind::DisplayKeyword,
                PropertyComputedValueKind::DisplayKeyword,
                PropertyLengthSignPolicy::NotLength,
            ),
            (
                PropertyId::FontSize,
                PropertyInheritance::Inherited,
                InitialStyleValue::FontSizePx16,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::Height,
                PropertyInheritance::NotInherited,
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::MarginBottom,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::AllowNegative,
            ),
            (
                PropertyId::MarginLeft,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::AllowNegative,
            ),
            (
                PropertyId::MarginRight,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::AllowNegative,
            ),
            (
                PropertyId::MarginTop,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::AllowNegative,
            ),
            (
                PropertyId::MaxWidth,
                PropertyInheritance::NotInherited,
                InitialStyleValue::NoneKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrNone,
                PropertyComputedValueKind::AbsoluteLengthOrNone,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::MinWidth,
                PropertyInheritance::NotInherited,
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::PaddingBottom,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::PaddingLeft,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::PaddingRight,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::PaddingTop,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
                PropertyLengthSignPolicy::NonNegative,
            ),
            (
                PropertyId::Width,
                PropertyInheritance::NotInherited,
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
                PropertyLengthSignPolicy::NonNegative,
            ),
        ];

        let registry = property_registry();
        assert_eq!(registry.entries().len(), expected.len());
        assert_eq!(PropertyId::ALL.len(), expected.len());

        for (
            index,
            (property, inheritance, initial, specified_value, computed_value, length_sign),
        ) in expected.into_iter().enumerate()
        {
            assert_eq!(PropertyId::ALL[index], property);

            let registration = &registry.entries()[index];
            assert_eq!(registration.id(), property);
            assert_eq!(registration.name(), property.name());
            assert_eq!(PropertyId::from_name(property.name()), Some(property));
            assert_eq!(registry.lookup_id(property.name()), Some(property));

            let metadata = registration.metadata();
            assert_eq!(metadata.inheritance, inheritance, "{}", property.name());
            assert_eq!(metadata.initial, initial, "{}", property.name());
            assert_eq!(
                metadata.specified_value,
                specified_value,
                "{}",
                property.name()
            );
            assert_eq!(
                metadata.computed_value,
                computed_value,
                "{}",
                property.name()
            );
            assert_eq!(
                metadata.invalid_value_policy,
                PropertyInvalidValuePolicy::RejectDeclaration,
                "{}",
                property.name()
            );
            assert_eq!(metadata.length_sign, length_sign, "{}", property.name());
            assert_eq!(property.initial_value(), initial, "{}", property.name());
        }
    }

    #[test]
    fn property_registry_lookup_is_deterministic_for_representative_property_names() {
        let registry = property_registry();

        assert_eq!(
            registry.lookup("background-color").map(|entry| entry.id()),
            Some(PropertyId::BackgroundColor)
        );
        assert_eq!(
            registry.lookup("font-size").map(|entry| entry.id()),
            Some(PropertyId::FontSize)
        );
        assert_eq!(
            registry.lookup("padding-left").map(|entry| entry.id()),
            Some(PropertyId::PaddingLeft)
        );
        assert_eq!(
            registry.lookup("width").map(|entry| entry.id()),
            Some(PropertyId::Width)
        );
        assert_eq!(registry.lookup("zoom"), None);
        assert_eq!(registry.lookup("COLOR"), None);
    }

    #[test]
    fn property_lookup_table_is_sorted_for_binary_search() {
        let names = PROPERTY_LOOKUP_BY_NAME
            .iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>();

        let mut sorted = names.clone();
        sorted.sort_unstable();

        assert_eq!(names, sorted);
    }

    #[test]
    fn property_registry_get_returns_registration_for_every_supported_id() {
        let registry = property_registry();

        for property in PropertyId::ALL {
            let registration = registry.get(property);
            assert_eq!(registration.id(), property);
            assert_eq!(registration.name(), property.name());
            assert_eq!(registration.metadata(), property.metadata());
        }
    }
}

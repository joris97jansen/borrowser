use super::{
    data::{PROPERTY_LOOKUP_BY_NAME, PROPERTY_REGISTRATION_DATA},
    shorthand_registry,
    types::{PropertyId, PropertyMetadata},
};
use std::fmt::Write;

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
pub(super) struct PropertyNameLookupEntry {
    pub(super) name: &'static str,
    pub(super) id: PropertyId,
}

impl PropertyNameLookupEntry {
    pub(super) const fn new(name: &'static str, id: PropertyId) -> Self {
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

/// Deterministic debug snapshot for the supported longhand registry metadata.
///
/// This is an internal regression surface. It intentionally follows registry
/// canonical order and does not expose map iteration order or parser internals.
pub fn property_registry_metadata_debug_snapshot() -> String {
    let registry = property_registry();
    let mut output = String::from("version: 1\nproperty-registry-metadata\n");
    writeln!(&mut output, "properties: {}", registry.entries().len()).expect("write snapshot");

    for (index, registration) in registry.entries().iter().enumerate() {
        let metadata = registration.metadata();
        writeln!(&mut output, "property[{index}]: {}", registration.name())
            .expect("write snapshot");
        writeln!(
            &mut output,
            "  inheritance: {}",
            metadata.inheritance.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  initial: {}",
            metadata.initial.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  specified-value: {}",
            metadata.specified_value.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  computed-value: {}",
            metadata.computed_value.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  invalid-value-policy: {}",
            metadata.invalid_value_policy.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  length-sign: {}",
            metadata.length_sign.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  invalidation-impact: {}",
            metadata.invalidation_impact.to_debug_label()
        )
        .expect("write snapshot");
    }

    output
}

/// Deterministic debug snapshot for current supported longhand coverage.
///
/// This is derived from the supported longhand registry and supported
/// shorthand registry. It is not a complete known-CSS-property inventory and
/// must not be extended with unsupported placeholder properties.
pub fn property_coverage_debug_snapshot() -> String {
    let registry = property_registry();
    let mut output = String::from("version: 1\nproperty-coverage\n");
    writeln!(&mut output, "properties: {}", registry.entries().len()).expect("write snapshot");

    for (index, registration) in registry.entries().iter().enumerate() {
        let metadata = registration.metadata();
        writeln!(&mut output, "property[{index}]: {}", registration.name())
            .expect("write snapshot");
        writeln!(&mut output, "  supported: yes").expect("write snapshot");
        writeln!(
            &mut output,
            "  inherited-by-default: {}",
            metadata.inheritance.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  initial: {}",
            metadata.initial.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  specified-value: {}",
            metadata.specified_value.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  computed-value: {}",
            metadata.computed_value.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  invalidation-impact: {}",
            metadata.invalidation_impact.to_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  shorthand-membership: {}",
            shorthand_membership_debug_label(registration.id())
        )
        .expect("write snapshot");
    }

    output
}

fn shorthand_membership_debug_label(property: PropertyId) -> String {
    let memberships = shorthand_registry()
        .entries()
        .iter()
        .filter_map(|shorthand| {
            shorthand
                .longhands()
                .iter()
                .position(|&longhand| longhand == property)
                .map(|index| format!("{}[{index}]", shorthand.name()))
        })
        .collect::<Vec<_>>();

    if memberships.is_empty() {
        "none".to_string()
    } else {
        memberships.join(", ")
    }
}

static PROPERTY_REGISTRY: PropertyRegistry =
    PropertyRegistry::new(&PROPERTY_REGISTRATION_DATA, &PROPERTY_LOOKUP_BY_NAME);

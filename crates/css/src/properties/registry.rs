use super::{
    data::{PROPERTY_LOOKUP_BY_NAME, PROPERTY_REGISTRATION_DATA},
    types::{PropertyId, PropertyMetadata},
};

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

static PROPERTY_REGISTRY: PropertyRegistry =
    PropertyRegistry::new(&PROPERTY_REGISTRATION_DATA, &PROPERTY_LOOKUP_BY_NAME);

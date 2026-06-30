use super::types::PropertyId;
use std::fmt::Write;

/// Engine-owned identifier for one supported CSS shorthand property.
///
/// Shorthands are intentionally separate from `PropertyId`: only longhands
/// enter the supported longhand registry, computed style, and invalidation
/// metadata. A shorthand may only contribute by expanding into registered
/// longhands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ShorthandId {
    Outline,
}

impl ShorthandId {
    pub const ALL: [Self; 1] = [Self::Outline];

    pub const fn as_index(self) -> usize {
        match self {
            Self::Outline => 0,
        }
    }

    pub fn name(self) -> &'static str {
        shorthand_registry().get(self).name()
    }

    pub fn from_name(name: &str) -> Option<Self> {
        shorthand_registry().lookup_id(name)
    }

    pub fn longhands(self) -> &'static [PropertyId] {
        shorthand_registry().get(self).longhands()
    }
}

/// One supported shorthand registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShorthandRegistration {
    id: ShorthandId,
    name: &'static str,
    longhands: &'static [PropertyId],
}

impl ShorthandRegistration {
    pub const fn new(
        id: ShorthandId,
        name: &'static str,
        longhands: &'static [PropertyId],
    ) -> Self {
        Self {
            id,
            name,
            longhands,
        }
    }

    pub fn id(&self) -> ShorthandId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn longhands(&self) -> &'static [PropertyId] {
        self.longhands
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShorthandNameLookupEntry {
    name: &'static str,
    id: ShorthandId,
}

impl ShorthandNameLookupEntry {
    const fn new(name: &'static str, id: ShorthandId) -> Self {
        Self { name, id }
    }
}

/// Deterministic registry for Borrowser's supported shorthand subset.
///
/// Entries are intentionally narrow. Unsupported shorthand names remain
/// unsupported properties until an issue defines their grammar, expansion
/// order, tests, and docs.
#[derive(Clone, Copy, Debug)]
pub struct ShorthandRegistry {
    entries: &'static [ShorthandRegistration],
    lookup_by_name: &'static [ShorthandNameLookupEntry],
}

impl ShorthandRegistry {
    const fn new(
        entries: &'static [ShorthandRegistration],
        lookup_by_name: &'static [ShorthandNameLookupEntry],
    ) -> Self {
        Self {
            entries,
            lookup_by_name,
        }
    }

    pub fn entries(&self) -> &'static [ShorthandRegistration] {
        self.entries
    }

    pub fn get(&self, id: ShorthandId) -> &'static ShorthandRegistration {
        let registration = &self.entries[id.as_index()];
        debug_assert_eq!(
            registration.id(),
            id,
            "shorthand registry entry order must align with ShorthandId::as_index()"
        );
        registration
    }

    pub fn lookup(&self, name: &str) -> Option<&'static ShorthandRegistration> {
        let lookup_index = self
            .lookup_by_name
            .binary_search_by_key(&name, |entry| entry.name)
            .ok()?;

        Some(self.get(self.lookup_by_name[lookup_index].id))
    }

    pub fn lookup_id(&self, name: &str) -> Option<ShorthandId> {
        self.lookup(name).map(|entry| entry.id())
    }
}

pub fn shorthand_registry() -> &'static ShorthandRegistry {
    &SHORTHAND_REGISTRY
}

/// Deterministic debug snapshot for supported shorthand registrations.
///
/// This reports only implemented shorthand support. Unsupported shorthands
/// remain absent until their grammar and expansion behavior are implemented.
pub fn shorthand_registry_debug_snapshot() -> String {
    let registry = shorthand_registry();
    let mut output = String::from("version: 1\nshorthand-registry\n");
    writeln!(&mut output, "shorthands: {}", registry.entries().len()).expect("write snapshot");

    for (index, shorthand) in registry.entries().iter().enumerate() {
        writeln!(&mut output, "shorthand[{index}]: {}", shorthand.name()).expect("write snapshot");
        writeln!(&mut output, "  longhands: {}", shorthand.longhands().len())
            .expect("write snapshot");
        for (longhand_index, longhand) in shorthand.longhands().iter().enumerate() {
            writeln!(
                &mut output,
                "  longhand[{longhand_index}]: {}",
                longhand.name()
            )
            .expect("write snapshot");
        }
    }

    output
}

const OUTLINE_LONGHANDS: [PropertyId; 3] = [
    PropertyId::OutlineColor,
    PropertyId::OutlineStyle,
    PropertyId::OutlineWidth,
];

const SHORTHAND_REGISTRATION_DATA: [ShorthandRegistration; 1] = [ShorthandRegistration::new(
    ShorthandId::Outline,
    "outline",
    &OUTLINE_LONGHANDS,
)];

const SHORTHAND_LOOKUP_BY_NAME: [ShorthandNameLookupEntry; 1] = [ShorthandNameLookupEntry::new(
    "outline",
    ShorthandId::Outline,
)];

static SHORTHAND_REGISTRY: ShorthandRegistry =
    ShorthandRegistry::new(&SHORTHAND_REGISTRATION_DATA, &SHORTHAND_LOOKUP_BY_NAME);

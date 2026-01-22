//! Generic, UI-agnostic identifier for input elements.
//!
//! This type intentionally uses a plain `u64` to avoid coupling to any DOM
//! or framework-specific identifier type. Integration layers can provide
//! `From` implementations to convert from their native ID types.

/// Opaque identifier for an input element within an [`InputValueStore`](crate::InputValueStore).
///
/// This is a lightweight, copyable handle that uniquely identifies an input field.
/// The actual value has no semantic meaning within this crate—it's just a key.
///
/// # Integration
///
/// To use with a DOM-based system, implement `From` in your integration layer:
///
/// ```ignore
/// use input_core::InputId;
/// use html::types::Id;
///
/// impl From<Id> for InputId {
///     fn from(id: Id) -> Self {
///         InputId::from_raw(id.0 as u64)
///     }
/// }
///
/// impl From<InputId> for Id {
///     fn from(id: InputId) -> Self {
///         Id(id.as_raw() as u32)
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InputId(u64);

impl InputId {
    /// Create an `InputId` from a raw u64 value.
    ///
    /// This is the primary way to construct an `InputId` from an external ID system.
    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Get the underlying raw value.
    ///
    /// Useful for converting back to an external ID system.
    #[inline]
    pub const fn as_raw(self) -> u64 {
        self.0
    }
}

impl From<u64> for InputId {
    #[inline]
    fn from(raw: u64) -> Self {
        Self::from_raw(raw)
    }
}

impl From<u32> for InputId {
    #[inline]
    fn from(raw: u32) -> Self {
        Self::from_raw(raw as u64)
    }
}

impl From<InputId> for u64 {
    #[inline]
    fn from(id: InputId) -> Self {
        id.as_raw()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_id_round_trip() {
        let raw = 42u64;
        let id = InputId::from_raw(raw);
        assert_eq!(id.as_raw(), raw);
    }

    #[test]
    fn input_id_from_u32() {
        let raw = 123u32;
        let id = InputId::from(raw);
        assert_eq!(id.as_raw(), 123u64);
    }

    #[test]
    fn input_id_equality() {
        let id1 = InputId::from_raw(1);
        let id2 = InputId::from_raw(1);
        let id3 = InputId::from_raw(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn input_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(InputId::from_raw(1));
        set.insert(InputId::from_raw(2));
        set.insert(InputId::from_raw(1)); // duplicate

        assert_eq!(set.len(), 2);
    }
}

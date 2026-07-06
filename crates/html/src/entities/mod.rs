mod minimal;
mod numeric;
mod policy;

#[cfg(feature = "html5-entities")]
mod html5;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use policy::decode_entities;
pub(crate) use policy::{
    CharacterReferenceContext, CharacterReferenceDiagnostic, decode_character_references,
};

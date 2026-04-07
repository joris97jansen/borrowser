mod minimal;
mod numeric;
mod policy;

#[cfg(feature = "html5-entities")]
mod html5;

#[cfg(test)]
mod tests;

pub(crate) use policy::decode_entities;

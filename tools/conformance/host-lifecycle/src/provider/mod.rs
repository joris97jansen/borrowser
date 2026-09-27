//! Versioned inert reconciliation contracts. No provider I/O or durable authority.
pub mod context;
pub mod coverage;
pub mod inventory;
pub mod limits;
pub mod manifest;
pub mod network_observation;
pub mod observation;
pub mod storage;

use crate::{Result, canonical, require};
use serde::Serialize;

pub(crate) fn sorted<T: Ord>(values: &[T], max: usize) -> Result<()> {
    require(
        values.len() <= max && values.windows(2).all(|v| v[0] < v[1]),
        "bounded sorted unique set",
    )
}
pub(crate) fn canonical_set<T: Serialize>(values: &[T], max: usize) -> Result<()> {
    require(values.len() <= max, "collection bound")?;
    let mut previous = None;
    for value in values {
        let bytes = canonical::encode(value)?;
        require(
            previous.as_ref().is_none_or(|p| p < &bytes),
            "canonical set order/duplicate",
        )?;
        previous = Some(bytes);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

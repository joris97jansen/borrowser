//! Versioned inert reconciliation contracts. No provider I/O or durable authority.
pub mod context;
pub mod context_v2;
pub mod coverage;
pub mod evidence_v2;
pub mod inventory;
pub mod inventory_v2;
pub mod limits;
pub mod management_observation_v2;
pub mod manifest;
pub mod network_observation;
pub mod observation;
pub mod observation_v2;
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

pub mod context_v3;
pub mod evidence_v3;
pub mod identity_observation_v3;
pub mod inventory_v3;
pub mod observation_v3;

pub mod ec2_observation_v4;

pub mod network_observation_v4;

pub mod endpoint_policy_observation_v4;

pub mod observation_v4;

pub mod allocation_value_v5;
pub mod ec2_allocation_observation_v5;
pub mod evidence_v4;
pub mod evidence_v5;
pub mod observation_v5;
pub mod source_occurrence_v5;

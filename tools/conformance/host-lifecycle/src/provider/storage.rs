//! Pure allocation arithmetic over supplied facts; never scans or changes storage.
use super::limits::*;
use crate::{Error, Result, canonical, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AllocationKind {
    ReconciliationEvidence,
    Manifest,
    Context,
    Metadata,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectAllocation {
    pub kind: AllocationKind,
    /// Complete retained length, not payload capacity.
    pub bytes: u64,
    /// Caller must verify exact existing bytes before asserting reuse.
    pub verified_reuse: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct StorageFacts {
    pub evidence_objects: u64,
    pub evidence_bytes: u64,
    pub next_sequence: u64,
    pub free_bytes: u64,
    pub free_inodes: u64,
    /// Filesystem allocation unit, not an assumed portable block size.
    pub allocation_unit: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReconciliationStorageBudgetV1 {
    pub new_objects: u64,
    pub new_evidence_bytes: u64,
    pub required_free_bytes: u64,
    pub required_free_inodes: u64,
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(Error("storage arithmetic overflow"))
}
fn allocated(n: u64, unit: u64) -> Result<u64> {
    require(unit > 0, "allocation unit")?;
    add(n, unit - 1)?
        .checked_div(unit)
        .and_then(|n| n.checked_mul(unit))
        .ok_or(Error("storage arithmetic overflow"))
}
impl ReconciliationStorageBudgetV1 {
    /// All referenced objects count toward the per-publication ceiling, including reused ones.
    /// All existing storage, including orphans, counts toward ordinary headroom.
    pub fn check(
        facts: StorageFacts,
        objects: &[ObjectAllocation],
        event_bytes: u64,
        normalized_bytes: u64,
    ) -> Result<Self> {
        require(
            objects.len() as u64 <= PUBLICATION_OBJECTS && normalized_bytes <= NORMALIZED_BYTES,
            "publication allocation bounds",
        )?;
        require(
            (1..=canonical::EVENT_BYTES as u64).contains(&event_bytes),
            "event allocation bound",
        )?;
        let mut total = 0;
        let mut fresh = 0;
        let mut count = 0;
        let mut disk = allocated(event_bytes, facts.allocation_unit)?;
        for o in objects {
            let cap = match o.kind {
                AllocationKind::ReconciliationEvidence => FUTURE_OBJECT_BYTES,
                AllocationKind::Context => 32 << 10,
                AllocationKind::Manifest | AllocationKind::Metadata => {
                    canonical::EVENT_BYTES as u64
                }
            };
            require(
                o.bytes > 0 && o.bytes <= cap,
                "opaque object allocation bound",
            )?;
            total = add(total, o.bytes)?;
            if !o.verified_reuse {
                count = add(count, 1)?;
                fresh = add(fresh, o.bytes)?;
                disk = add(disk, allocated(o.bytes, facts.allocation_unit)?)?;
            }
        }
        require(
            total <= PUBLICATION_BYTES && total.saturating_sub(normalized_bytes) <= OVERHEAD_BYTES,
            "publication evidence/overhead bound",
        )?;
        require(
            add(facts.evidence_objects, count)? <= 192
                && add(facts.evidence_bytes, fresh)? <= 48 << 20,
            "ordinary evidence headroom",
        )?;
        require(
            facts.next_sequence < 16_384 - 64,
            "ordinary journal headroom",
        )?;
        // One extra maximum canonical staging allocation and inode; reserves never fund this.
        disk = add(
            disk,
            allocated(canonical::EVENT_BYTES as u64, facts.allocation_unit)?,
        )?;
        let required_free_bytes = add(disk, 1 << 20)?;
        let required_free_inodes = add(count, 130)?; // event + staging + 128 remaining
        require(
            facts.free_bytes >= required_free_bytes && facts.free_inodes >= required_free_inodes,
            "ordinary publication space",
        )?;
        Ok(Self {
            new_objects: count,
            new_evidence_bytes: fresh,
            required_free_bytes,
            required_free_inodes,
        })
    }
}

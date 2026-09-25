//! Reviewed metadata is evidence of a review, not an authentication capability.
use crate::{Result, identity::ReviewText, require};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewV2 {
    pub reviewer: ReviewText,
    pub reference: ReviewText,
    pub reviewed_at_unix_seconds: u64,
    pub valid_from_unix_seconds: u64,
    pub valid_until_unix_seconds: u64,
}
impl ReviewV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.reviewed_at_unix_seconds > 0
                && self.reviewed_at_unix_seconds <= self.valid_from_unix_seconds
                && self.valid_from_unix_seconds < self.valid_until_unix_seconds
                && self.valid_until_unix_seconds <= 253_402_300_799,
            "review interval",
        )
    }
    /// Pure, caller-supplied time check; does not establish clock or reviewer trust.
    pub fn validate_at(&self, unix_seconds: u64) -> Result<()> {
        self.validate()?;
        require(
            self.valid_from_unix_seconds <= unix_seconds
                && unix_seconds < self.valid_until_unix_seconds,
            "review outside validity interval",
        )
    }
}

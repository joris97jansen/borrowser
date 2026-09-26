use crate::{Result, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSample {
    pub boot_id: String,
    pub boottime_ns: u64,
    pub realtime_ns: u64,
    pub time_namespace: String,
}
impl TimeSample {
    pub fn validate(&self) -> Result<()> {
        require(
            self.boot_id.len() == 36
                && self.boot_id.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                    }
                }),
            "boot identity",
        )?;
        crate::canonical::text(&self.time_namespace, 128)
    }
}

impl TimeSample {
    pub fn same_clock(&self, other: &Self) -> bool {
        self.boot_id == other.boot_id && self.time_namespace == other.time_namespace
    }
}
/// Exactly 120 seconds from the controller preparation sample, never renewed by retry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchDispatchDeadline {
    pub boottime_ns: u64,
}
impl LaunchDispatchDeadline {
    pub fn after(prepared_at: &TimeSample) -> Result<Self> {
        prepared_at.validate()?;
        Ok(Self {
            boottime_ns: prepared_at
                .boottime_ns
                .checked_add(120_000_000_000)
                .ok_or(crate::Error("dispatch deadline overflow"))?,
        })
    }
    pub fn validate(&self, prepared_at: &TimeSample) -> Result<()> {
        require(self == &Self::after(prepared_at)?, "exact dispatch window")
    }
    pub fn permits(&self, prepared_at: &TimeSample, now: &TimeSample) -> Result<()> {
        self.validate(prepared_at)?;
        now.validate()?;
        require(
            prepared_at.same_clock(now)
                && now.boottime_ns >= prepared_at.boottime_ns
                && now.boottime_ns < self.boottime_ns,
            "dispatch window/clock",
        )
    }
}
pub(crate) fn retry_not_before(
    outcome: &TimeSample,
    delay_seconds: u64,
    now: &TimeSample,
) -> Result<()> {
    outcome.validate()?;
    now.validate()?;
    let deadline = outcome
        .boottime_ns
        .checked_add(
            delay_seconds
                .checked_mul(1_000_000_000)
                .ok_or(crate::Error("retry delay overflow"))?,
        )
        .ok_or(crate::Error("retry deadline overflow"))?;
    require(
        outcome.same_clock(now) && now.boottime_ns >= deadline,
        "retry delay/clock",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derived_retry_arithmetic_never_wraps_or_accepts_a_caller_deadline() {
        let mut outcome = crate::test_support::genesis().time;
        outcome.boottime_ns = u64::MAX;
        assert!(retry_not_before(&outcome, 2, &outcome).is_err());
        outcome.boottime_ns = 1;
        assert!(retry_not_before(&outcome, u64::MAX, &outcome).is_err());
        for delay in [2, 8] {
            let mut now = outcome.clone();
            now.boottime_ns += delay * 1_000_000_000 - 1;
            assert!(retry_not_before(&outcome, delay, &now).is_err());
            now.boottime_ns += 1;
            assert!(retry_not_before(&outcome, delay, &now).is_ok());
        }
    }
}

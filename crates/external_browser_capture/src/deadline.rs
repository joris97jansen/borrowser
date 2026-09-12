//! One monotonic budget, created before any per-browser resources.
use crate::{
    CaptureError as E, Result,
    limits::{ATTEMPT_MS, COMMAND_MS},
};
use std::time::{Duration, Instant};
#[derive(Clone, Copy, Debug)]
pub struct AttemptDeadline(Instant);
impl Default for AttemptDeadline {
    fn default() -> Self {
        Self::new()
    }
}
impl AttemptDeadline {
    pub fn new() -> Self {
        Self(Instant::now() + Duration::from_millis(ATTEMPT_MS))
    }
    pub fn check(self) -> Result<()> {
        self.remaining().map(|_| ())
    }
    pub fn remaining(self) -> Result<Duration> {
        self.0
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(E::Deadline)
    }
    pub fn instant(self) -> Instant {
        self.0
    }
    pub fn command(self) -> Result<Instant> {
        self.check()?;
        Ok(self
            .0
            .min(Instant::now() + Duration::from_millis(COMMAND_MS)))
    }
    #[cfg(test)]
    pub(crate) fn expired() -> Self {
        Self(Instant::now())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_attempt_never_renews_for_later_phases() {
        let budget = AttemptDeadline::expired();
        for _phase in [
            "profile",
            "launch/handshake",
            "protocol",
            "post-observation",
            "cleanup/reap",
        ] {
            assert_eq!(budget.check(), Err(E::Deadline));
            assert_eq!(budget.command(), Err(E::Deadline));
        }
        let budget = AttemptDeadline::new();
        assert!(budget.command().unwrap() <= budget.instant());
    }
}

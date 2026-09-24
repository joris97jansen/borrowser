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

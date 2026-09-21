//! Closed, platform-independent wire contract for the three host-only probes.
//! Parsing observations is not native execution or qualification authority.
use crate::{Result, evidence::identifier, require};
use serde::{Deserialize, Serialize};

pub const PROBE_FORMAT: &str = "borrowser-ag9g0a-host-probe-observations-v1";
pub const PROBE_AUTHORITY: &str = "host-kernel-util-linux-prerequisite-only";
pub const REPORT_BYTES: usize = 65536;
pub const OBSERVATIONS: usize = 64;
pub const VALUE_BYTES: usize = 16384;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostProbe {
    #[serde(rename = "host-seccomp-prerequisite")]
    Seccomp,
    #[serde(rename = "host-unshare-fd-prerequisite")]
    UnshareFd,
    #[serde(rename = "host-process-inspection-prerequisite")]
    ProcessInspection,
}
impl HostProbe {
    pub const ALL: [Self; 3] = [Self::Seccomp, Self::UnshareFd, Self::ProcessInspection];
    pub const fn command(self) -> &'static str {
        match self {
            Self::Seccomp => "host-seccomp-prerequisite",
            Self::UnshareFd => "host-unshare-fd-prerequisite",
            Self::ProcessInspection => "host-process-inspection-prerequisite",
        }
    }
    pub fn parse(command: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.command() == command)
            .ok_or_else(|| "unknown host probe".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub name: String,
    pub value: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub format: String,
    pub authority: String,
    pub probe: HostProbe,
    pub observations: Vec<Observation>,
}
impl Report {
    pub fn new(probe: HostProbe) -> Self {
        Self {
            format: PROBE_FORMAT.into(),
            authority: PROBE_AUTHORITY.into(),
            probe,
            observations: vec![],
        }
    }
    pub fn add(&mut self, name: &str, value: impl ToString) {
        self.observations.push(Observation {
            name: name.into(),
            value: value.to_string(),
        });
    }
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == PROBE_FORMAT && self.authority == PROBE_AUTHORITY,
            "host report identity",
        )?;
        require(
            !self.observations.is_empty() && self.observations.len() <= OBSERVATIONS,
            "observation population",
        )?;
        let mut last: Option<&str> = None;
        for observation in &self.observations {
            identifier(&observation.name)?;
            require(
                last.is_none_or(|name| name < observation.name.as_str()),
                "duplicate or unordered observations",
            )?;
            require(observation.value.len() <= VALUE_BYTES, "observation size")?;
            last = Some(&observation.name);
        }
        Ok(())
    }
    pub fn canonical(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self)?;
        bytes.push(b'\n');
        require(bytes.len() <= REPORT_BYTES, "probe report bound")?;
        Ok(bytes)
    }
    /// Producer-only sorting. Readers never normalize an untrusted response.
    pub fn bytes(mut self) -> Result<Vec<u8>> {
        self.observations.sort_by(|a, b| a.name.cmp(&b.name));
        self.canonical()
    }
    pub fn parse_expected(bytes: &[u8], expected: HostProbe) -> Result<Self> {
        require(bytes.len() <= REPORT_BYTES, "probe report bound")?;
        let report: Self = serde_json::from_slice(bytes)?;
        require(report.probe == expected, "host report/request mismatch")?;
        require(report.canonical()? == bytes, "noncanonical host report")?;
        Ok(report)
    }
}

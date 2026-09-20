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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deadline {
    pub boot_id: String,
    pub time_namespace: String,
    pub expires_ns: u64,
}
impl Deadline {
    pub fn after(now: &TimeSample, seconds: u64) -> Result<Self> {
        now.validate()?;
        let expires_ns = seconds
            .checked_mul(1_000_000_000)
            .and_then(|n| now.boottime_ns.checked_add(n))
            .ok_or(crate::Error("deadline overflow"))?;
        Ok(Self {
            boot_id: now.boot_id.clone(),
            time_namespace: now.time_namespace.clone(),
            expires_ns,
        })
    }
    pub fn permits(&self, now: &TimeSample) -> bool {
        self.boot_id == now.boot_id
            && self.time_namespace == now.time_namespace
            && now.boottime_ns < self.expires_ns
    }
}

/// Each endpoint class has its own persisted request accounting. Counts never
/// authorize a resend; dispatch intent/reconciliation remain separate predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointClass {
    Allocation,
    Cancellation,
    TransactionHistory,
    Transaction,
    Server,
    CancellationRead,
    Catalogue,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Quota {
    pub requests: u64,
    pub interval_seconds: u64,
}
impl EndpointClass {
    /// Robot Webservice reference, reviewed 2026-09-19.
    pub fn quota(self) -> Quota {
        match self {
            Self::Allocation => Quota {
                requests: 20,
                interval_seconds: 86400,
            },
            Self::TransactionHistory | Self::Transaction | Self::Catalogue => Quota {
                requests: 500,
                interval_seconds: 3600,
            },
            _ => Quota {
                requests: 200,
                interval_seconds: 3600,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetWindow {
    pub endpoint: EndpointClass,
    pub boot_id: String,
    pub time_namespace: String,
    pub start_ns: u64,
    pub spent: u64,
    pub charges_ns: Vec<u64>,
}
impl BudgetWindow {
    pub fn charge(&mut self, now: &TimeSample) -> Result<()> {
        self.charge_with(now, self.endpoint.quota())
    }
    pub(crate) fn charge_with(&mut self, now: &TimeSample, q: Quota) -> Result<()> {
        require(
            self.boot_id == now.boot_id && self.time_namespace == now.time_namespace,
            "budget reboot hold requires reconciliation",
        )?;
        require(now.boottime_ns >= self.start_ns, "clock regression")?;
        require(
            self.charges_ns.len() <= self.endpoint.quota().requests as usize
                && self.charges_ns.windows(2).all(|p| p[0] <= p[1])
                && self.charges_ns.last().is_none_or(|t| *t <= now.boottime_ns),
            "request accounting order",
        )?;
        let interval_ns = q
            .interval_seconds
            .checked_mul(1_000_000_000)
            .ok_or(crate::Error("quota interval overflow"))?;
        self.charges_ns
            .retain(|t| now.boottime_ns - *t < interval_ns);
        require(
            self.charges_ns.len() < q.requests as usize,
            "endpoint budget exhausted",
        )?;
        self.charges_ns.push(now.boottime_ns);
        self.spent = self.charges_ns.len() as u64;
        Ok(())
    }
}

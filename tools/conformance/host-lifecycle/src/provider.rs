//! Hetzner-specific wire projection. No provisioning or arbitrary HTTP surface.
use crate::{Result, canonical, require};
use crate::{approval::*, identity::*};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationRequest {
    pub product_id: ProductId,
    pub location: String,
    pub addons: Vec<String>,
}
impl AllocationRequest {
    pub fn validate(&self) -> Result<()> {
        canonical::text(&self.product_id, 128)?;
        require(
            self.location == "FSN1" && self.addons == ["primary_ipv4"],
            "unsupported allocation projection",
        )
    }
    pub fn form(&self) -> Result<String> {
        self.validate()?;
        Ok(format!(
            "product_id={}&location=FSN1&addon%5B%5D=primary_ipv4",
            form_component(&self.product_id)
        ))
    }
    pub fn fingerprint(&self) -> Result<RequestFingerprint> {
        self.validate()?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}
fn form_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
pub const CANCELLATION_FORM: &str = "cancellation_date=now&reserve_location=false";

/// Exact gross EUR units of 1/10,000; no floating point or currency conversion.
pub fn euro_units(s: &str) -> Result<u64> {
    let (whole, fraction) = s.split_once('.').ok_or(crate::Error("price decimal"))?;
    require(
        !whole.is_empty()
            && whole.len() <= 10
            && whole.bytes().all(|b| b.is_ascii_digit())
            && fraction.len() == 4
            && fraction.bytes().all(|b| b.is_ascii_digit()),
        "price syntax",
    )?;
    whole
        .parse::<u64>()
        .ok()
        .and_then(|w| w.checked_mul(10_000))
        .and_then(|w| fraction.parse::<u64>().ok().and_then(|f| w.checked_add(f)))
        .ok_or(crate::Error("price overflow"))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueQuote {
    pub approval: ProductApproval,
    pub catalogue_evidence_sha256: EvidenceDigest,
    pub live_identity: CatalogueIdentity,
    pub available_locations: Vec<String>,
    pub ipv4: Ipv4Capability,
    pub product_id: ProductId,
    pub name: String,
    pub location: String,
    pub monthly_gross_units: u64,
    pub setup_gross_units: u64,
    pub approved_monthly_gross_units: u64,
    pub approved_setup_gross_units: u64,
}
impl CatalogueQuote {
    pub fn validate(&self) -> Result<()> {
        self.approval.validate()?;
        self.live_identity.validate()?;
        self.ipv4.validate()?;
        require(
            self.approval.digest()? == self.catalogue_evidence_sha256
                && self.live_identity == self.approval.catalogue
                && self.product_id == self.approval.catalogue.product_id
                && self.name == self.live_identity.name
                && self.approved_monthly_gross_units == self.approval.monthly_gross_ceiling
                && self.approved_setup_gross_units == self.approval.setup_gross_ceiling
                && self.available_locations.len() <= 16
                && self.available_locations.windows(2).all(|w| w[0] < w[1])
                && self.available_locations.iter().any(|l| l == "FSN1"),
            "reviewed product binding mismatch",
        )?;
        canonical::text(&self.product_id, 128)?;
        canonical::text(&self.name, 1024)?;
        require(
            self.location == "FSN1"
                && self.monthly_gross_units <= self.approved_monthly_gross_units
                && self.setup_gross_units <= self.approved_setup_gross_units,
            "catalogue spending approval",
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    #[serde(rename = "in process")]
    InProcess,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "cancelled")]
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    pub id: RobotTransactionId,
    pub date: String,
    pub status: TransactionStatus,
    pub server_number: Option<ServerNumber>,
    pub product_id: ProductId,
    pub location: Option<String>,
    pub addons: Vec<String>,
}
impl Transaction {
    pub fn validate(&self) -> Result<()> {
        canonical::text(&self.id, 128)?;
        canonical::text(&self.date, 128)?;
        canonical::text(&self.product_id, 128)?;
        if let Some(l) = &self.location {
            canonical::text(l, 128)?;
        }
        require(
            self.addons.len() <= 16 && self.addons.windows(2).all(|p| p[0] < p[1]),
            "addon ordering",
        )?;
        for a in &self.addons {
            canonical::text(a, 128)?;
        }
        require(
            self.status != TransactionStatus::Ready || self.server_number.is_some(),
            "ready without server",
        )
    }
    pub fn compatible(&self, r: &AllocationRequest) -> bool {
        self.product_id == r.product_id
            && self.location.as_ref() == Some(&r.location)
            && self.addons == r.addons
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationObservation {
    pub server_number: ServerNumber,
    pub cancelled: bool,
    pub reservation_possible: bool,
    pub reserved: bool,
    pub cancellation_date: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProviderFailure {
    Authentication,
    RateLimited {
        max_request: u64,
        interval_seconds: u64,
    },
    Maintenance,
    DefinitelyNotTransmitted,
    TransmissionUncertain,
    Malformed,
    Oversized,
    Rejected,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum TransactionResponse {
    NoIdentity(ProviderFailure),
    IdentityOnly {
        id: RobotTransactionId,
        failure: ProviderFailure,
    },
    Normalized(Transaction),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "in process")]
    InProcess,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerObservation {
    pub number: ServerNumber,
    pub product: String,
    pub datacenter: String,
    pub status: ServerStatus,
    pub cancelled: bool,
}

/// Request which produced transaction material; never inferred from its identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransactionSource {
    AllocationResponse,
    TransactionRead,
    TransactionHistoryItem,
}
impl TransactionSource {
    pub fn endpoint(self) -> crate::scheduling::EndpointClass {
        use crate::scheduling::EndpointClass;
        match self {
            Self::AllocationResponse => EndpointClass::Allocation,
            Self::TransactionRead => EndpointClass::Transaction,
            Self::TransactionHistoryItem => EndpointClass::TransactionHistory,
        }
    }
}

impl TransactionResponse {
    pub fn identity(&self) -> Option<&RobotTransactionId> {
        match self {
            Self::NoIdentity(_) => None,
            Self::IdentityOnly { id, .. } => Some(id),
            Self::Normalized(t) => Some(&t.id),
        }
    }
}

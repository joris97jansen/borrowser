//! Reviewed mapping between Robot catalogue/server names and the approved class.
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueIdentity {
    pub product_id: ProductId,
    pub name: String,
    pub description: Vec<String>,
}
impl CatalogueIdentity {
    pub fn validate(&self) -> Result<()> {
        canonical::text(&self.name, 1024)?;
        require(
            !self.description.is_empty() && self.description.len() <= 16,
            "catalogue description bound",
        )?;
        for line in &self.description {
            canonical::text(line, 512)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductApproval {
    pub schema_version: u64,
    pub account_scope: AccountScopeId,
    pub hardware_class: String,
    pub catalogue: CatalogueIdentity,
    /// Provider-native server.product spelling, independently reviewed; not marketing-name inference.
    pub server_product: String,
    pub location: String,
    pub addon: String,
    pub monthly_gross_ceiling: u64,
    pub setup_gross_ceiling: u64,
    pub attested_account_currency: String,
    pub reviewer: String,
    pub provider_reference: String,
}
impl ProductApproval {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 1
                && self.hardware_class == "AX42-1"
                && self.location == "FSN1"
                && self.addon == "primary_ipv4"
                && self.attested_account_currency == "EUR",
            "unsupported product approval",
        )?;
        self.catalogue.validate()?;
        canonical::text(&self.server_product, 128)?;
        canonical::text(&self.reviewer, 128)?;
        canonical::text(&self.provider_reference, 1024)
    }
    pub fn digest(&self) -> Result<EvidenceDigest> {
        self.validate()?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
    pub fn verify_bytes(bytes: &[u8], expected: &EvidenceDigest) -> Result<Self> {
        require(
            canonical::sha256(bytes) == expected.as_str(),
            "product approval evidence digest",
        )?;
        let approval: Self = canonical::decode(bytes)?;
        approval.validate()?;
        Ok(approval)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ipv4Capability {
    pub id: String,
    pub minimum: u64,
    pub maximum: u64,
    /// Optional provider-level location restriction plus location-specific pricing.
    pub location: Option<String>,
    pub price_location: String,
}
impl Ipv4Capability {
    pub fn validate(&self) -> Result<()> {
        require(
            self.id == "primary_ipv4"
                && self.minimum <= 1
                && self.maximum >= 1
                && self.minimum <= self.maximum
                && self.location.as_deref().is_none_or(|l| l == "FSN1")
                && self.price_location == "FSN1",
            "primary IPv4 not orderable at FSN1",
        )
    }
}

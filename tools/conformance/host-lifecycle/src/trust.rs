//! Trust-artifact structure only. No certificate, CMS, or IID verification.
use crate::{Result, canonical, identity::*, require, review::ReviewV2};
use serde::{Deserialize, Serialize};
pub const TRUST_FORMAT: &str = "borrowser-aws-ec2-identity-trust";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentitySignatureProfileV2 {
    #[serde(rename = "rsa2048-cms-signed-data-sha256-rsa-pkcs1-v1_5")]
    Rsa2048CmsSha256,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityTrustV2 {
    pub format: String,
    pub schema_version: u64,
    pub region: Region,
    pub profile: IdentitySignatureProfileV2,
    /// Lowercase hex, 1..=16,384 bytes, losslessly reconstructed. Not parsed here.
    pub certificate_der_hex: String,
    pub certificate_sha256: CertificateDigest,
    pub rsa_modulus_bits: u64,
    pub rsa_public_exponent: u64,
    pub source_reference: ReviewText,
    pub review: ReviewV2,
    pub replaces: Option<IdentityTrustDigest>,
}
impl IdentityTrustV2 {
    pub fn certificate_bytes(&self) -> Result<Vec<u8>> {
        let h = &self.certificate_der_hex;
        require(
            !h.is_empty()
                && h.len() <= 32_768
                && h.len().is_multiple_of(2)
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "certificate hex encoding",
        )?;
        h.as_bytes()
            .chunks_exact(2)
            .map(|p| {
                let digit = |b: u8| {
                    if b.is_ascii_digit() {
                        b - b'0'
                    } else {
                        b - b'a' + 10
                    }
                };
                Ok(digit(p[0]) * 16 + digit(p[1]))
            })
            .collect()
    }
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == TRUST_FORMAT && self.schema_version == 2,
            "trust generation",
        )?;
        require(
            self.rsa_modulus_bits == 2048 && self.rsa_public_exponent == 65_537,
            "trust key profile",
        )?;
        require(
            canonical::sha256(&self.certificate_bytes()?) == self.certificate_sha256.as_str(),
            "certificate digest",
        )?;
        self.review.validate()
    }
    pub fn digest(&self) -> Result<IdentityTrustDigest> {
        self.validate()?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}

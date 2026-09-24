//! Generation-specific local deployment and immutable completion marker.
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};
pub const DEPLOYMENT_FORMAT: &str = "borrowser-aws-ec2-deployment";
pub const ROOT_FORMAT: &str = "borrowser-aws-ec2-authority-root";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityIdentityV2 {
    pub authority_id: AuthorityId,
    pub account_id: AwsAccountId,
    pub region: Region,
    pub controller_machine_id: String,
    pub filesystem_uuid: String,
}
impl AuthorityIdentityV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.controller_machine_id.len() == 32
                && self
                    .controller_machine_id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "controller machine identity",
        )?;
        require(
            self.filesystem_uuid.len() == 36
                && self.filesystem_uuid.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                    }
                }),
            "filesystem identity",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentV2 {
    pub format: String,
    pub schema_version: u64,
    pub identity: AuthorityIdentityV2,
}
impl DeploymentV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == DEPLOYMENT_FORMAT && self.schema_version == 2,
            "deployment generation",
        )?;
        self.identity.validate()
    }
    pub fn marker(&self) -> Result<AuthorityRootV2> {
        self.validate()?;
        Ok(AuthorityRootV2 {
            authority: crate::model::AUTHORITY.into(),
            format: ROOT_FORMAT.into(),
            schema_version: 2,
            identity: self.identity.clone(),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRootV2 {
    pub authority: String,
    pub format: String,
    pub schema_version: u64,
    pub identity: AuthorityIdentityV2,
}
impl AuthorityRootV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.authority == crate::model::AUTHORITY
                && self.format == ROOT_FORMAT
                && self.schema_version == 2,
            "authority root generation",
        )?;
        self.identity.validate()
    }
    pub fn digest(&self) -> Result<AuthorityRootDigest> {
        self.validate()?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}

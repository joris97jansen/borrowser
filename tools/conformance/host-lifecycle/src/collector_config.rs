//! Non-executable user data for an already installed, reviewed collector.
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};
pub const COLLECTOR_FORMAT: &str = "borrowser-aws-ec2-identity-collector-config";
pub const INGRESS_SCHEME: &str = "ag9g0d/aws-ec2-v2/identity-ingress/<aws-userid>";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectorConfigV2 {
    pub format: String,
    pub schema_version: u64,
    pub account_id: AwsAccountId,
    pub region: Region,
    pub role_unique_id: IamRoleId,
    pub evidence_bucket: EvidenceBucketName,
    pub ingress_scheme: String,
}
impl CollectorConfigV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == COLLECTOR_FORMAT
                && self.schema_version == 2
                && self.ingress_scheme == INGRESS_SCHEME,
            "collector configuration generation",
        )
    }
    /// Exact UTF-8 bytes to be base64 encoded by the future SDK projection once.
    pub fn user_data_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = canonical::encode(self)?;
        require(bytes.len() <= 1024, "collector user-data bound")?;
        Ok(bytes)
    }
}

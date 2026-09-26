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
    /// Absent only for the frozen local-only Pass-1 document. Never a launch default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_support: Option<ReviewedSupportV2>,
}
impl DeploymentV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == DEPLOYMENT_FORMAT && self.schema_version == 2,
            "deployment generation",
        )?;
        self.identity.validate()?;
        if let Some(support) = &self.reviewed_support {
            support.validate(&self.identity)?;
        }
        require(canonical::encode(self)?.len() <= 16_384, "deployment bound")
    }
    pub fn digest(&self) -> Result<DeploymentDigest> {
        self.validate()?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
    pub fn support(&self) -> Result<&ReviewedSupportV2> {
        self.validate()?;
        self.reviewed_support
            .as_ref()
            .ok_or(crate::Error("reviewed AWS support required"))
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

/// Static reviewed deployed identities, not CDK discovery or launch permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedSupportV2 {
    pub infrastructure_reference: ReviewText,
    pub infrastructure_sha256: InfrastructureDigest,
    pub review: crate::review::ReviewV2,
    pub availability_zone: AvailabilityZone,
    pub availability_zone_id: AvailabilityZoneId,
    pub vpc_id: VpcId,
    pub subnet_id: SubnetId,
    pub security_group_ids: Vec<SecurityGroupId>,
    pub instance_profile_arn: InstanceProfileArn,
    pub instance_profile_id: InstanceProfileId,
    pub role_arn: IamRoleArn,
    pub role_unique_id: IamRoleId,
    pub evidence_bucket: EvidenceBucketName,
    pub evidence_bucket_region: Region,
    pub s3_gateway_endpoint_region: Region,
    pub s3_gateway_endpoint_id: VpcEndpointId,
    pub subnet_route_table_id: RouteTableId,
    pub kms_key_arn: KmsKeyArn,
    pub identity_trust_sha256: IdentityTrustDigest,
}
impl ReviewedSupportV2 {
    pub fn validate(&self, identity: &AuthorityIdentityV2) -> Result<()> {
        self.review.validate()?;
        require(
            self.evidence_bucket_region == identity.region
                && self.s3_gateway_endpoint_region == identity.region,
            "private S3 same-region boundary",
        )?;
        validate_groups(&self.security_group_ids)?;
        arn_binding(
            self.instance_profile_arn.as_str(),
            &identity.account_id,
            None,
        )?;
        arn_binding(self.role_arn.as_str(), &identity.account_id, None)?;
        arn_binding(
            self.kms_key_arn.as_str(),
            &identity.account_id,
            Some(&identity.region),
        )?;
        require(
            self.instance_profile_arn.split(':').nth(1) == self.role_arn.split(':').nth(1)
                && self.role_arn.split(':').nth(1) == self.kms_key_arn.split(':').nth(1),
            "ARN partition binding",
        )
    }
}
pub(crate) fn validate_groups(groups: &[SecurityGroupId]) -> Result<()> {
    require(
        !groups.is_empty() && groups.len() <= 5 && groups.windows(2).all(|p| p[0] < p[1]),
        "sorted unique security groups",
    )
}

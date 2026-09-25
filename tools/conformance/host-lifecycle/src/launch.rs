//! Closed reviewed launch documents. None of these values is a mutation capability.
use crate::{
    Error, Result, canonical, collector_config::*, deployment::*, identity::*, require,
    review::ReviewV2, trust::IdentityTrustV2,
};
use serde::{Deserialize, Serialize};
pub const APPROVAL_FORMAT: &str = "borrowser-aws-ec2-launch-approval";
pub const SPEC_FORMAT: &str = "borrowser-aws-ec2-launch-spec";
pub const REQUEST_FORMAT: &str = "borrowser-aws-ec2-run-instances-request";

/// All policy fields are explicit on the wire. Equality to V2 is mandatory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchPolicyV2 {
    pub architecture: String,
    pub operating_system: String,
    pub virtualization: String,
    pub root_device_type: String,
    pub market: String,
    pub min_count: u64,
    pub max_count: u64,
    pub network_interface_count: u64,
    pub network_device_index: u64,
    pub network_card_index: u64,
    pub delete_interface_on_termination: bool,
    pub associate_public_ip_address: bool,
    pub ipv6_address_count: u64,
    pub primary_ipv6: bool,
    pub private_ipv4_assignment: String,
    pub root_volume_count: u64,
    pub encrypted: bool,
    pub delete_volume_on_termination: bool,
    pub http_endpoint: String,
    pub http_tokens: String,
    pub http_put_response_hop_limit: u64,
    pub http_protocol_ipv6: String,
    pub instance_metadata_tags: String,
    pub instance_initiated_shutdown_behavior: String,
    pub disable_api_termination: bool,
    pub disable_api_stop: bool,
    pub monitoring_enabled: bool,
    pub ebs_optimized: bool,
    pub capacity_reservation_preference: String,
    pub hibernation: bool,
    pub enclaves: bool,
    pub tenancy: String,
    pub automatic_recovery: String,
    pub private_dns_hostname_type: String,
    pub private_dns_a_record: bool,
    pub private_dns_aaaa_record: bool,
}
impl LaunchPolicyV2 {
    pub fn fixed() -> Self {
        Self {
            architecture: "x86_64".into(),
            operating_system: "linux".into(),
            virtualization: "hvm".into(),
            root_device_type: "ebs".into(),
            market: "on-demand".into(),
            min_count: 1,
            max_count: 1,
            network_interface_count: 1,
            network_device_index: 0,
            network_card_index: 0,
            delete_interface_on_termination: true,
            associate_public_ip_address: false,
            ipv6_address_count: 0,
            primary_ipv6: false,
            private_ipv4_assignment: "subnet-assigned-primary-only".into(),
            root_volume_count: 1,
            encrypted: true,
            delete_volume_on_termination: true,
            http_endpoint: "enabled".into(),
            http_tokens: "required".into(),
            http_put_response_hop_limit: 1,
            http_protocol_ipv6: "disabled".into(),
            instance_metadata_tags: "disabled".into(),
            instance_initiated_shutdown_behavior: "stop".into(),
            disable_api_termination: false,
            disable_api_stop: true,
            monitoring_enabled: false,
            ebs_optimized: true,
            capacity_reservation_preference: "none".into(),
            hibernation: false,
            enclaves: false,
            tenancy: "default".into(),
            automatic_recovery: "disabled".into(),
            private_dns_hostname_type: "ip-name".into(),
            private_dns_a_record: false,
            private_dns_aaaa_record: false,
        }
    }
    pub fn validate(&self) -> Result<()> {
        require(self == &Self::fixed(), "unsupported V2 launch policy")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum RootVolumeV2 {
    Gp3 {
        size_gib: u64,
        iops: u64,
        throughput_mib_s: u64,
        kms_key_arn: KmsKeyArn,
    },
}
impl RootVolumeV2 {
    fn validate(&self, ceilings: &ResourceCeilingsV2) -> Result<()> {
        let Self::Gp3 {
            size_gib,
            iops,
            throughput_mib_s,
            ..
        } = self;
        // Structural gp3 bounds; exact reviewed values and future regional/type admission
        // still apply. See the pinned field-disposition contract, never use defaults.
        require(
            (1..=65_536).contains(size_gib)
                && (3000..=80_000).contains(iops)
                && (125..=2000).contains(throughput_mib_s)
                && *throughput_mib_s <= iops / 4
                && *iops <= 3000.max(size_gib * 500),
            "unsupported gp3 capacity",
        )?;
        require(
            *size_gib <= ceilings.max_root_size_gib
                && *iops <= ceilings.max_iops
                && *throughput_mib_s <= ceilings.max_throughput_mib_s,
            "root capacity ceiling",
        )
    }
    fn key(&self) -> &KmsKeyArn {
        let Self::Gp3 { kms_key_arn, .. } = self;
        kms_key_arn
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceCeilingsV2 {
    pub max_instances: u64,
    pub max_network_interfaces: u64,
    pub max_volumes: u64,
    pub max_root_size_gib: u64,
    pub max_iops: u64,
    pub max_throughput_mib_s: u64,
    pub max_vcpus: u64,
    pub max_memory_mib: u64,
}
impl ResourceCeilingsV2 {
    fn validate(&self) -> Result<()> {
        require(
            self.max_instances == 1
                && self.max_network_interfaces == 1
                && self.max_volumes == 1
                && (1..=65_536).contains(&self.max_root_size_gib)
                && (3000..=80_000).contains(&self.max_iops)
                && (125..=2000).contains(&self.max_throughput_mib_s)
                && self.max_vcpus > 0
                && self.max_memory_mib > 0,
            "resource ceilings",
        )
    }
}
/// Already-reviewed hourly allocations in the envelope's currency; no pricing formulas.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedHourlyCostV2 {
    pub compute_microunits: u64,
    pub root_ebs_microunits: u64,
    pub applicable_other_microunits: u64,
}
impl ReviewedHourlyCostV2 {
    pub fn total_microunits(&self) -> Result<u64> {
        self.compute_microunits
            .checked_add(self.root_ebs_microunits)
            .and_then(|sum| sum.checked_add(self.applicable_other_microunits))
            .ok_or(Error("reviewed hourly cost overflow"))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostCeilingsV2 {
    pub currency: String,
    pub reviewed_hourly: ReviewedHourlyCostV2,
    pub fixed_operation_microunits: u64,
    pub pricing_evidence_sha256: PricingEvidenceDigest,
    pub max_hourly_microunits: u64,
    pub max_operation_microunits: u64,
    pub planned_max_runtime_seconds: u64,
    pub review: ReviewV2,
}
/// Qualification planning is bounded to seven days; this is not a timer or permission.
pub const MAX_PLANNED_RUNTIME_SECONDS: u64 = 604_800;
impl CostCeilingsV2 {
    /// Reviewed variable-operation estimate. No enforcement or spending guarantee.
    pub fn planned_variable_cost_microunits(&self) -> Result<u64> {
        require(
            (1..=MAX_PLANNED_RUNTIME_SECONDS).contains(&self.planned_max_runtime_seconds),
            "planned runtime bound",
        )?;
        let product = self
            .reviewed_hourly
            .total_microunits()?
            .checked_mul(self.planned_max_runtime_seconds)
            .ok_or(Error("planned variable cost overflow"))?;
        let whole = product
            .checked_div(3600)
            .ok_or(Error("planned variable cost division"))?;
        let remainder = product
            .checked_rem(3600)
            .ok_or(Error("planned variable cost remainder"))?;
        whole
            .checked_add(u64::from(remainder != 0))
            .ok_or(Error("planned variable cost overflow"))
    }
    pub fn planned_operation_cost_microunits(&self) -> Result<u64> {
        self.planned_variable_cost_microunits()?
            .checked_add(self.fixed_operation_microunits)
            .ok_or(Error("planned operation cost overflow"))
    }
    fn validate(&self) -> Result<()> {
        self.review.validate()?;
        let hourly_total = self.reviewed_hourly.total_microunits()?;
        require(
            self.currency.len() == 3
                && self.currency.bytes().all(|b| b.is_ascii_uppercase())
                && hourly_total > 0
                && hourly_total <= self.max_hourly_microunits
                && self.planned_operation_cost_microunits()? <= self.max_operation_microunits,
            "reviewed cost ceiling",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagV2 {
    pub key: String,
    pub value: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagSpecificationsV2 {
    pub instance: Vec<TagV2>,
    pub volume: Vec<TagV2>,
    pub network_interface: Vec<TagV2>,
}
impl TagSpecificationsV2 {
    fn validate(&self, operation_tags: bool) -> Result<()> {
        for tags in [&self.instance, &self.volume, &self.network_interface] {
            require(
                !tags.is_empty()
                    && tags.len() <= if operation_tags { 18 } else { 16 }
                    && tags.windows(2).all(|p| p[0].key < p[1].key),
                "sorted unique bounded tags",
            )?;
            for tag in tags {
                canonical::text(&tag.key, 128)?;
                canonical::text(&tag.value, 256)?;
                require(
                    !tag.key.to_ascii_lowercase().starts_with("aws:")
                        && (operation_tags || !tag.key.starts_with("borrowser:")),
                    "reserved tag namespace",
                )?;
            }
        }
        Ok(())
    }
    fn for_operation(&self, authority: &AuthorityId, operation: &OperationId) -> Self {
        let mut tags = self.clone();
        for set in [
            &mut tags.instance,
            &mut tags.volume,
            &mut tags.network_interface,
        ] {
            set.extend([
                TagV2 {
                    key: "borrowser:authority-id".into(),
                    value: authority.to_string(),
                },
                TagV2 {
                    key: "borrowser:operation-id".into(),
                    value: operation.to_string(),
                },
            ]);
            set.sort();
        }
        tags
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmiReviewV2 {
    pub image_id: AmiId,
    pub provenance_reference: ReviewText,
    pub provenance_sha256: AmiProvenanceDigest,
    pub owner_account_id: AwsAccountId,
    pub root_device_name: String,
    /// Reviewed AMI must have precisely one EBS mapping, no inherited extra storage,
    /// no paid product/billing code and a preinstalled non-executable-config collector.
    pub review: ReviewV2,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchParametersV2 {
    pub account_id: AwsAccountId,
    pub region: Region,
    pub availability_zone: AvailabilityZone,
    pub availability_zone_id: AvailabilityZoneId,
    pub subnet_id: SubnetId,
    pub vpc_id: VpcId,
    pub ami: AmiReviewV2,
    pub instance_type: InstanceType,
    pub security_group_ids: Vec<SecurityGroupId>,
    pub instance_profile_arn: InstanceProfileArn,
    pub instance_profile_id: InstanceProfileId,
    pub role_arn: IamRoleArn,
    pub role_unique_id: IamRoleId,
    pub root_volume: RootVolumeV2,
    pub policy: LaunchPolicyV2,
    pub tags: TagSpecificationsV2,
    /// Exact canonical collector JSON including terminal LF, not shell/cloud-init.
    pub user_data: String,
}
impl LaunchParametersV2 {
    fn validate(
        &self,
        deployment: &DeploymentV2,
        ceilings: &ResourceCeilingsV2,
        operation_tags: bool,
    ) -> Result<()> {
        let s = deployment.support()?;
        require(
            self.account_id == deployment.identity.account_id
                && self.region == deployment.identity.region
                && self.availability_zone == s.availability_zone
                && self.availability_zone_id == s.availability_zone_id
                && self.subnet_id == s.subnet_id
                && self.vpc_id == s.vpc_id
                && self.security_group_ids == s.security_group_ids
                && self.instance_profile_arn == s.instance_profile_arn
                && self.instance_profile_id == s.instance_profile_id
                && self.role_arn == s.role_arn
                && self.role_unique_id == s.role_unique_id
                && self.root_volume.key() == &s.kms_key_arn,
            "launch/deployment binding",
        )?;
        self.policy.validate()?;
        self.root_volume.validate(ceilings)?;
        self.tags.validate(operation_tags)?;
        self.ami.review.validate()?;
        let device = self
            .ami
            .root_device_name
            .strip_prefix("/dev/")
            .ok_or(Error("root device name"))?;
        require(
            !device.is_empty()
                && device.len() <= 32
                && device.bytes().all(|b| b.is_ascii_alphanumeric()),
            "root device name",
        )?;
        require(self.user_data.len() <= 1024, "user-data bound")?;
        let config: CollectorConfigV2 = canonical::decode(self.user_data.as_bytes())?;
        config.validate()?;
        require(
            config.account_id == self.account_id
                && config.region == self.region
                && config.role_unique_id == self.role_unique_id
                && config.evidence_bucket == s.evidence_bucket,
            "collector deployment binding",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchApprovalV2 {
    pub format: String,
    pub schema_version: u64,
    pub deployment_sha256: DeploymentDigest,
    pub infrastructure_sha256: InfrastructureDigest,
    pub identity_trust_sha256: IdentityTrustDigest,
    pub launch: LaunchParametersV2,
    pub resource_ceilings: ResourceCeilingsV2,
    pub cost_ceilings: CostCeilingsV2,
    pub review: ReviewV2,
}
impl LaunchApprovalV2 {
    pub fn validate(&self, deployment: &DeploymentV2, trust: &IdentityTrustV2) -> Result<()> {
        require(
            self.format == APPROVAL_FORMAT && self.schema_version == 2,
            "launch approval generation",
        )?;
        let support = deployment.support()?;
        require(
            self.deployment_sha256 == deployment.digest()?
                && self.infrastructure_sha256 == support.infrastructure_sha256
                && self.identity_trust_sha256 == support.identity_trust_sha256
                && self.identity_trust_sha256 == trust.digest()?
                && trust.region == deployment.identity.region,
            "approval deployment/trust binding",
        )?;
        self.review.validate()?;
        self.resource_ceilings.validate()?;
        self.cost_ceilings.validate()?;
        for prerequisite in [
            &support.review,
            &trust.review,
            &self.launch.ami.review,
            &self.cost_ceilings.review,
        ] {
            prerequisite.validate()?;
            require(
                self.review.valid_from_unix_seconds >= prerequisite.valid_from_unix_seconds
                    && self.review.valid_until_unix_seconds
                        <= prerequisite.valid_until_unix_seconds,
                "launch review exceeds prerequisite validity",
            )?;
        }
        self.launch
            .validate(deployment, &self.resource_ceilings, false)
    }
    pub fn digest(
        &self,
        deployment: &DeploymentV2,
        trust: &IdentityTrustV2,
    ) -> Result<LaunchApprovalDigest> {
        self.validate(deployment, trust)?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchSpecV2 {
    pub format: String,
    pub schema_version: u64,
    pub authority_id: AuthorityId,
    pub operation_id: OperationId,
    pub deployment_sha256: DeploymentDigest,
    pub launch_approval_sha256: LaunchApprovalDigest,
    pub launch: LaunchParametersV2,
}
impl LaunchSpecV2 {
    pub fn new(
        deployment: &DeploymentV2,
        approval: &LaunchApprovalV2,
        trust: &IdentityTrustV2,
        operation_id: OperationId,
    ) -> Result<Self> {
        let launch_approval_sha256 = approval.digest(deployment, trust)?;
        let mut launch = approval.launch.clone();
        launch.tags = launch
            .tags
            .for_operation(&deployment.identity.authority_id, &operation_id);
        Ok(Self {
            format: SPEC_FORMAT.into(),
            schema_version: 2,
            authority_id: deployment.identity.authority_id.clone(),
            operation_id,
            deployment_sha256: deployment.digest()?,
            launch_approval_sha256,
            launch,
        })
    }
    pub fn validate(
        &self,
        deployment: &DeploymentV2,
        approval: &LaunchApprovalV2,
        trust: &IdentityTrustV2,
    ) -> Result<()> {
        require(
            self == &Self::new(deployment, approval, trust, self.operation_id.clone())?,
            "exact approved launch specification",
        )
    }
    /// Hashes typed canonical bytes, never implies review or mutation authority.
    pub fn fingerprint(&self) -> Result<LaunchSpecDigest> {
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}

/// A derived request identity, never a mutation capability or user-selected token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ClientToken(String);
impl TryFrom<String> for ClientToken {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        canonical::digest(&value)?;
        Ok(Self(value))
    }
}
impl From<ClientToken> for String {
    fn from(value: ClientToken) -> Self {
        value.0
    }
}
impl ClientToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn derive(
        spec: &LaunchSpecV2,
        deployment: &DeploymentV2,
        approval: &LaunchApprovalV2,
        trust: &IdentityTrustV2,
    ) -> Result<Self> {
        spec.validate(deployment, approval, trust)?;
        #[derive(Serialize)]
        struct Binding<'a> {
            domain: &'static str,
            authority_id: &'a AuthorityId,
            operation_id: &'a OperationId,
            account_id: &'a AwsAccountId,
            region: &'a Region,
            availability_zone_id: &'a AvailabilityZoneId,
            spec_hash: LaunchSpecDigest,
        }
        Self::try_from(canonical::sha256(&canonical::encode(&Binding {
            domain: "borrowser-aws-ec2-client-token-v2",
            authority_id: &spec.authority_id,
            operation_id: &spec.operation_id,
            account_id: &spec.launch.account_id,
            region: &spec.launch.region,
            availability_zone_id: &spec.launch.availability_zone_id,
            spec_hash: spec.fingerprint()?,
        })?))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunInstancesRequestV2 {
    pub format: String,
    pub schema_version: u64,
    pub min_count: u64,
    pub max_count: u64,
    pub client_token: ClientToken,
    pub spec_sha256: LaunchSpecDigest,
    pub spec: LaunchSpecV2,
}
impl RunInstancesRequestV2 {
    pub fn new(
        deployment: &DeploymentV2,
        approval: &LaunchApprovalV2,
        trust: &IdentityTrustV2,
        spec: LaunchSpecV2,
        client_token: ClientToken,
    ) -> Result<Self> {
        require(
            client_token == ClientToken::derive(&spec, deployment, approval, trust)?,
            "request token/spec binding",
        )?;
        Ok(Self {
            format: REQUEST_FORMAT.into(),
            schema_version: 2,
            min_count: 1,
            max_count: 1,
            client_token,
            spec_sha256: spec.fingerprint()?,
            spec,
        })
    }
    pub fn validate(
        &self,
        deployment: &DeploymentV2,
        approval: &LaunchApprovalV2,
        trust: &IdentityTrustV2,
    ) -> Result<()> {
        require(
            self == &Self::new(
                deployment,
                approval,
                trust,
                self.spec.clone(),
                self.client_token.clone(),
            )?,
            "exact final launch request",
        )
    }
    pub fn fingerprint(
        &self,
        deployment: &DeploymentV2,
        approval: &LaunchApprovalV2,
        trust: &IdentityTrustV2,
    ) -> Result<LaunchRequestDigest> {
        self.validate(deployment, approval, trust)?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}

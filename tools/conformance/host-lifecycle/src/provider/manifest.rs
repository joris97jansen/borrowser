//! Closed reviewed expectations. No AWS response parser or policy evaluator.
use super::{canonical_set, sorted};
use crate::{Result, canonical, deployment::DeploymentV2, identity::*, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ipv4Cidr {
    pub network: u32,
    pub prefix: u8,
}
impl Ipv4Cidr {
    pub fn validate(&self) -> Result<()> {
        require(self.prefix <= 32, "IPv4 prefix")?;
        let mask = if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix)
        };
        require(
            self.network & mask == self.network,
            "noncanonical IPv4 network",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Protocol {
    All,
    Tcp {
        from: u16,
        to: u16,
    },
    Udp {
        from: u16,
        to: u16,
    },
    Icmp {
        icmp_type: Option<u8>,
        code: Option<u8>,
    },
}
impl Protocol {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::All => Ok(()),
            Self::Tcp { from, to } | Self::Udp { from, to } => require(from <= to, "port range"),
            Self::Icmp { icmp_type, code } => require(
                icmp_type.is_some() || code.is_none(),
                "ICMP wildcard type/code",
            ),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Peer {
    Ipv4 {
        cidr: Ipv4Cidr,
    },
    SecurityGroup {
        account: AwsAccountId,
        vpc: VpcId,
        group: SecurityGroupId,
    },
    S3PrefixList {
        id: PrefixListId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityRule {
    pub protocol: Protocol,
    pub peer: Peer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityGroupExpectation {
    pub id: SecurityGroupId,
    pub ingress: Vec<SecurityRule>,
    pub egress: Vec<SecurityRule>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteAssociation {
    ExplicitSubnet,
    MainInheritance,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ReviewedRoute {
    /// The supported local route has CreateRouteTable origin and active state.
    Local { cidr: Ipv4Cidr },
    /// The supported S3 route has CreateRoute origin and active state.
    S3Gateway {
        prefix_list: PrefixListId,
        endpoint: VpcEndpointId,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    Allow,
    Deny,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum S3Action {
    #[serde(rename = "s3:GetObject")]
    GetObject,
    #[serde(rename = "s3:GetObjectVersion")]
    GetObjectVersion,
    #[serde(rename = "s3:PutObject")]
    PutObject,
    #[serde(rename = "s3:AbortMultipartUpload")]
    AbortMultipartUpload,
    #[serde(rename = "s3:ListMultipartUploadParts")]
    ListMultipartUploadParts,
    #[serde(rename = "s3:ListBucket")]
    ListBucket,
    #[serde(rename = "s3:ListBucketMultipartUploads")]
    ListBucketMultipartUploads,
    #[serde(rename = "s3:GetBucketLocation")]
    GetBucketLocation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Principal {
    Any,
    Roles { arns: Vec<IamRoleArn> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PolicyResource {
    Bucket,
    /// Literal prefix followed by one terminal '*'; no embedded pattern language.
    ObjectPrefix {
        prefix: String,
    },
    /// Exactly ag9g0d/aws-ec2-v2/identity-ingress/${aws:userid}/*.
    PrincipalIngress,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PolicyCondition {
    SecureTransport {
        required: bool,
    },
    PrincipalAccount {
        account: AwsAccountId,
    },
    PrincipalRole {
        arn: IamRoleArn,
    },
    SourceVpc {
        vpc: VpcId,
    },
    SourceEndpoint {
        endpoint: VpcEndpointId,
    },
    AwsKmsEncryption,
    EncryptionKey {
        key: KmsKeyArn,
    },
    /// StringLike aws:userid, exact reviewed role unique ID followed by ':*'.
    RoleSession {
        role_id: IamRoleId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyStatement {
    pub sid: Option<String>,
    pub effect: Effect,
    pub principal: Principal,
    pub actions: Vec<S3Action>,
    pub resources: Vec<PolicyResource>,
    pub conditions: Vec<PolicyCondition>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointPolicy {
    pub statements: Vec<PolicyStatement>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsDhcpExpectation {
    pub dhcp_options: DhcpOptionsId,
    pub enable_dns_support: bool,
    pub enable_dns_hostnames: bool,
    /// V1 permits AmazonProvidedDNS only. Explicit variant prevents arbitrary resolvers.
    pub servers: DnsServers,
    pub domain_name: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsServers {
    AmazonProvidedDNS,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NaclRule {
    pub number: u16,
    pub effect: Effect,
    pub cidr: Ipv4Cidr,
    pub protocol: Protocol,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NaclExpectation {
    pub id: NetworkAclId,
    pub ingress: Vec<NaclRule>,
    pub egress: Vec<NaclRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedInfrastructureV1 {
    pub format: String,
    pub schema_version: u64,
    pub account: AwsAccountId,
    pub region: Region,
    pub vpc: VpcId,
    pub subnet: SubnetId,
    pub route_table: RouteTableId,
    pub route_association: RouteAssociation,
    pub routes: Vec<ReviewedRoute>,
    pub s3_endpoint: VpcEndpointId,
    pub s3_prefix_list: PrefixListId,
    pub bucket: EvidenceBucketName,
    pub security_groups: Vec<SecurityGroupExpectation>,
    pub endpoint_policy: EndpointPolicy,
    pub dns: DnsDhcpExpectation,
    pub nacl: NaclExpectation,
}
impl ReviewedInfrastructureV1 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == "borrowser-reviewed-infrastructure" && self.schema_version == 1,
            "manifest version",
        )?;
        canonical_set(&self.routes, 128)?;
        require(!self.routes.is_empty(), "reviewed routes missing")?;
        let mut local = false;
        let mut s3 = false;
        for route in &self.routes {
            match route {
                ReviewedRoute::Local { cidr } => {
                    cidr.validate()?;
                    local = true;
                }
                ReviewedRoute::S3Gateway {
                    prefix_list,
                    endpoint,
                } => {
                    require(
                        prefix_list == &self.s3_prefix_list && endpoint == &self.s3_endpoint && !s3,
                        "S3 route binding",
                    )?;
                    s3 = true;
                }
            }
        }
        require(local && s3, "private route contract")?;
        require(
            !self.security_groups.is_empty()
                && self.security_groups.len() <= 5
                && self.security_groups.windows(2).all(|p| p[0].id < p[1].id),
            "security group set",
        )?;
        for group in &self.security_groups {
            for rules in [&group.ingress, &group.egress] {
                canonical_set(rules, 128)?;
                for rule in rules {
                    rule.protocol.validate()?;
                    match &rule.peer {
                        Peer::Ipv4 { cidr } => cidr.validate()?,
                        Peer::SecurityGroup { account, vpc, .. } => require(
                            account == &self.account && vpc == &self.vpc,
                            "SG peer ownership",
                        )?,
                        Peer::S3PrefixList { id } => {
                            require(id == &self.s3_prefix_list, "SG prefix list")?
                        }
                    }
                }
            }
        }
        self.validate_policy()?;
        let domain = &self.dns.domain_name;
        require(
            !domain.is_empty()
                && domain.len() <= 253
                && domain.split('.').all(|s| {
                    !s.is_empty()
                        && s.len() <= 63
                        && s.bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                        && !s.starts_with('-')
                        && !s.ends_with('-')
                }),
            "DHCP domain",
        )?;
        for rules in [&self.nacl.ingress, &self.nacl.egress] {
            require(
                !rules.is_empty()
                    && rules.len() <= 128
                    && rules.windows(2).all(|p| p[0].number < p[1].number),
                "NACL order/bound",
            )?;
            for rule in rules {
                require((1..=32767).contains(&rule.number), "NACL rule number")?;
                rule.cidr.validate()?;
                rule.protocol.validate()?;
            }
            let last = rules.last().expect("nonempty checked");
            require(
                last.number == 32767
                    && last.effect == Effect::Deny
                    && last.protocol == Protocol::All
                    && last.cidr
                        == (Ipv4Cidr {
                            network: 0,
                            prefix: 0,
                        }),
                "NACL default deny",
            )?;
        }
        canonical::encode(self).map(|_| ())
    }
    fn validate_policy(&self) -> Result<()> {
        let policy = &self.endpoint_policy;
        require(!policy.statements.is_empty(), "endpoint policy empty")?;
        canonical_set(&policy.statements, 16)?;
        require(
            canonical::encode(policy)?.len() <= 8192,
            "endpoint policy bound",
        )?;
        for s in &policy.statements {
            if let Some(sid) = &s.sid {
                require(
                    !sid.is_empty()
                        && sid.len() <= 128
                        && sid.bytes().all(|b| b.is_ascii_alphanumeric()),
                    "policy Sid",
                )?;
            }
            if let Principal::Roles { arns } = &s.principal {
                require(!arns.is_empty(), "empty principal")?;
                sorted(arns, 16)?;
                for arn in arns {
                    arn_binding(arn, &self.account, None)?;
                }
            }
            require(
                !s.actions.is_empty() && !s.resources.is_empty(),
                "empty policy action/resource",
            )?;
            canonical_set(&s.actions, 8)?;
            canonical_set(&s.resources, 16)?;
            canonical_set(&s.conditions, 8)?;
            let mut condition_kinds = std::collections::HashSet::new();
            for condition in &s.conditions {
                require(
                    condition_kinds.insert(std::mem::discriminant(condition)),
                    "duplicate policy condition kind",
                )?;
            }
            for resource in &s.resources {
                if let PolicyResource::ObjectPrefix { prefix } = resource {
                    require(
                        !prefix.is_empty()
                            && prefix.len() <= 256
                            && prefix.ends_with('/')
                            && prefix
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
                            && !prefix.split('/').any(|p| p == ".." || p == "."),
                        "literal object prefix",
                    )?;
                }
            }
            for condition in &s.conditions {
                match condition {
                    PolicyCondition::PrincipalAccount { account } => {
                        require(account == &self.account, "policy account")?
                    }
                    PolicyCondition::PrincipalRole { arn } => {
                        arn_binding(arn, &self.account, None)?
                    }
                    PolicyCondition::SourceVpc { vpc } => require(vpc == &self.vpc, "policy VPC")?,
                    PolicyCondition::SourceEndpoint { endpoint } => {
                        require(endpoint == &self.s3_endpoint, "policy endpoint")?
                    }
                    PolicyCondition::EncryptionKey { key } => {
                        arn_binding(key, &self.account, Some(&self.region))?
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        canonical::encode(self)
    }
    pub fn digest(&self) -> Result<InfrastructureDigest> {
        canonical::sha256(&self.canonical_bytes()?).parse()
    }
    pub fn parse_bound(bytes: &[u8], deployment: &DeploymentV2) -> Result<Self> {
        require(bytes.len() <= canonical::EVENT_BYTES, "manifest byte bound")?;
        let value: Self = canonical::decode(bytes)?;
        value.validate()?;
        deployment.validate()?;
        let support = deployment.support()?;
        require(
            value.digest()? == support.infrastructure_sha256,
            "manifest exact digest",
        )?;
        require(
            value.account == deployment.identity.account_id
                && value.region == deployment.identity.region
                && value.vpc == support.vpc_id
                && value.subnet == support.subnet_id
                && value.route_table == support.subnet_route_table_id
                && value.s3_endpoint == support.s3_gateway_endpoint_id
                && value.bucket == support.evidence_bucket
                && value
                    .security_groups
                    .iter()
                    .map(|s| &s.id)
                    .eq(support.security_group_ids.iter()),
            "manifest deployment binding",
        )?;
        for s in &value.endpoint_policy.statements {
            for c in &s.conditions {
                match c {
                    PolicyCondition::RoleSession { role_id } => {
                        require(role_id == &support.role_unique_id, "policy role ID")?
                    }
                    PolicyCondition::PrincipalRole { arn } => {
                        require(arn == &support.role_arn, "policy role ARN")?
                    }
                    PolicyCondition::EncryptionKey { key } => {
                        require(key == &support.kms_key_arn, "policy KMS key")?
                    }
                    _ => (),
                }
            }
        }
        Ok(value)
    }
}

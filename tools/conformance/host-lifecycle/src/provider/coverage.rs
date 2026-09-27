use super::{
    inventory::ResourceIdentity,
    limits::{self, LimitKind},
};
use crate::{Result, canonical, identity::*, launch::ClientToken, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AdmissionFactId {
    A01,
    A02,
    A03,
    A04,
    A05,
    A06,
    A07,
    A08,
    A09,
    A10,
    A11,
    A12,
    A13,
    A14,
    A15,
    A16,
    A17,
    A18,
    A19,
    A20,
    A21,
    A22,
    A23,
    A24,
    A25,
    A26,
    A27,
    A28,
    A29,
    A30,
    A31,
    A32,
    A33,
    A34,
    A35,
    A36,
    A37,
    A38,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadOperationV1 {
    GetCallerIdentity,
    HeadBucket,
    GetInstanceProfile,
    DescribeKey,
    DescribeRegions,
    DescribeAvailabilityZones,
    DescribeSubnets,
    DescribeVpcs,
    DescribeSecurityGroups,
    DescribeRouteTables,
    DescribeVpcEndpoints,
    DescribePrefixLists,
    DescribeVpcAttribute,
    DescribeDhcpOptions,
    DescribeNetworkAcls,
    DescribeImages,
    DescribeInstanceTypes,
    DescribeInstanceTypeOfferings,
    DescribeIamInstanceProfileAssociations,
    DescribeInstances,
    DescribeNetworkInterfaces,
    DescribeVolumes,
    DescribeInstanceAttribute,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceAttribute {
    UserData,
    InstanceInitiatedShutdownBehavior,
    DisableApiTermination,
    DisableApiStop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VpcAttribute {
    EnableDnsSupport,
    EnableDnsHostnames,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum QueryScopeV1 {
    Regional,
    Exact {
        identities: Vec<ResourceIdentity>,
    },
    ClientToken {
        token: ClientToken,
    },
    OperationTags {
        authority: AuthorityId,
        operation: OperationId,
    },
    AuthorityTag {
        authority: AuthorityId,
    },
    OperationTag {
        operation: OperationId,
    },
    AttachedTo {
        instance: InstanceId,
    },
    InstanceAttribute {
        instance: InstanceId,
        attribute: InstanceAttribute,
    },
    VpcAttribute {
        vpc: VpcId,
        attribute: VpcAttribute,
    },
    TypeOffering {
        instance_type: InstanceType,
        zone: AvailabilityZone,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryIdentityV1 {
    pub operation: ReadOperationV1,
    pub account: AwsAccountId,
    pub region: Region,
    pub scope: QueryScopeV1,
}
impl QueryIdentityV1 {
    pub fn validate(&self) -> Result<()> {
        if let QueryScopeV1::Exact { identities } = &self.scope {
            require(!identities.is_empty(), "empty exact query")?;
            super::canonical_set(identities, 128)?;
        }
        require(
            canonical::encode(self)?.len() <= limits::RECORD_BYTES,
            "query identity bound",
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "limit",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ReadFailureV1 {
    AccessDenied,
    NotFound,
    Service,
    Transport,
    SessionExpired,
    Unsupported,
    Malformed,
    PaginationCycle,
    Limit(LimitKind),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "reason",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum CoverageStatus {
    Complete,
    Incomplete(ReadFailureV1),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadCoverageV1 {
    pub query: QueryIdentityV1,
    pub required: bool,
    pub requests: u64,
    pub pages: u64,
    pub records: u64,
    pub terminal_page: bool,
    pub status: CoverageStatus,
}
impl ReadCoverageV1 {
    pub fn validate(&self) -> Result<()> {
        self.query.validate()?;
        require(
            self.requests <= limits::REQUESTS
                && self.pages <= limits::PAGES
                && self.pages <= self.requests
                && self.records <= limits::RECORDS,
            "coverage bounds",
        )?;
        if self.status == CoverageStatus::Complete {
            require(
                self.requests > 0 && self.pages > 0 && self.terminal_page,
                "complete coverage requires terminal response",
            )?;
        }
        Ok(())
    }
}

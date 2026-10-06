//! The A07 successor represents only DescribeRouteTables filtered by the reviewed
//! subnet association. It is not a V1 exact route-table query or a V4 record.
use super::{coverage::*, ec2_observation_v4::ObservationDataV4, limits::*};
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedSubnetRouteQueryV1 {
    pub schema_version: u64,
    pub operation: ReadOperationV1,
    pub account: AwsAccountId,
    pub region: Region,
    pub association_subnet: SubnetId,
}
impl ReviewedSubnetRouteQueryV1 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 1 && self.operation == ReadOperationV1::DescribeRouteTables,
            "reviewed subnet route query",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedSubnetRouteRecordV1 {
    pub schema_version: u64,
    pub query: ReviewedSubnetRouteQueryV1,
    pub data: ObservationDataV4,
}
impl ReviewedSubnetRouteRecordV1 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 1 && matches!(self.data, ObservationDataV4::RouteTable { .. }),
            "reviewed subnet route record",
        )?;
        self.query.validate()?;
        self.data.validate()
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        ObservationAccounting::default().canonical_record(self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedSubnetRouteCoverageV1 {
    pub schema_version: u64,
    pub query: ReviewedSubnetRouteQueryV1,
    pub required: bool,
    pub requests: u64,
    pub pages: u64,
    pub records: u64,
    pub terminal_page: bool,
    pub status: CoverageStatus,
}
impl ReviewedSubnetRouteCoverageV1 {
    pub fn validate(&self) -> Result<()> {
        self.query.validate()?;
        require(
            self.schema_version == 1
                && self.requests <= REQUESTS
                && self.pages <= PAGES
                && self.pages <= self.requests
                && self.records <= RECORDS,
            "route coverage bounds",
        )?;
        require(
            self.status != CoverageStatus::Complete
                || (self.requests > 0 && self.pages > 0 && self.terminal_page),
            "route terminal coverage",
        )
    }
}
/// No serialization: the mixed carrier and this family share one round budget.
#[derive(Clone, Debug)]
pub struct ReviewedSubnetRouteEvidenceV1 {
    pub context: ReconciliationContextDigest,
    pub records: Vec<ReviewedSubnetRouteRecordV1>,
    pub coverage: ReviewedSubnetRouteCoverageV1,
}

/// Closed in-memory selection; serialized member identities retain their own schemas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryQuery {
    Existing(QueryIdentityV1),
    ReviewedSubnetRoutes(ReviewedSubnetRouteQueryV1),
}
impl DiscoveryQuery {
    pub(crate) fn key(&self) -> Result<Vec<u8>> {
        match self {
            Self::Existing(v) => canonical::encode(v),
            Self::ReviewedSubnetRoutes(v) => canonical::encode(v),
        }
    }
    pub(crate) fn reservation(&self, required: bool) -> Result<usize> {
        let mut maximum = 0;
        for reason in [
            ReadFailureV1::AccessDenied,
            ReadFailureV1::NotFound,
            ReadFailureV1::Service,
            ReadFailureV1::Transport,
            ReadFailureV1::SessionExpired,
            ReadFailureV1::Unsupported,
            ReadFailureV1::Malformed,
            ReadFailureV1::PaginationCycle,
        ]
        .into_iter()
        .chain(
            [
                LimitKind::Pages,
                LimitKind::Requests,
                LimitKind::Records,
                LimitKind::ResponseBytes,
                LimitKind::RoundResponseBytes,
                LimitKind::NormalizedBytes,
                LimitKind::RecordBytes,
                LimitKind::Elapsed,
                LimitKind::Clock,
                LimitKind::Session,
                LimitKind::Cancelled,
                LimitKind::Body,
            ]
            .into_iter()
            .map(ReadFailureV1::Limit),
        ) {
            let status = CoverageStatus::Incomplete(reason);
            let size = match self {
                Self::Existing(query) => canonical::encode(&ReadCoverageV1 {
                    query: query.clone(),
                    required,
                    requests: REQUESTS,
                    pages: PAGES,
                    records: RECORDS,
                    terminal_page: false,
                    status,
                })?
                .len(),
                Self::ReviewedSubnetRoutes(query) => {
                    canonical::encode(&ReviewedSubnetRouteCoverageV1 {
                        schema_version: 1,
                        query: query.clone(),
                        required,
                        requests: REQUESTS,
                        pages: PAGES,
                        records: RECORDS,
                        terminal_page: false,
                        status,
                    })?
                    .len()
                }
            };
            maximum = maximum.max(size);
        }
        Ok(maximum)
    }
}

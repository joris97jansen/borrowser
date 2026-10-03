//! EC2 records and an inert mixed-version carrier. Historical schemas stay closed.
use super::{
    coverage::*,
    ec2_observation_v4::ObservationDataV4,
    evidence_v4::*,
    inventory::ResourceIdentity,
    limits::*,
    observation_v2::{ObservationDataV2, ObservationRecordV2},
    observation_v3::ObservationRecordV3,
};
use crate::{Result, canonical, identity::ReconciliationContextDigest, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecordV4 {
    pub schema_version: u64,
    pub query: QueryIdentityV1,
    pub data: ObservationDataV4,
}
impl ObservationRecordV4 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(self.schema_version == 4, "observation version")?;
        self.query.validate()?;
        use ObservationDataV4 as D;
        use ReadOperationV1 as O;
        use ResourceIdentity as R;
        let exact = |predicate: fn(&R) -> bool| {
            matches!(&self.query.scope,
            QueryScopeV1::Exact { identities } if identities.iter().all(predicate))
        };
        let compatible = match (&self.data, self.query.operation) {
            (D::Region { .. }, O::DescribeRegions)
            | (D::AvailabilityZone { .. }, O::DescribeAvailabilityZones) => {
                matches!(self.query.scope, QueryScopeV1::Regional)
            }
            (D::Subnet { .. }, O::DescribeSubnets) => exact(|v| matches!(v, R::Subnet(_))),
            (D::Vpc { .. }, O::DescribeVpcs) => exact(|v| matches!(v, R::Vpc(_))),
            (D::SecurityGroup { .. }, O::DescribeSecurityGroups) => {
                exact(|v| matches!(v, R::SecurityGroup(_)))
            }
            (D::RouteTable { .. }, O::DescribeRouteTables) => {
                exact(|v| matches!(v, R::RouteTable(_)))
            }
            (D::Endpoint { .. }, O::DescribeVpcEndpoints) => exact(|v| matches!(v, R::Endpoint(_))),
            (D::PrefixList { .. }, O::DescribePrefixLists) => {
                exact(|v| matches!(v, R::PrefixList(_)))
            }
            (D::Dhcp { .. }, O::DescribeDhcpOptions) => exact(|v| matches!(v, R::Dhcp(_))),
            (D::Nacl { .. }, O::DescribeNetworkAcls) => exact(|v| matches!(v, R::Nacl(_))),
            (D::Dns { .. }, O::DescribeVpcAttribute) => {
                matches!(self.query.scope, QueryScopeV1::VpcAttribute { .. })
            }
            _ => false,
        };
        require(compatible, "EC2 observation query scope")?;
        self.data.validate()?;
        ObservationAccounting::default().canonical_record(self)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= RECORD_BYTES, "observation record bytes")?;
        let value: Self = canonical::decode(bytes)?;
        value.canonical_bytes()?;
        Ok(value)
    }
    pub fn identity(&self) -> Result<EvidenceIdentityV4> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV4 {
            kind: EvidenceKindV4::Observation,
            schema_version: 4,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
}
#[derive(Clone, Debug)]
pub enum ObservationEntryV4 {
    V2(Box<ObservationRecordV2>),
    V3(Box<ObservationRecordV3>),
    V4(Box<ObservationRecordV4>),
}
impl ObservationEntryV4 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Self::V2(v) => {
                require(
                    !matches!(
                        v.data,
                        ObservationDataV2::Key { .. }
                            | ObservationDataV2::Profile { .. }
                            | ObservationDataV2::Region { .. }
                            | ObservationDataV2::AvailabilityZone { .. }
                            | ObservationDataV2::Subnet { .. }
                            | ObservationDataV2::Vpc { .. }
                            | ObservationDataV2::SecurityGroup { .. }
                            | ObservationDataV2::RouteTable { .. }
                            | ObservationDataV2::Endpoint { .. }
                            | ObservationDataV2::PrefixList { .. }
                            | ObservationDataV2::Dns { .. }
                            | ObservationDataV2::Dhcp { .. }
                            | ObservationDataV2::Nacl { .. }
                    ),
                    "superseded observation",
                )?;
                v.canonical_bytes()
            }
            Self::V3(v) => v.canonical_bytes(),
            Self::V4(v) => v.canonical_bytes(),
        }
    }
    pub fn query(&self) -> &QueryIdentityV1 {
        match self {
            Self::V2(v) => &v.query,
            Self::V3(v) => &v.query,
            Self::V4(v) => &v.query,
        }
    }
    pub fn minimum_occurrences(&self) -> Result<u64> {
        match self {
            Self::V2(_) => Ok(1),
            Self::V3(v) => Ok(v.data.minimum_occurrences()),
            Self::V4(v) => v.data.minimum_occurrences(),
        }
    }
}
/// Inert, nonserializable aggregate. Coverage remains V1; no query coordinator or policy.
#[derive(Clone, Debug)]
pub struct ProviderObservationV4 {
    pub context: ReconciliationContextDigest,
    pub records: Vec<ObservationEntryV4>,
    pub coverage: Vec<ReadCoverageV1>,
}
impl ProviderObservationV4 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.records.len() <= RECORDS as usize && self.coverage.len() <= REQUESTS as usize,
            "observation collection bound",
        )?;
        let mut accounting = ObservationAccounting::default();
        let mut previous: Option<Vec<u8>> = None;
        for record in &self.records {
            let bytes = record.canonical_bytes()?;
            require(
                previous.as_ref().is_none_or(|p| p <= &bytes),
                "observation canonical order",
            )?;
            previous = Some(bytes);
            match record {
                ObservationEntryV4::V2(v) => accounting.canonical_record(v)?,
                ObservationEntryV4::V3(v) => accounting.canonical_record(v)?,
                ObservationEntryV4::V4(v) => accounting.canonical_record(v)?,
            };
        }
        let mut requests = 0u64;
        let mut records = 0u64;
        let mut queries = std::collections::BTreeMap::new();
        for coverage in &self.coverage {
            coverage.validate()?;
            require(
                queries
                    .insert(canonical::encode(&coverage.query)?, coverage.records)
                    .is_none(),
                "duplicate query coverage",
            )?;
            requests = requests
                .checked_add(coverage.requests)
                .ok_or(crate::Error("coverage arithmetic"))?;
            records = records
                .checked_add(coverage.records)
                .ok_or(crate::Error("coverage arithmetic"))?;
            accounting.canonical_record(coverage)?;
        }
        require(
            requests <= REQUESTS && records <= RECORDS,
            "aggregate coverage counts",
        )?;
        for record in &self.records {
            let credit = queries
                .get_mut(&canonical::encode(record.query())?)
                .ok_or(crate::Error("observation without coverage"))?;
            *credit = credit
                .checked_sub(record.minimum_occurrences()?)
                .ok_or(crate::Error("query occurrence credit"))?;
        }
        Ok(())
    }
}

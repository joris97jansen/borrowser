//! V5 allocation records and an inert mixed carrier. Historical contracts stay frozen.
use super::{
    coverage::*,
    ec2_allocation_observation_v5::ObservationDataV5,
    ec2_observation_v4::FactsV4,
    evidence_v5::*,
    inventory::ResourceIdentity as R,
    limits::*,
    observation_v2::{ObservationDataV2, ObservationRecordV2},
    observation_v3::ObservationRecordV3,
    observation_v4::ObservationRecordV4,
    source_occurrence_v5::{SourceCreditV5, SourceOccurrenceV5},
};
use crate::{Error, Result, canonical, identity::ReconciliationContextDigest, require};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Additional allocation authority restrictions; never changes QueryIdentityV1 validation.
pub(crate) fn validate_allocation_query(query: &QueryIdentityV1) -> Result<()> {
    query.validate()?;
    use QueryScopeV1 as S;
    use ReadOperationV1 as O;
    let exact = |predicate: fn(&R) -> bool| matches!(&query.scope,S::Exact { identities } if identities.iter().all(predicate));
    let discovery = matches!(
        query.scope,
        S::OperationTags { .. } | S::AuthorityTag { .. } | S::OperationTag { .. }
    );
    let valid = match query.operation {
        O::DescribeImages => exact(|r| matches!(r, R::Image(_))),
        O::DescribeInstanceTypes => {
            exact(|r| matches!(r, R::InstanceType(_)))
                && matches!(&query.scope,S::Exact { identities } if identities.len() <= 100)
        }
        O::DescribeInstanceTypeOfferings => matches!(query.scope, S::TypeOffering { .. }),
        O::DescribeIamInstanceProfileAssociations => {
            exact(|r| matches!(r, R::ProfileAssociation(_)))
                || matches!(query.scope, S::AttachedTo { .. })
        }
        O::DescribeInstances => {
            exact(|r| matches!(r, R::Instance(_)))
                || discovery
                || matches!(query.scope, S::ClientToken { .. })
        }
        O::DescribeNetworkInterfaces => {
            exact(|r| matches!(r, R::NetworkInterface(_)))
                || discovery
                || matches!(query.scope, S::AttachedTo { .. })
        }
        O::DescribeVolumes => {
            exact(|r| matches!(r, R::Volume(_)))
                || discovery
                || matches!(query.scope, S::AttachedTo { .. })
        }
        O::DescribeInstanceAttribute => matches!(query.scope, S::InstanceAttribute { .. }),
        _ => false,
    };
    require(valid, "allocation query operation/scope")
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecordV5 {
    pub schema_version: u64,
    pub query: QueryIdentityV1,
    pub source: SourceOccurrenceV5,
    pub data: ObservationDataV5,
}
impl ObservationRecordV5 {
    pub fn validate(&self) -> Result<()> {
        require(self.schema_version == 5, "observation version")?;
        validate_allocation_query(&self.query)?;
        require(
            self.query.operation == self.source.path.operation(),
            "source operation",
        )?;
        self.data.validate()?;
        let mut credit = SourceCreditV5::default();
        credit.record(self.source, self.data.projection(), self.data.claims())
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        ObservationAccounting::default().canonical_record(self)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= RECORD_BYTES, "observation record bytes")?;
        let record: Self = canonical::decode(bytes)?;
        record.canonical_bytes()?;
        Ok(record)
    }
    pub fn identity(&self) -> Result<EvidenceIdentityV5> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV5 {
            kind: EvidenceKindV5::Observation,
            schema_version: 5,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
    pub(crate) fn failure(&self) -> Result<Option<ReadFailureV1>> {
        let attribute = match self.query.scope {
            QueryScopeV1::InstanceAttribute { attribute, .. } => Some(attribute),
            _ => None,
        };
        let facts = self.data.facts()?.failure;
        Ok(facts.or(self.data.required_failure(attribute)))
    }
}

#[derive(Clone, Debug)]
pub enum ObservationEntryV5 {
    V2(Box<ObservationRecordV2>),
    V3(Box<ObservationRecordV3>),
    V4(Box<ObservationRecordV4>),
    V5(Box<ObservationRecordV5>),
}
impl ObservationEntryV5 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Self::V2(v) => {
                require(
                    matches!(
                        v.data,
                        ObservationDataV2::Caller { .. } | ObservationDataV2::Bucket { .. }
                    ),
                    "superseded observation",
                )?;
                v.canonical_bytes()
            }
            Self::V3(v) => v.canonical_bytes(),
            Self::V4(v) => v.canonical_bytes(),
            Self::V5(v) => v.canonical_bytes(),
        }
    }
    pub fn query(&self) -> &QueryIdentityV1 {
        match self {
            Self::V2(v) => &v.query,
            Self::V3(v) => &v.query,
            Self::V4(v) => &v.query,
            Self::V5(v) => &v.query,
        }
    }
    /// Per-record lower bound only. V5 aggregates must use SourceCreditV5 for shared prefixes.
    pub fn minimum_occurrences(&self) -> Result<u64> {
        match self {
            Self::V2(_) => Ok(1),
            Self::V3(v) => Ok(v.data.minimum_occurrences()),
            Self::V4(v) => v.data.minimum_occurrences(),
            Self::V5(v) => {
                let mut c = SourceCreditV5::default();
                c.record(v.source, v.data.projection(), v.data.claims())?;
                c.minimum()
            }
        }
    }
}

/// No Serialize/Deserialize: not an inventory, publication or replay format.
#[derive(Clone, Debug)]
pub struct ProviderObservationV5 {
    pub context: ReconciliationContextDigest,
    pub records: Vec<ObservationEntryV5>,
    pub coverage: Vec<ReadCoverageV1>,
}
impl ProviderObservationV5 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.records.len() <= RECORDS as usize && self.coverage.len() <= REQUESTS as usize,
            "observation collection bound",
        )?;
        let mut accounting = ObservationAccounting::default();
        let mut queries = BTreeMap::new();
        let (mut requests, mut occurrences) = (0u64, 0u64);
        for c in &self.coverage {
            c.validate()?;
            require(
                queries
                    .insert(
                        canonical::encode(&c.query)?,
                        (c, 0u64, SourceCreditV5::default()),
                    )
                    .is_none(),
                "duplicate query coverage",
            )?;
            requests = requests
                .checked_add(c.requests)
                .ok_or(Error("coverage arithmetic"))?;
            occurrences = occurrences
                .checked_add(c.records)
                .ok_or(Error("coverage arithmetic"))?;
            accounting.canonical_record(c)?;
        }
        require(
            requests <= REQUESTS && occurrences <= RECORDS,
            "aggregate coverage counts",
        )?;
        let mut previous = None;
        for record in &self.records {
            let bytes = record.canonical_bytes()?;
            require(
                previous.as_ref().is_none_or(|p| p <= &bytes),
                "observation canonical order",
            )?;
            previous = Some(bytes);
            let (coverage, legacy, credit) = queries
                .get_mut(&canonical::encode(record.query())?)
                .ok_or(Error("observation without coverage"))?;
            match record {
                ObservationEntryV5::V2(v) => {
                    accounting.canonical_record(v)?;
                }
                ObservationEntryV5::V3(v) => {
                    accounting.canonical_record(v)?;
                }
                ObservationEntryV5::V4(v) => {
                    accounting.canonical_record(v)?;
                }
                ObservationEntryV5::V5(v) => {
                    accounting.canonical_record(v)?;
                    require(
                        u64::from(v.source.page) <= coverage.pages,
                        "source page beyond coverage",
                    )?;
                    require(
                        v.failure()?.is_none()
                            || !matches!(coverage.status, CoverageStatus::Complete),
                        "incomplete allocation evidence",
                    )?;
                    credit.record(v.source, v.data.projection(), v.data.claims())?;
                }
            }
            if !matches!(record, ObservationEntryV5::V5(_)) {
                *legacy = legacy
                    .checked_add(record.minimum_occurrences()?)
                    .ok_or(Error("legacy occurrence arithmetic"))?;
            }
        }
        for (_, (coverage, legacy, credit)) in queries {
            require(
                legacy
                    .checked_add(credit.minimum()?)
                    .is_some_and(|n| n <= coverage.records),
                "query occurrence credit",
            )?;
        }
        Ok(())
    }
}

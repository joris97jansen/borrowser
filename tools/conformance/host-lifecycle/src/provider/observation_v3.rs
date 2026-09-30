//! Narrow identity-service successor records; unrelated observations keep their V2 identity.
use super::{
    coverage::*,
    evidence_v3::*,
    identity_observation_v3::*,
    inventory::ResourceIdentity,
    limits::*,
    management_observation_v2::ObservationValueV2,
    observation_v2::{ObservationDataV2, ObservationRecordV2},
};
use crate::{Result, canonical, identity::ReconciliationContextDigest, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ObservationDataV3 {
    Key {
        metadata: ObservationValueV2<KmsKeyEvidenceV3>,
    },
    Profile {
        profile: ObservationValueV2<IamProfileEvidenceV3>,
    },
}
impl ObservationDataV3 {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Key {
                metadata: ObservationValueV2::Present(v),
            } => v.validate(),
            Self::Profile {
                profile: ObservationValueV2::Present(v),
            } => v.validate(),
            _ => Ok(()),
        }
    }
    /// One enclosing response occurrence plus every returned role, including empty/duplicate roles.
    pub fn minimum_occurrences(&self) -> u64 {
        match self {
            Self::Profile {
                profile:
                    ObservationValueV2::Present(IamProfileEvidenceV3 {
                        roles: ObservationValueV2::Present(roles),
                        ..
                    }),
            } => 1 + roles.as_slice().len() as u64,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecordV3 {
    pub schema_version: u64,
    pub query: QueryIdentityV1,
    pub data: ObservationDataV3,
}
impl ObservationRecordV3 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(self.schema_version == 3, "observation version")?;
        self.query.validate()?;
        let compatible = match (&self.data, self.query.operation, &self.query.scope) {
            (
                ObservationDataV3::Key { .. },
                ReadOperationV1::DescribeKey,
                QueryScopeV1::Exact { identities },
            ) => matches!(identities.as_slice(), [ResourceIdentity::Key(_)]),
            (
                ObservationDataV3::Profile { .. },
                ReadOperationV1::GetInstanceProfile,
                QueryScopeV1::Exact { identities },
            ) => matches!(identities.as_slice(), [ResourceIdentity::Profile(_)]),
            _ => false,
        };
        require(compatible, "identity observation query scope")?;
        self.data.validate()?;
        ObservationAccounting::default().canonical_record(self)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= RECORD_BYTES, "observation record bytes")?;
        let value: Self = canonical::decode(bytes)?;
        value.canonical_bytes()?;
        Ok(value)
    }
    pub fn identity(&self) -> Result<EvidenceIdentityV3> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV3 {
            kind: EvidenceKindV3::Observation,
            schema_version: 3,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
}

/// No serde wrapper: each record is hashed/accounted in its original version.
#[derive(Clone, Debug)]
pub enum ObservationEntryV3 {
    V2(Box<ObservationRecordV2>),
    V3(Box<ObservationRecordV3>),
}
impl ObservationEntryV3 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Self::V2(v) => {
                require(
                    !matches!(
                        v.data,
                        ObservationDataV2::Key { .. } | ObservationDataV2::Profile { .. }
                    ),
                    "superseded identity observation",
                )?;
                v.canonical_bytes()
            }
            Self::V3(v) => v.canonical_bytes(),
        }
    }
    pub fn query(&self) -> &QueryIdentityV1 {
        match self {
            Self::V2(v) => &v.query,
            Self::V3(v) => &v.query,
        }
    }
    fn minimum_occurrences(&self) -> u64 {
        match self {
            Self::V2(_) => 1,
            Self::V3(v) => v.data.minimum_occurrences(),
        }
    }
}

/// Inert, nonserializable aggregate. Coverage remains V1; no query coordinator or policy.
#[derive(Clone, Debug)]
pub struct ProviderObservationV3 {
    pub context: ReconciliationContextDigest,
    pub records: Vec<ObservationEntryV3>,
    pub coverage: Vec<ReadCoverageV1>,
}
impl ProviderObservationV3 {
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
                ObservationEntryV3::V2(v) => accounting.canonical_record(v)?,
                ObservationEntryV3::V3(v) => accounting.canonical_record(v)?,
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
                .checked_sub(record.minimum_occurrences())
                .ok_or(crate::Error("query occurrence credit"))?;
        }
        Ok(())
    }
}

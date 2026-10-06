//! Bound retained inputs and joint evidence ingestion. No live clocks or provider I/O.
use super::preflight::*;
use crate::provider::{
    context_v3::ReconciliationContextV3, limits::*, manifest::ReviewedInfrastructureV1,
    observation_v5::*, reviewed_subnet_routes_v1::*,
};
use crate::{Error, Result, canonical, dispatch::PreparedLaunchV2, identity::*, require};

#[derive(Clone, Debug)]
pub struct PriorDiscoveryFacts {
    pub state: ProviderStateDigest,
    pub resources: Vec<super::relationships::AllocationIdentity>,
    pub relationships: Vec<(
        super::relationships::AllocationIdentity,
        super::relationships::AllocationIdentity,
    )>,
    /// Supplied resolution status; resolving or authenticating old references belongs to e3.
    pub fully_supplied: bool,
}
pub struct DiscoveryInputs<'a> {
    pub(crate) context: &'a ReconciliationContextV3,
    pub(crate) prepared: &'a PreparedLaunchV2,
    pub(crate) manifest: ReviewedInfrastructureV1,
    pub(crate) prior: Option<&'a PriorDiscoveryFacts>,
}
impl<'a> DiscoveryInputs<'a> {
    pub fn new(
        context: &'a ReconciliationContextV3,
        prepared: &'a PreparedLaunchV2,
        manifest: &[u8],
        prior: Option<&'a PriorDiscoveryFacts>,
    ) -> Result<Self> {
        // Every independently retained artifact is bounded before validators encode it.
        canonical_size(&prepared.deployment, canonical::EVENT_BYTES)?;
        canonical_size(&prepared.approval, canonical::EVENT_BYTES)?;
        canonical_size(&prepared.trust, canonical::EVENT_BYTES)?;
        canonical_size(&prepared.specification, canonical::EVENT_BYTES)?;
        canonical_size(&prepared.request, canonical::EVENT_BYTES)?;
        canonical_size(&prepared.authorization, canonical::EVENT_BYTES)?;
        let manifest = ReviewedInfrastructureV1::parse_bound(manifest, &prepared.deployment)?;
        require(
            prepared.binding()? == context.fields().binding
                && prepared.references()? == context.fields().artifacts
                && prepared.deployment.marker()? == context.fields().root
                && manifest.digest()? == context.fields().manifest,
            "discovery retained binding",
        )?;
        if let Some(prior) = prior {
            require(
                prior.resources.len() <= RECORDS as usize
                    && prior.relationships.len() <= RECORDS as usize,
                "prior discovery collection bound",
            )?;
            let bytes = prior
                .resources
                .iter()
                .chain(prior.relationships.iter().flat_map(|(a, b)| [a, b]))
                .try_fold(0, |n, id| Ok::<_, Error>(n + id.logical_bytes()))?;
            require(bytes <= 64 << 10, "prior discovery byte bound")?;
            require(
                context
                    .fields()
                    .prior_provider
                    .as_ref()
                    .is_some_and(|p| p.state == prior.state),
                "prior discovery state binding",
            )?;
        }
        Ok(Self {
            context,
            prepared,
            manifest,
            prior,
        })
    }
    pub fn context_identity(&self) -> Result<ReconciliationContextDigest> {
        self.context.identity()
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveryEvidence {
    pub observations: ProviderObservationV5,
    pub reviewed_subnet_routes: Option<ReviewedSubnetRouteEvidenceV1>,
}
#[derive(Debug)]
pub struct RejectedDiscoveryEvidence {
    pub error: Error,
    pub evidence: DiscoveryEvidence,
}
#[derive(Debug)]
pub struct ValidatedDiscoveryEvidence {
    pub(crate) evidence: DiscoveryEvidence,
    pub(crate) retained_bytes: u64,
    pub(crate) reservations: u64,
}
impl ValidatedDiscoveryEvidence {
    pub fn evidence(&self) -> &DiscoveryEvidence {
        &self.evidence
    }
    pub fn into_evidence(self) -> DiscoveryEvidence {
        self.evidence
    }
}

fn entry_size(v: &ObservationEntryV5) -> Result<usize> {
    match v {
        ObservationEntryV5::V2(v) => canonical_size(v, RECORD_BYTES),
        ObservationEntryV5::V3(v) => canonical_size(v, RECORD_BYTES),
        ObservationEntryV5::V4(v) => canonical_size(v, RECORD_BYTES),
        ObservationEntryV5::V5(v) => canonical_size(v, RECORD_BYTES),
    }
}
/// Takes ownership so sorting moves records rather than cloning provider payloads.
/// Even rejected input is returned intact (possibly with normalized outer order).
pub fn ingest(
    inputs: &DiscoveryInputs<'_>,
    mut evidence: DiscoveryEvidence,
) -> std::result::Result<ValidatedDiscoveryEvidence, Box<RejectedDiscoveryEvidence>> {
    match validate(inputs, &mut evidence) {
        Ok((retained_bytes, reservations)) => Ok(ValidatedDiscoveryEvidence {
            evidence,
            retained_bytes,
            reservations,
        }),
        Err(error) => Err(Box::new(RejectedDiscoveryEvidence { error, evidence })),
    }
}
fn validate(inputs: &DiscoveryInputs<'_>, evidence: &mut DiscoveryEvidence) -> Result<(u64, u64)> {
    let records = evidence.observations.records.len()
        + evidence
            .reviewed_subnet_routes
            .as_ref()
            .map_or(0, |v| v.records.len());
    let queries = evidence.observations.coverage.len()
        + usize::from(evidence.reviewed_subnet_routes.is_some());
    require(
        records <= RECORDS as usize && queries <= REQUESTS as usize,
        "combined discovery collection bound",
    )?;
    // No canonical key, JSON value, index, or evidence clone exists before this pass.
    let mut total = 0usize;
    let mut retained = 0usize;
    let mut sizes = Vec::with_capacity(evidence.observations.records.len());
    for entry in &evidence.observations.records {
        let size = entry_size(entry)?;
        sizes.push(size);
        total += size;
        retained += size;
        require(
            total <= NORMALIZED_BYTES as usize,
            "combined discovery bytes",
        )?;
    }
    for c in &evidence.observations.coverage {
        total += canonical_size(c, RECORD_BYTES)?;
        require(
            total <= NORMALIZED_BYTES as usize,
            "combined discovery bytes",
        )?;
    }
    if let Some(routes) = &evidence.reviewed_subnet_routes {
        for record in &routes.records {
            let size = canonical_size(record, RECORD_BYTES)?;
            total += size;
            retained += size;
            require(
                total <= NORMALIZED_BYTES as usize,
                "combined discovery bytes",
            )?;
        }
        total += canonical_size(&routes.coverage, RECORD_BYTES)?;
        require(
            total <= NORMALIZED_BYTES as usize,
            "combined discovery bytes",
        )?;
    }
    let mut keys = Vec::with_capacity(sizes.len());
    for (entry, size) in evidence.observations.records.iter().zip(sizes) {
        let bytes = entry.canonical_bytes()?;
        require(bytes.len() == size, "discovery canonical size disagreement")?;
        keys.push(bytes);
    }
    let mut entries: Vec<_> = keys
        .into_iter()
        .zip(std::mem::take(&mut evidence.observations.records))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    evidence.observations.records = entries.into_iter().map(|(_, v)| v).collect();
    let mut coverage = Vec::new();
    for c in &evidence.observations.coverage {
        let size = canonical_size(c, RECORD_BYTES)?;
        checked_encoding(c, size)?;
        coverage.push(canonical::encode(&c.query)?);
    }
    let mut coverage: Vec<_> = coverage
        .into_iter()
        .zip(std::mem::take(&mut evidence.observations.coverage))
        .collect();
    coverage.sort_by(|a, b| a.0.cmp(&b.0));
    evidence.observations.coverage = coverage.into_iter().map(|(_, v)| v).collect();
    evidence.observations.validate()?;
    let context = inputs.context_identity()?;
    require(
        evidence.observations.context == context,
        "discovery evidence context",
    )?;
    let binding = &inputs.context.fields().binding;
    let (mut requests, mut sources, mut reservations) = (0, 0, 0);
    for c in &evidence.observations.coverage {
        require(
            c.records == 0 || c.pages > 0,
            "source credit without executed page",
        )?;
        require(
            c.status != crate::provider::coverage::CoverageStatus::Complete
                || c.pages == c.requests,
            "complete query with unmatched attempts",
        )?;
        require(
            c.query.account == binding.account_id && c.query.region == binding.region,
            "discovery query account/region",
        )?;
        requests += c.requests;
        sources += c.records;
        reservations += DiscoveryQuery::Existing(c.query.clone()).reservation(c.required)? as u64;
    }
    if let Some(routes) = &mut evidence.reviewed_subnet_routes {
        let c = &routes.coverage;
        c.validate()?;
        require(
            c.records == 0 || c.pages > 0,
            "route source credit without executed page",
        )?;
        require(
            c.status != crate::provider::coverage::CoverageStatus::Complete
                || c.pages == c.requests,
            "complete route query with unmatched attempts",
        )?;
        require(
            routes.context == context
                && c.query.account == binding.account_id
                && c.query.region == binding.region
                && c.query.association_subnet == inputs.manifest.subnet,
            "reviewed subnet route binding",
        )?;
        requests += c.requests;
        sources += c.records;
        reservations +=
            DiscoveryQuery::ReviewedSubnetRoutes(c.query.clone()).reservation(c.required)? as u64;
        let mut keyed = Vec::new();
        for record in &routes.records {
            require(record.query == c.query, "route record coverage")?;
            let bytes = record.canonical_bytes()?;
            require(
                bytes.len() == canonical_size(record, RECORD_BYTES)?,
                "route canonical size disagreement",
            )?;
        }
        for record in std::mem::take(&mut routes.records) {
            keyed.push((record.canonical_bytes()?, record));
        }
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        routes.records = keyed.into_iter().map(|(_, v)| v).collect();
        checked_encoding(c, canonical_size(c, RECORD_BYTES)?)?;
    }
    require(
        requests <= REQUESTS && sources <= RECORDS,
        "combined discovery coverage bound",
    )?;
    Ok((retained as u64, reservations))
}

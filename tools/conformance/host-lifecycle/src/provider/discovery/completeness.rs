//! Execution, representation, closure and final validation are independent gates.
use super::*;
use crate::{
    Result,
    provider::{
        coverage::*, ec2_observation_v4::FactsV4, limits::*,
        reviewed_subnet_routes_v1::DiscoveryQuery,
    },
    require,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkStatus {
    Complete,
    Missing,
    RejectedBeforeAdmission(ReadFailureV1),
    Incomplete(ReadFailureV1),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservedQueryIndex {
    Existing(usize),
    ReviewedSubnetRoutes,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryReport {
    pub required: requirements::RequiredReads,
    pub execution: BTreeMap<usize, WorkStatus>,
    pub representation: BTreeMap<ObservedQueryIndex, representation::RepresentationAudit>,
    pub relationships: relationships::Relationships,
    pub accounting_gap: bool,
    pub metadata_exhausted: bool,
    pub metadata_bytes: usize,
    pub final_check: FinalRoundCheck,
    pub complete: bool,
    exclusion_candidates: std::collections::BTreeSet<relationships::AllocationIdentity>,
}
pub struct PreparedDiscoveryReport {
    report: DiscoveryReport,
    before_final: bool,
}
impl PreparedDiscoveryReport {
    /// Constant-size finalization, after all substantive derivation and allocation.
    pub fn finalize(mut self, check: FinalRoundCheck) -> DiscoveryReport {
        self.report.final_check = check;
        self.report.complete = self.before_final && check == FinalRoundCheck::Passed;
        self.report
    }
}
impl DiscoveryReport {
    pub fn next_pending(&self) -> Option<&requirements::RequiredRead> {
        self.required
            .reads
            .iter()
            .enumerate()
            .filter(|(i, _)| self.execution.get(i) == Some(&WorkStatus::Missing))
            .min_by_key(|(_, (key, read))| (read.stage, *key, read.observations.first().copied()))
            .map(|(_, (_, read))| read)
    }
    /// Exclusion candidates are effective only when the final four gates pass.
    pub fn attribution(
        &self,
        id: &relationships::AllocationIdentity,
    ) -> Option<relationships::Attribution> {
        self.relationships.resources.get(id).map(|r| {
            if self.complete && self.exclusion_candidates.contains(id) {
                relationships::Attribution::AffirmativelyUnrelated
            } else {
                r.attribution
            }
        })
    }
    pub fn plausible_instances(&self) -> impl Iterator<Item = &crate::identity::InstanceId> {
        self.relationships
            .resources
            .iter()
            .filter_map(|(id, _)| match id {
                relationships::AllocationIdentity::Instance(v)
                    if self.attribution(id)
                        != Some(relationships::Attribution::AffirmativelyUnrelated) =>
                {
                    Some(v)
                }
                _ => None,
            })
    }
}

pub fn derive(
    inputs: &DiscoveryInputs<'_>,
    evidence: &ValidatedDiscoveryEvidence,
    facts: &DiscoveryExecutionFacts,
) -> Result<PreparedDiscoveryReport> {
    require(
        evidence.evidence.observations.context == inputs.context_identity()?,
        "derivation context binding",
    )?;
    validate_execution(evidence, facts)?;
    let mut graph = relationships::derive(inputs, evidence);
    let required = requirements::derive_required_reads(inputs, &mut graph)?;
    let mut representation = BTreeMap::new();
    let (mut requests, mut sources) = (0, 0);
    for (index, coverage) in evidence.evidence.observations.coverage.iter().enumerate() {
        let audit = representation::audit_query(
            coverage,
            evidence
                .evidence
                .observations
                .records
                .iter()
                .filter(|v| v.query() == &coverage.query),
        )?;
        if graph.budget.charge(32 + 16 * audit.gaps.len()) {
            representation.insert(ObservedQueryIndex::Existing(index), audit);
        }
        requests += coverage.requests;
        sources += coverage.records;
    }
    let mut outputs = evidence.evidence.observations.records.len() as u64;
    if let Some(routes) = &evidence.evidence.reviewed_subnet_routes {
        let c = &routes.coverage;
        let mut count = 0;
        for record in &routes.records {
            let summary = record.data.facts()?;
            require(
                summary.failure.is_none() || c.status != CoverageStatus::Complete,
                "complete route coverage with failed evidence",
            )?;
            count += summary.occurrences;
        }
        require(count <= c.records, "route sources exceed coverage")?;
        let mut gaps = std::collections::BTreeSet::new();
        if count != c.records {
            gaps.insert(representation::RepresentationGap::UnrepresentedSourceOccurrences);
        }
        if graph.budget.charge(32 + 16 * gaps.len()) {
            representation.insert(
                ObservedQueryIndex::ReviewedSubnetRoutes,
                representation::RepresentationAudit {
                    represented_sources: Some(count),
                    gaps,
                },
            );
        }
        requests += c.requests;
        sources += c.records;
        outputs += routes.records.len() as u64;
    }
    require(
        facts.accounting.requests >= requests
            && facts.accounting.source_occurrences >= sources
            && facts.accounting.retained_outputs >= outputs,
        "accounting understates supplied evidence",
    )?;
    let accounting_gap = facts.accounting.requests != requests
        || facts.accounting.source_occurrences != sources
        || facts.accounting.retained_outputs != outputs;
    let mut execution = BTreeMap::new();
    for (index, read) in required.reads.values().enumerate() {
        let coverage = match &read.query {
            DiscoveryQuery::Existing(q) => evidence
                .evidence
                .observations
                .coverage
                .iter()
                .find(|c| &c.query == q)
                .map(|c| &c.status),
            DiscoveryQuery::ReviewedSubnetRoutes(q) => evidence
                .evidence
                .reviewed_subnet_routes
                .as_ref()
                .filter(|r| &r.coverage.query == q)
                .map(|r| &r.coverage.status),
        };
        let state = coverage
            .map(|s| match s {
                CoverageStatus::Complete => WorkStatus::Complete,
                CoverageStatus::Incomplete(reason) => WorkStatus::Incomplete(*reason),
            })
            .unwrap_or_else(|| {
                facts
                    .rejected_before_admission
                    .iter()
                    .find(|v| v.query == read.query)
                    .map_or(WorkStatus::Missing, |v| {
                        WorkStatus::RejectedBeforeAdmission(v.reason)
                    })
            });
        if graph.budget.charge(24) {
            execution.insert(index, state);
        }
    }
    let metadata_exhausted = graph.budget.exhausted()
        || graph.exhausted
        || required.exhausted
        || facts.stop == CoordinatorStop::MetadataExhausted
        || representation.values().any(|a| {
            a.gaps
                .contains(&representation::RepresentationGap::AuditBudgetExhausted)
        });
    let before_final = !metadata_exhausted
        && !accounting_gap
        && !graph.unresolved_references
        && facts.stop == CoordinatorStop::Quiescent
        && execution.values().all(|v| *v == WorkStatus::Complete)
        && representation
            .values()
            .all(representation::RepresentationAudit::complete);
    let exclusion_candidates = if before_final {
        relationships::exclude_foreign(inputs, evidence, &mut graph)
    } else {
        Default::default()
    };
    Ok(PreparedDiscoveryReport {
        before_final: before_final
            && !graph.budget.exhausted()
            && facts.accounting.failure.is_none(),
        report: DiscoveryReport {
            required,
            execution,
            representation,
            metadata_bytes: graph.budget.used(),
            metadata_exhausted: metadata_exhausted || graph.budget.exhausted(),
            relationships: graph,
            accounting_gap,
            final_check: FinalRoundCheck::Pending,
            complete: false,
            exclusion_candidates,
        },
    })
}
fn validate_execution(
    evidence: &ValidatedDiscoveryEvidence,
    facts: &DiscoveryExecutionFacts,
) -> Result<()> {
    require(
        facts.rejected_before_admission.len() <= 128,
        "rejected work bound",
    )?;
    // Check query bytes before encoding keys or cloning any rejected-work metadata.
    let mut bytes = 0;
    for v in &facts.rejected_before_admission {
        bytes += match &v.query {
            DiscoveryQuery::Existing(q) => preflight::canonical_size(q, RECORD_BYTES)?,
            DiscoveryQuery::ReviewedSubnetRoutes(q) => preflight::canonical_size(q, RECORD_BYTES)?,
        };
        require(bytes <= 64 << 10, "rejected work bytes")?;
    }
    let a = facts.accounting;
    require(
        a.requests <= REQUESTS
            && a.source_occurrences <= RECORDS
            && a.retained_outputs <= RECORDS
            && a.response_bytes <= ROUND_RESPONSE_BYTES
            && a.normalized_bytes <= NORMALIZED_BYTES,
        "discovery accounting bounds",
    )?;
    require(
        a.normalized_bytes >= evidence.retained_bytes + evidence.reservations,
        "accounting understates retained bytes or reservations",
    )?;
    require(
        match facts.final_check {
            FinalRoundCheck::Passed => a.failure.is_none(),
            FinalRoundCheck::Failed(reason) => a.failure == Some(reason),
            FinalRoundCheck::Pending => true,
        },
        "final round/accounting contradiction",
    )?;
    require(
        facts.stop != CoordinatorStop::RoundStopped || a.failure.is_some(),
        "round stop without latch",
    )?;
    // Limit and session failures latch the shared round. Query-local failures
    // (service errors, malformed data, cycles) need not do so. A poisoned round
    // mutex is reported as Clock by the existing snapshot/failure boundary.
    let statuses = evidence
        .evidence
        .observations
        .coverage
        .iter()
        .map(|c| &c.status)
        .chain(
            evidence
                .evidence
                .reviewed_subnet_routes
                .iter()
                .map(|r| &r.coverage.status),
        );
    for status in statuses {
        if let CoverageStatus::Incomplete(reason) = status {
            validate_shared_failure(*reason, a.failure)?;
        }
    }
    let mut rejected = std::collections::BTreeSet::new();
    for v in &facts.rejected_before_admission {
        validate_shared_failure(v.reason, a.failure)?;
        let key = v.query.key()?;
        require(rejected.insert(key), "duplicate rejected work")?;
        let covered = match &v.query {
            DiscoveryQuery::Existing(q) => evidence
                .evidence
                .observations
                .coverage
                .iter()
                .any(|c| &c.query == q),
            DiscoveryQuery::ReviewedSubnetRoutes(q) => evidence
                .evidence
                .reviewed_subnet_routes
                .as_ref()
                .is_some_and(|r| &r.coverage.query == q),
        };
        require(!covered, "rejected-before-admission query has coverage")?;
    }
    Ok(())
}

fn validate_shared_failure(reason: ReadFailureV1, latch: Option<LimitKind>) -> Result<()> {
    let shared = match reason {
        ReadFailureV1::SessionExpired => Some(LimitKind::Session),
        ReadFailureV1::Limit(limit) => Some(limit),
        _ => None,
    };
    require(
        shared.is_none() || shared == latch || latch == Some(LimitKind::Clock),
        "query failure contradicts shared round latch",
    )
}

/// Offline recomputation uses explicit supplied finalization facts, never a clock.
pub fn derive_offline(
    inputs: &DiscoveryInputs<'_>,
    evidence: &ValidatedDiscoveryEvidence,
    facts: &DiscoveryExecutionFacts,
) -> Result<DiscoveryReport> {
    Ok(derive(inputs, evidence, facts)?.finalize(facts.final_check))
}

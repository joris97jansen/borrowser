use crate::{Result, canonical, provider::*, require, scheduling::*};
use crate::{approval::ProductApproval, identity::*, mutation::MutationDescriptor};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "borrowser-host-lifecycle-event";
pub const AUTHORITY: &str = "provider-lifecycle-only";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolIdentityV1 {
    pub package: String,
    pub package_version: String,
    pub schema_version: u64,
    pub source_revision: String,
    pub cargo_lock_sha256: String,
    pub source_clean: bool,
}
impl ToolIdentityV1 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.package == "borrowser-host-lifecycle" && self.schema_version == 1,
            "tool identity",
        )?;
        canonical::text(&self.package_version, 64)?;
        require(
            self.source_revision.len() == 40
                && self
                    .source_revision
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "source revision",
        )?;
        canonical::digest(&self.cargo_lock_sha256)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub account_id: AccountScopeId,
    pub authority: String,
    pub authority_id: AuthorityId,
    pub event: Event,
    pub format: String,
    pub operation_id: Option<OperationId>,
    pub previous_sha256: Option<EventDigest>,
    pub schema_version: u64,
    pub sequence: u64,
    pub time: TimeSample,
    pub tool: ToolIdentityV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub sha256: String,
    pub bytes: u64,
    pub provider_reference: String,
    pub reviewer: String,
    pub rationale: String,
}
impl Evidence {
    pub fn validate(&self) -> Result<()> {
        canonical::digest(&self.sha256)?;
        require(self.bytes > 0 && self.bytes <= 1_048_576, "evidence bound")?;
        canonical::text(&self.provider_reference, 1024)?;
        canonical::text(&self.reviewer, 128)?;
        canonical::text(&self.rationale, 1024)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Event {
    AuthorityInitialized,
    OperationAuthorized {
        request: AllocationRequest,
        authorization: String,
        deadline: Deadline,
    },
    CatalogueObserved {
        quote: Box<CatalogueQuote>,
    },
    BaselineStarted,
    BaselineTransaction {
        transaction: Transaction,
    },
    BaselineServer {
        server_number: ServerNumber,
    },
    BaselineCompleted {
        transactions: u64,
        servers: u64,
    },
    AllocationDispatchIntent {
        descriptor: MutationDescriptor,
    },
    AllocationResponse {
        transaction: Transaction,
    },
    PartialTransactionIdentity {
        id: RobotTransactionId,
        source: TransactionSource,
        failure: ProviderFailure,
    },
    TransactionObserved {
        source: TransactionSource,
        transaction: Transaction,
    },
    ServerObserved {
        server_number: ServerNumber,
        product: String,
        datacenter: String,
        status: ServerStatus,
        cancelled: bool,
    },
    FailureObserved {
        endpoint: EndpointClass,
        failure: ProviderFailure,
    },
    CancellationAuthorized {
        server_number: ServerNumber,
        authorization: String,
    },
    CancellationDispatchIntent {
        descriptor: MutationDescriptor,
    },
    CancellationObserved {
        observation: CancellationObservation,
        readback: bool,
    },
    AllocationResolved {
        transaction: Transaction,
        evidence: Evidence,
    },
    IdentityConflictResolved {
        transaction: Transaction,
        evidence: Evidence,
    },
    NonAllocationResolved {
        evidence: Evidence,
    },
    ReleaseResolved {
        server_number: ServerNumber,
        reservation_absent: bool,
        ipv4_obligation_closed: bool,
        evidence: Evidence,
    },
    BaselineAttributionResolved {
        transaction: Transaction,
        evidence: Evidence,
    },
    BaselineCandidateDisqualified {
        transaction: Transaction,
        evidence: Evidence,
    },
    CancellationRetryResolved {
        server_number: ServerNumber,
        evidence: Evidence,
    },
    HistoryConflictObserved {
        observation: TransactionResponse,
    },
    AuthenticationResolved {
        evidence: Evidence,
    },
    EndpointCharged {
        endpoint: EndpointClass,
    },
    ObservationRoundStarted,
    WatchProgress {
        after: OperationId,
    },
    MutationResponseLost {
        endpoint: EndpointClass,
    },
    ReadSucceeded {
        endpoint: EndpointClass,
    },
    BudgetRebootHold {
        endpoint: EndpointClass,
    },
    ProviderAccessResolved {
        endpoint: EndpointClass,
        evidence: Evidence,
    },
    CancellationResubmissionAuthorized {
        server_number: ServerNumber,
        authorization: String,
    },
}
impl Event {
    pub fn evidence(&self) -> Option<&Evidence> {
        match self {
            Self::AllocationResolved { evidence, .. }
            | Self::IdentityConflictResolved { evidence, .. }
            | Self::NonAllocationResolved { evidence }
            | Self::ReleaseResolved { evidence, .. }
            | Self::AuthenticationResolved { evidence }
            | Self::ProviderAccessResolved { evidence, .. }
            | Self::BaselineAttributionResolved { evidence, .. }
            | Self::BaselineCandidateDisqualified { evidence, .. }
            | Self::CancellationRetryResolved { evidence, .. } => Some(evidence),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AllocationProgress {
    Prepared,
    DispatchRecorded,
    Pending,
    Allocated,
    Rejected,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CancellationProgress {
    NotAuthorized,
    Authorized,
    DispatchRecorded,
    Acknowledged,
    Released,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CancellationReadback {
    None,
    Acknowledged,
    NoCancellationScheduled,
    ReviewedNotScheduled,
    Inconclusive,
}
/// Resource identity is deliberately weaker than a complete observed Transaction snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResourceAttributionIdentity {
    pub transaction_id: RobotTransactionId,
    pub server_number: Option<ServerNumber>,
}
impl Transaction {
    pub fn resource_attribution(&self) -> ResourceAttributionIdentity {
        ResourceAttributionIdentity {
            transaction_id: self.id.clone(),
            server_number: self.server_number,
        }
    }
}
fn canonical_sort<T: Serialize>(values: &mut Vec<T>) -> Result<()> {
    let mut keyed = values
        .drain(..)
        .map(|v| Ok((canonical::encode(&v)?, v)))
        .collect::<Result<Vec<_>>>()?;
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    *values = keyed.into_iter().map(|(_, v)| v).collect();
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BaselineContradiction {
    pub transaction_id: RobotTransactionId,
    pub server_number: Option<ServerNumber>,
    pub transaction_present: bool,
    pub server_present: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReviewedBaselineAttribution {
    pub transaction: Transaction,
    pub disposition: BaselineDisposition,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BaselineDisposition {
    Viable,
    Disqualified,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AllocationUncertainty {
    None,
    RequiresReconciliation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CancellationUncertainty {
    None,
    OutcomeUncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationLimit {
    HistoryRounds,
    TransactionRounds,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationEligibility {
    NotDue,
    Due,
    RequiresOperatorRecovery(ObservationLimit),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "kebab-case")]
pub enum ConflictFact {
    TransactionIdentity(RobotTransactionId),
    Transaction(Transaction),
    Server(ServerObservation),
    ServerCancellation(ServerObservation),
    Cancellation(CancellationObservation),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetainedConflict {
    pub fact: ConflictFact,
    pub resolved: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Operation {
    pub id: OperationId,
    pub request: AllocationRequest,
    pub request_sha256: RequestFingerprint,
    pub deadline: Deadline,
    pub allocation: AllocationProgress,
    pub transaction: Option<Transaction>,
    pub response_transaction_id: Option<RobotTransactionId>,
    pub retained_transaction_ids: Vec<RobotTransactionId>,
    pub product_approval: Option<ProductApproval>,
    pub server_number: Option<ServerNumber>,
    pub server_confirmed: bool,
    pub last_server_observation: Option<ServerObservation>,
    pub cancellation: CancellationProgress,
    pub cancellation_readback: CancellationReadback,
    pub baseline_contradictions: Vec<BaselineContradiction>,
    pub reviewed_baseline_attributions: Vec<ReviewedBaselineAttribution>,
    pub history_conflicts: Vec<TransactionResponse>,
    pub uncertain: bool,
    pub allocation_uncertainty: AllocationUncertainty,
    pub cancellation_uncertainty: CancellationUncertainty,
    pub conflict: bool,
    pub retained_conflicts: Vec<RetainedConflict>,
    pub candidates: Vec<Transaction>,
    pub billing_settled: bool,
    pub next_observation: Option<Deadline>,
    pub observation_rounds: u64,
    pub(crate) baseline_started: bool,
    pub(crate) baseline_complete: bool,
    catalogue_verified: bool,
    baseline_transactions: Vec<Transaction>,
    baseline_servers: Vec<ServerNumber>,
}
impl Operation {
    fn baseline_unresolved(&self, fact: &BaselineContradiction) -> bool {
        let identity = ResourceAttributionIdentity {
            transaction_id: fact.transaction_id.clone(),
            server_number: fact.server_number,
        };
        let snapshots: Vec<_> = self
            .candidates
            .iter()
            .filter(|t| t.resource_attribution() == identity)
            .collect();
        if snapshots.is_empty() {
            // A partial response ID is not a normalized identity-only snapshot.
            return fact.server_number.is_some()
                || !self.reviewed_baseline_attributions.iter().any(|r| {
                    r.transaction.id == fact.transaction_id
                        && r.disposition == BaselineDisposition::Viable
                });
        }
        snapshots.iter().any(|t| {
            !self
                .reviewed_baseline_attributions
                .iter()
                .any(|r| &r.transaction == *t)
        })
    }
    pub fn has_conflict(&self) -> bool {
        self.baseline_contradictions
            .iter()
            .any(|f| self.baseline_unresolved(f))
            || self.has_candidate_ambiguity()
            || !self.history_conflicts.is_empty()
            || self.retained_conflicts.iter().any(|f| !f.resolved)
    }
    fn has_candidate_ambiguity(&self) -> bool {
        self.candidates
            .iter()
            .filter(|t| !self.candidate_disqualified(t))
            .take(2)
            .count()
            > 1
    }
    fn candidate_disqualified(&self, t: &Transaction) -> bool {
        self.reviewed_baseline_attributions
            .iter()
            .any(|r| r.transaction == *t && r.disposition == BaselineDisposition::Disqualified)
    }
    fn retain_candidate(&mut self, t: &Transaction) -> Result<()> {
        if !self.candidates.contains(t) {
            require(self.candidates.len() < 1024, "candidate bound")?;
            self.candidates.push(t.clone());
            canonical_sort(&mut self.candidates)?;
        }
        Ok(())
    }
    fn unique_bindable_candidate(&self) -> Option<&Transaction> {
        if self.has_conflict() {
            return None;
        }
        let mut viable = self
            .candidates
            .iter()
            .filter(|t| !self.candidate_disqualified(t));
        let candidate = viable.next()?;
        if viable.next().is_some()
            || !candidate.compatible(&self.request)
            || !self.reviewed_baseline_attributions.iter().any(|r| {
                r.transaction == *candidate && r.disposition == BaselineDisposition::Viable
            })
            || self
                .transaction
                .as_ref()
                .is_some_and(|t| t.id != candidate.id)
            || self
                .server_number
                .is_some_and(|n| candidate.server_number != Some(n))
        {
            return None;
        }
        Some(candidate)
    }
    /// AllocationResolved itself supplies the reviewed recovery authority. It does
    /// not require an artificial baseline viability review for an empty baseline.
    fn bind_reviewed_recovery(&mut self, t: &Transaction) -> Result<()> {
        t.validate()?;
        require(
            self.candidates.contains(t)
                && !self.candidate_disqualified(t)
                && t.compatible(&self.request)
                && self.transaction.is_none()
                && self.server_number.is_none(),
            "recovery requires exact retained unowned candidate",
        )?;
        self.baseline_conflict(&t.id, t.server_number)?;
        // Preserve the reviewed event, but never turn it into a choice among
        // alternatives. Baseline-free recovery needs a fresh resolution after dispositions.
        if self.has_conflict() {
            self.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
            return Ok(());
        }
        let mut viable = self
            .candidates
            .iter()
            .filter(|c| !self.candidate_disqualified(c));
        require(
            viable.next() == Some(t) && viable.next().is_none(),
            "recovery candidate is not uniquely admissible",
        )?;
        self.bind(t)
    }
    fn review_baseline(
        &mut self,
        transaction: &Transaction,
        disposition: BaselineDisposition,
    ) -> Result<()> {
        if let Some(review) = self
            .reviewed_baseline_attributions
            .iter_mut()
            .find(|r| r.transaction == *transaction)
        {
            require(
                review.disposition != BaselineDisposition::Disqualified
                    || disposition == BaselineDisposition::Disqualified,
                "disqualified candidate cannot be revived implicitly",
            )?;
            review.disposition = disposition;
        } else {
            require(
                self.reviewed_baseline_attributions.len() < 1024,
                "baseline review bound",
            )?;
            self.reviewed_baseline_attributions
                .push(ReviewedBaselineAttribution {
                    transaction: transaction.clone(),
                    disposition,
                });
        }
        canonical_sort(&mut self.reviewed_baseline_attributions)?;
        if let Some(candidate) = self.unique_bindable_candidate().cloned() {
            self.bind(&candidate)?;
        }
        Ok(())
    }
    fn retain_conflict(&mut self, fact: ConflictFact) -> Result<()> {
        if let Some(existing) = self.retained_conflicts.iter_mut().find(|f| f.fact == fact) {
            existing.resolved = false;
        } else {
            require(self.retained_conflicts.len() < 1024, "conflict fact bound")?;
            self.retained_conflicts.push(RetainedConflict {
                fact,
                resolved: false,
            });
            let mut ordered = self
                .retained_conflicts
                .drain(..)
                .map(|f| Ok((canonical::encode(&f.fact)?, f)))
                .collect::<Result<Vec<_>>>()?;
            ordered.sort_by(|a, b| a.0.cmp(&b.0));
            self.retained_conflicts = ordered.into_iter().map(|(_, f)| f).collect();
        }
        Ok(())
    }
    pub fn observation_eligibility(&self, now: &TimeSample) -> ObservationEligibility {
        let known = self.transaction.is_some() || self.response_transaction_id.is_some();
        if self.observation_rounds >= 576 {
            return ObservationEligibility::RequiresOperatorRecovery(
                ObservationLimit::TransactionRounds,
            );
        }
        if !known && self.observation_rounds >= 4 {
            return ObservationEligibility::RequiresOperatorRecovery(
                ObservationLimit::HistoryRounds,
            );
        }
        if self
            .next_observation
            .as_ref()
            .is_some_and(|d| d.boot_id == now.boot_id && d.permits(now))
        {
            ObservationEligibility::NotDue
        } else {
            ObservationEligibility::Due
        }
    }
    pub(crate) fn check_observation_due(&self, now: &TimeSample) -> Result<()> {
        match self.observation_eligibility(now) {
            ObservationEligibility::Due => Ok(()),
            ObservationEligibility::NotDue => Err(crate::Error("observation not due")),
            ObservationEligibility::RequiresOperatorRecovery(_) => Err(crate::Error(
                "observation allowance requires operator recovery",
            )),
        }
    }
    pub fn phase(&self) -> &'static str {
        if self.conflict || self.uncertain {
            return "requires-reconciliation";
        }
        match self.cancellation {
            CancellationProgress::Released => "released",
            CancellationProgress::Authorized
            | CancellationProgress::DispatchRecorded
            | CancellationProgress::Acknowledged => "cancellation-pending",
            CancellationProgress::NotAuthorized => match self.allocation {
                AllocationProgress::Prepared => "prepared",
                AllocationProgress::DispatchRecorded => "requires-reconciliation",
                AllocationProgress::Pending => "allocation-pending",
                AllocationProgress::Allocated => "allocated",
                AllocationProgress::Rejected => "allocation-rejected",
            },
        }
    }
    pub fn closed(&self) -> bool {
        !self.conflict
            && !self.uncertain
            && (self.cancellation == CancellationProgress::Released
                || self.allocation == AllocationProgress::Rejected)
    }
    fn remember_identity(&mut self, id: &RobotTransactionId, response: bool) -> Result<()> {
        if !self.retained_transaction_ids.contains(id) {
            require(
                self.retained_transaction_ids.len() < 1024,
                "retained transaction identity bound",
            )?;
            self.retained_transaction_ids.push(id.clone());
        }
        if response {
            if self
                .response_transaction_id
                .as_ref()
                .is_some_and(|old| old != id)
            {
                self.retain_conflict(ConflictFact::TransactionIdentity(id.clone()))?;
            } else {
                self.response_transaction_id = Some(id.clone());
            }
        }
        Ok(())
    }
    fn baseline_conflict(
        &mut self,
        id: &RobotTransactionId,
        server: Option<ServerNumber>,
    ) -> Result<bool> {
        let transaction_present =
            self.baseline_complete && self.baseline_transactions.iter().any(|t| &t.id == id);
        let server_present =
            self.baseline_complete && server.is_some_and(|n| self.baseline_servers.contains(&n));
        if !transaction_present && !server_present {
            return Ok(false);
        }
        let fact = BaselineContradiction {
            transaction_id: id.clone(),
            server_number: server,
            transaction_present,
            server_present,
        };
        if !self.baseline_contradictions.contains(&fact) {
            require(
                self.baseline_contradictions.len() < 1024,
                "baseline contradiction bound",
            )?;
            self.baseline_contradictions.push(fact);
            canonical_sort(&mut self.baseline_contradictions)?;
        }
        Ok(self
            .baseline_contradictions
            .iter()
            .any(|f| self.baseline_unresolved(f)))
    }
    fn bind(&mut self, t: &Transaction) -> Result<()> {
        t.validate()?;
        self.remember_identity(&t.id, false)?;
        self.baseline_conflict(&t.id, t.server_number)?;
        if !t.compatible(&self.request)
            || self.transaction.as_ref().is_some_and(|old| old.id != t.id)
            || self
                .server_number
                .is_some_and(|n| t.server_number.is_some_and(|new| n != new))
            || self.transaction.as_ref().is_some_and(|old| {
                old.status == TransactionStatus::Ready && t.status != TransactionStatus::Ready
            })
        {
            self.retain_conflict(ConflictFact::Transaction(t.clone()))?;
        }
        if !self.baseline_contradictions.is_empty() {
            // Retain first: a new snapshot must participate in cardinality before binding.
            self.retain_candidate(t)?;
        }
        if self.has_conflict()
            || self.candidate_disqualified(t)
            || (!self.baseline_contradictions.is_empty()
                && self.unique_bindable_candidate() != Some(t))
        {
            self.retain_candidate(t)?;
            self.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
            return Ok(());
        }
        if let Some(n) = t.server_number {
            self.server_number = Some(n);
        }
        self.transaction = Some(t.clone());
        self.allocation = if t.status == TransactionStatus::Ready && self.server_confirmed {
            AllocationProgress::Allocated
        } else {
            AllocationProgress::Pending
        };
        self.allocation_uncertainty = AllocationUncertainty::None;
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PendingRequest {
    pub endpoint: EndpointClass,
    pub operation_id: OperationId,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AccountState {
    pub account_id: Option<AccountScopeId>,
    pub authority_id: Option<AuthorityId>,
    pub sequence: u64,
    pub head: Option<EventDigest>,
    pub operations: Vec<Operation>,
    pub watch_after: Option<OperationId>,
    pub authentication_blocked: bool,
    pub pending_request: Option<PendingRequest>,
    pub budgets: Vec<BudgetWindow>,
    pub observed_limits: Vec<(EndpointClass, crate::scheduling::Quota)>,
    pub provider_holds: Vec<(EndpointClass, Deadline, u64)>,
    pub failures: Vec<(EndpointClass, u64, Deadline)>,
}
impl AccountState {
    fn quota(&self, endpoint: EndpointClass) -> crate::scheduling::Quota {
        self.observed_limits
            .iter()
            .find(|(k, _)| *k == endpoint)
            .map_or(endpoint.quota(), |(_, q)| *q)
    }
    pub fn operation(&self, id: &OperationId) -> Result<&Operation> {
        self.operations
            .iter()
            .find(|o| &o.id == id)
            .ok_or(crate::Error("unknown operation"))
    }
    /// Pure, transactional reducer. Failed validation cannot partially modify state.
    pub fn apply(&self, e: &Envelope) -> Result<Self> {
        e.tool.validate()?;
        e.time.validate()?;
        canonical::text(&e.account_id, 128)?;
        canonical::text(&e.authority_id, 128)?;
        require(
            e.format == FORMAT && e.authority == AUTHORITY && e.schema_version == 1,
            "envelope authority/version",
        )?;
        require(
            e.sequence == self.sequence && e.previous_sha256 == self.head,
            "journal chain",
        )?;
        require(
            self.account_id
                .as_ref()
                .is_none_or(|id| id == &e.account_id)
                && self
                    .authority_id
                    .as_ref()
                    .is_none_or(|id| id == &e.authority_id),
            "authority identity changed",
        )?;
        let mut s = self.clone();
        if self.sequence == 0 {
            require(
                e.event == Event::AuthorityInitialized && e.operation_id.is_none(),
                "genesis required",
            )?;
            s.account_id = Some(e.account_id.clone());
            s.authority_id = Some(e.authority_id.clone());
        } else {
            require(e.event != Event::AuthorityInitialized, "duplicate genesis")?;
            s.transition(e)?;
        }
        for operation in &mut s.operations {
            operation.conflict = operation.has_conflict();
            operation.uncertain = operation.conflict
                || operation.allocation_uncertainty != AllocationUncertainty::None
                || operation.cancellation_uncertainty != CancellationUncertainty::None;
        }
        let bytes = canonical::encode(e)?;
        s.head = Some(canonical::sha256(&bytes).parse()?);
        s.sequence = s
            .sequence
            .checked_add(1)
            .ok_or(crate::Error("sequence overflow"))?;
        Ok(s)
    }
    fn transition(&mut self, e: &Envelope) -> Result<()> {
        if let Event::WatchProgress { after } = &e.event {
            require(e.operation_id.is_none(), "watch progress is account-scoped")?;
            self.operation(after)?;
            self.watch_after = Some(after.clone());
            return Ok(());
        }
        if let Some(pending) = &self.pending_request {
            require(
                e.operation_id.as_ref() == Some(&pending.operation_id),
                "pending request belongs to another operation",
            )?;
        }
        if let Event::MutationResponseLost { endpoint } = e.event {
            require(
                matches!(
                    endpoint,
                    EndpointClass::Allocation | EndpointClass::Cancellation
                ) && self
                    .pending_request
                    .as_ref()
                    .is_some_and(|p| p.endpoint == endpoint),
                "lost mutation identity",
            )?;
            self.pending_request = None;
            let id = e
                .operation_id
                .as_ref()
                .ok_or(crate::Error("operation required"))?;
            let o = self
                .operations
                .iter_mut()
                .find(|o| &o.id == id)
                .ok_or(crate::Error("unknown operation"))?;
            if endpoint == EndpointClass::Cancellation {
                o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
            } else {
                o.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
            }
            return Ok(());
        }
        if let Event::BudgetRebootHold { endpoint } = e.event {
            let quota = self.quota(endpoint);
            let b = self
                .budgets
                .iter_mut()
                .find(|b| b.endpoint == endpoint)
                .ok_or(crate::Error("missing endpoint budget"))?;
            require(b.boot_id != e.time.boot_id, "budget boot unchanged")?;
            b.boot_id = e.time.boot_id.clone();
            b.time_namespace = e.time.time_namespace.clone();
            b.start_ns = e.time.boottime_ns;
            b.spent = quota.requests;
            b.charges_ns = vec![e.time.boottime_ns; b.spent as usize];
            for (k, d, interval) in &mut self.provider_holds {
                if *k == endpoint {
                    *d = Deadline::after(&e.time, *interval)?;
                }
            }
            for (k, _, d) in &mut self.failures {
                if *k == endpoint {
                    *d = Deadline::after(&e.time, 300)?;
                }
            }
            return Ok(());
        }
        if let Event::ProviderAccessResolved { endpoint, evidence } = &e.event {
            evidence.validate()?;
            self.failures.retain(|(k, _, _)| k != endpoint);
            if self
                .pending_request
                .as_ref()
                .is_some_and(|p| p.endpoint == *endpoint)
            {
                self.pending_request = None;
            }
            return Ok(());
        }
        if let Event::ReadSucceeded { endpoint } = e.event {
            require(
                !matches!(
                    endpoint,
                    EndpointClass::Allocation | EndpointClass::Cancellation
                ) && self
                    .pending_request
                    .as_ref()
                    .is_some_and(|p| p.endpoint == endpoint),
                "read completion identity",
            )?;
            self.pending_request = None;
            self.failures.retain(|(k, _, _)| *k != endpoint);
            return Ok(());
        }
        if matches!(
            e.event,
            Event::AllocationResponse { .. }
                | Event::PartialTransactionIdentity {
                    source: TransactionSource::AllocationResponse,
                    ..
                }
        ) && self
            .pending_request
            .as_ref()
            .is_some_and(|p| p.endpoint == EndpointClass::Allocation)
        {
            self.pending_request = None;
            self.failures
                .retain(|(k, _, _)| *k != EndpointClass::Allocation);
        }
        if matches!(e.event, Event::CancellationObserved { .. })
            && self
                .pending_request
                .as_ref()
                .is_some_and(|p| p.endpoint == EndpointClass::Cancellation)
        {
            self.pending_request = None;
            self.failures
                .retain(|(k, _, _)| *k != EndpointClass::Cancellation);
        }
        if let Event::AuthenticationResolved { evidence } = &e.event {
            evidence.validate()?;
            self.authentication_blocked = false;
            return Ok(());
        }
        if let Event::EndpointCharged { endpoint } = e.event {
            require(!self.authentication_blocked, "authentication blocked")?;
            require(
                self.pending_request.is_none(),
                "unresolved previous provider request",
            )?;
            require(
                !self.failures.iter().any(|(k, n, d)| {
                    *k == endpoint && (*n >= 5 || d.boot_id != e.time.boot_id || d.permits(&e.time))
                }),
                "provider retry hold",
            )?;
            require(
                !self.provider_holds.iter().any(|(k, d, _)| {
                    k == &endpoint && (d.boot_id != e.time.boot_id || d.permits(&e.time))
                }),
                "provider rate hold",
            )?;
            let quota = self.quota(endpoint);
            if let Some(b) = self.budgets.iter_mut().find(|b| b.endpoint == endpoint) {
                b.charge_with(&e.time, quota)?;
            } else {
                self.budgets.push(BudgetWindow {
                    endpoint,
                    boot_id: e.time.boot_id.clone(),
                    time_namespace: e.time.time_namespace.clone(),
                    start_ns: e.time.boottime_ns,
                    spent: 1,
                    charges_ns: vec![e.time.boottime_ns],
                });
            }
            let operation_id = e
                .operation_id
                .clone()
                .ok_or(crate::Error("operation required"))?;
            self.operation(&operation_id)?;
            self.pending_request = Some(PendingRequest {
                endpoint,
                operation_id,
            });
            return Ok(());
        }
        if let Event::FailureObserved { endpoint, failure } = &e.event {
            if self
                .pending_request
                .as_ref()
                .is_some_and(|p| p.endpoint == *endpoint)
            {
                self.pending_request = None;
            }
            match failure {
                ProviderFailure::Authentication => self.authentication_blocked = true,
                ProviderFailure::RateLimited {
                    max_request,
                    interval_seconds,
                } => {
                    require(
                        *max_request > 0 && *interval_seconds > 0,
                        "invalid rate observation",
                    )?;
                    let new = Deadline::after(&e.time, *interval_seconds)?;
                    let old = self.quota(*endpoint);
                    let quota = crate::scheduling::Quota {
                        requests: old.requests.min(*max_request),
                        interval_seconds: old.interval_seconds.max(*interval_seconds),
                    };
                    self.observed_limits.retain(|(k, _)| k != endpoint);
                    self.observed_limits.push((*endpoint, quota));
                    if let Some((_, d, i)) = self
                        .provider_holds
                        .iter_mut()
                        .find(|(k, _, _)| k == endpoint)
                    {
                        if d.boot_id != new.boot_id || d.expires_ns < new.expires_ns {
                            *d = new;
                        }
                        *i = (*i).max(*interval_seconds);
                    } else {
                        self.provider_holds
                            .push((*endpoint, new, *interval_seconds));
                    }
                }
                _ => {}
            }
            if !matches!(
                failure,
                ProviderFailure::Authentication | ProviderFailure::RateLimited { .. }
            ) {
                let n = self
                    .failures
                    .iter()
                    .find(|(k, _, _)| k == endpoint)
                    .map_or(1, |(_, n, _)| n.saturating_add(1));
                self.failures.retain(|(k, _, _)| k != endpoint);
                self.failures.push((
                    *endpoint,
                    n,
                    Deadline::after(&e.time, [30, 60, 120, 240, 300][(n.min(5) - 1) as usize])?,
                ));
            }
        }
        let id = e
            .operation_id
            .as_ref()
            .ok_or(crate::Error("operation required"))?;
        canonical::text(id, 128)?;
        if let Event::OperationAuthorized {
            request,
            authorization,
            deadline,
        } = &e.event
        {
            request.validate()?;
            canonical::text(authorization, 1024)?;
            require(deadline.permits(&e.time), "expired authorization")?;
            require(
                self.operations.len() < 64
                    && self.operations.iter().all(|o| &o.id != id && o.closed()),
                "unresolved or duplicate operation",
            )?;
            self.operations.push(Operation {
                id: id.clone(),
                request: request.clone(),
                request_sha256: request.fingerprint()?,
                deadline: deadline.clone(),
                allocation: AllocationProgress::Prepared,
                transaction: None,
                response_transaction_id: None,
                retained_transaction_ids: vec![],
                product_approval: None,
                server_number: None,
                server_confirmed: false,
                last_server_observation: None,
                cancellation: CancellationProgress::NotAuthorized,
                cancellation_readback: CancellationReadback::None,
                baseline_contradictions: vec![],
                reviewed_baseline_attributions: vec![],
                history_conflicts: vec![],
                uncertain: false,
                allocation_uncertainty: AllocationUncertainty::None,
                cancellation_uncertainty: CancellationUncertainty::None,
                conflict: false,
                retained_conflicts: vec![],
                candidates: vec![],
                billing_settled: false,
                next_observation: None,
                observation_rounds: 0,
                baseline_started: false,
                baseline_complete: false,
                catalogue_verified: false,
                baseline_transactions: vec![],
                baseline_servers: vec![],
            });
            return Ok(());
        }
        let o = self
            .operations
            .iter_mut()
            .find(|o| &o.id == id)
            .ok_or(crate::Error("unknown operation"))?;

        match &e.event {
            Event::ObservationRoundStarted => {
                o.check_observation_due(&e.time)?;
                o.next_observation = Some(Deadline::after(&e.time, 300)?);
                o.observation_rounds += 1;
            }
            Event::CatalogueObserved { quote } => {
                require(
                    o.allocation == AllocationProgress::Prepared
                        && quote.product_id == o.request.product_id,
                    "catalogue identity",
                )?;
                quote.validate()?;
                require(
                    quote.approval.account_scope == e.account_id,
                    "product approval account scope",
                )?;
                o.product_approval = Some(quote.approval.clone());
                o.catalogue_verified = true;
            }
            Event::BaselineStarted => {
                require(
                    o.allocation == AllocationProgress::Prepared && !o.baseline_started,
                    "baseline state",
                )?;
                o.baseline_started = true;
            }
            Event::BaselineTransaction { transaction } => {
                transaction.validate()?;
                require(
                    o.baseline_started
                        && !o.baseline_complete
                        && o.baseline_transactions.len() < 1024
                        && !o
                            .baseline_transactions
                            .iter()
                            .any(|t| t.id == transaction.id),
                    "baseline transaction",
                )?;
                o.baseline_transactions.push(transaction.clone());
            }
            Event::BaselineServer { server_number } => {
                require(
                    o.baseline_started
                        && !o.baseline_complete
                        && server_number.get() > 0
                        && o.baseline_servers.len() < 256
                        && !o.baseline_servers.contains(server_number),
                    "baseline server",
                )?;
                o.baseline_servers.push(*server_number);
            }
            Event::BaselineCompleted {
                transactions,
                servers,
            } => {
                require(
                    o.baseline_started
                        && !o.baseline_complete
                        && *transactions == o.baseline_transactions.len() as u64
                        && *servers == o.baseline_servers.len() as u64,
                    "incomplete baseline",
                )?;
                o.baseline_complete = true;
            }
            Event::AllocationDispatchIntent { descriptor } => {
                descriptor.validate_allocation(&o.request)?;
                require(
                    o.allocation == AllocationProgress::Prepared
                        && o.catalogue_verified
                        && o.baseline_complete
                        && o.deadline.permits(&e.time)
                        && !self.authentication_blocked
                        && self
                            .pending_request
                            .as_ref()
                            .is_some_and(|p| p.endpoint == EndpointClass::Allocation),
                    "allocation dispatch prohibited",
                )?;
                o.allocation = AllocationProgress::DispatchRecorded;
                o.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
            }
            Event::AllocationResponse { transaction } => {
                require(
                    o.allocation == AllocationProgress::DispatchRecorded,
                    "unsolicited allocation response",
                )?;
                o.remember_identity(&transaction.id, true)?;
                o.bind(transaction)?;
            }
            Event::PartialTransactionIdentity { id, source, .. } => {
                if *source == TransactionSource::AllocationResponse {
                    require(
                        o.allocation == AllocationProgress::DispatchRecorded,
                        "partial identity without dispatch",
                    )?;
                }
                if o.transaction.as_ref().is_some_and(|t| &t.id != id)
                    || o.response_transaction_id.as_ref().is_some_and(|t| t != id)
                {
                    o.retain_conflict(ConflictFact::TransactionIdentity(id.clone()))?;
                }
                o.remember_identity(id, *source == TransactionSource::AllocationResponse)?;
                if *source == TransactionSource::AllocationResponse {
                    o.baseline_conflict(id, None)?;
                }
                o.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
            }
            Event::TransactionObserved {
                transaction,
                source,
            } => {
                require(
                    *source != TransactionSource::AllocationResponse,
                    "read observation source",
                )?;
                transaction.validate()?;
                if o.transaction
                    .as_ref()
                    .is_some_and(|t| t.id == transaction.id)
                    || o.response_transaction_id.as_ref() == Some(&transaction.id)
                {
                    o.bind(transaction)?;
                } else if !o
                    .baseline_transactions
                    .iter()
                    .any(|t| t.id == transaction.id)
                {
                    o.remember_identity(&transaction.id, false)?;
                    if o.transaction.is_some() || o.response_transaction_id.is_some() {
                        o.retain_conflict(ConflictFact::Transaction(transaction.clone()))?;
                    }
                    o.retain_candidate(transaction)?;
                    o.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
                }
            }
            Event::ServerObserved {
                server_number,
                product,
                datacenter,
                status,
                cancelled,
            } => {
                canonical::text(product, 1024)?;
                canonical::text(datacenter, 128)?;
                o.last_server_observation = Some(ServerObservation {
                    number: *server_number,
                    product: product.clone(),
                    datacenter: datacenter.clone(),
                    status: status.clone(),
                    cancelled: *cancelled,
                });
                if !cancelled
                    && matches!(
                        o.cancellation,
                        CancellationProgress::Acknowledged | CancellationProgress::Released
                    )
                {
                    o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
                    o.retain_conflict(ConflictFact::ServerCancellation(
                        o.last_server_observation.clone().unwrap(),
                    ))?;
                }
                if o.server_number != Some(*server_number)
                    || !(datacenter == "FSN1" || datacenter.starts_with("FSN1-"))
                {
                    o.retain_conflict(ConflictFact::Server(
                        o.last_server_observation.clone().unwrap(),
                    ))?;
                    o.server_confirmed = false;
                    return Ok(());
                }
                let expected = o
                    .product_approval
                    .as_ref()
                    .ok_or(crate::Error("missing product approval"))?;
                if product != &expected.server_product {
                    o.retain_conflict(ConflictFact::Server(
                        o.last_server_observation.clone().unwrap(),
                    ))?;
                    o.server_confirmed = false;
                    return Ok(());
                }
                o.server_confirmed = *status == ServerStatus::Ready && !cancelled;
                if *cancelled && o.cancellation == CancellationProgress::NotAuthorized {
                    o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
                    o.retain_conflict(ConflictFact::ServerCancellation(
                        o.last_server_observation.clone().unwrap(),
                    ))?;
                }
                if !o.server_confirmed {
                    o.allocation = AllocationProgress::Pending;
                    return Ok(());
                }
                if o.transaction
                    .as_ref()
                    .is_some_and(|t| t.status == TransactionStatus::Ready)
                {
                    o.allocation = AllocationProgress::Allocated;
                }
            }
            Event::FailureObserved { endpoint, failure } => {
                if *endpoint == EndpointClass::Allocation {
                    require(
                        o.allocation == AllocationProgress::DispatchRecorded,
                        "allocation failure without dispatch",
                    )?;
                    if *failure == ProviderFailure::Rejected {
                        o.allocation = AllocationProgress::Rejected;
                        o.allocation_uncertainty = AllocationUncertainty::None;
                    } else {
                        o.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
                    }
                } else if *endpoint == EndpointClass::Cancellation {
                    o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
                }
            }
            Event::CancellationAuthorized {
                server_number,
                authorization,
            } => {
                canonical::text(authorization, 1024)?;
                require(
                    o.server_number == Some(*server_number)
                        && !o.conflict
                        && o.cancellation == CancellationProgress::NotAuthorized,
                    "cancellation authorization",
                )?;
                o.cancellation = CancellationProgress::Authorized;
            }
            Event::CancellationDispatchIntent { descriptor } => {
                descriptor.validate_cancellation(
                    o.server_number
                        .ok_or(crate::Error("missing cancellation server"))?,
                )?;
                require(
                    o.server_number.is_some()
                        && !o.conflict
                        && o.cancellation == CancellationProgress::Authorized
                        && matches!(
                            o.cancellation_readback,
                            CancellationReadback::NoCancellationScheduled
                                | CancellationReadback::ReviewedNotScheduled
                        )
                        && self
                            .pending_request
                            .as_ref()
                            .is_some_and(|p| p.endpoint == EndpointClass::Cancellation),
                    "cancellation dispatch prohibited",
                )?;
                o.cancellation = CancellationProgress::DispatchRecorded;
                o.cancellation_readback = CancellationReadback::None;
                o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
            }
            Event::CancellationResubmissionAuthorized {
                server_number,
                authorization,
            } => {
                canonical::text(authorization, 1024)?;
                require(
                    o.server_number == Some(*server_number)
                        && !o.conflict
                        && o.cancellation == CancellationProgress::DispatchRecorded
                        && o.cancellation_uncertainty == CancellationUncertainty::OutcomeUncertain
                        && o.cancellation_readback == CancellationReadback::ReviewedNotScheduled,
                    "resubmission prerequisites",
                )?;
                o.cancellation = CancellationProgress::Authorized;
            }
            Event::CancellationObserved {
                observation,
                readback,
            } => {
                if o.server_number != Some(observation.server_number) {
                    o.cancellation_readback = CancellationReadback::Inconclusive;
                    o.retain_conflict(ConflictFact::Cancellation(observation.clone()))?;
                    o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
                    return Ok(());
                }
                if let Some(d) = &observation.cancellation_date {
                    canonical::text(d, 128)?;
                }
                let result = match (
                    observation.cancelled,
                    observation.cancellation_date.is_some(),
                    observation.reserved,
                ) {
                    (_, _, true) | (true, false, _) | (false, true, _) => {
                        CancellationReadback::Inconclusive
                    }
                    (true, true, false) => CancellationReadback::Acknowledged,
                    (false, false, false) => CancellationReadback::NoCancellationScheduled,
                };
                if *readback {
                    o.cancellation_readback = if result
                        == CancellationReadback::NoCancellationScheduled
                        && o.cancellation_readback == CancellationReadback::ReviewedNotScheduled
                    {
                        CancellationReadback::ReviewedNotScheduled
                    } else {
                        result.clone()
                    };
                }
                match result {
                    CancellationReadback::Acknowledged => {
                        require(
                            o.cancellation != CancellationProgress::NotAuthorized,
                            "cancellation without authorization",
                        )?;
                        if o.cancellation != CancellationProgress::Released {
                            o.cancellation = CancellationProgress::Acknowledged;
                        }
                        o.cancellation_uncertainty = CancellationUncertainty::None;
                    }
                    CancellationReadback::NoCancellationScheduled => {
                        if matches!(
                            o.cancellation,
                            CancellationProgress::Acknowledged | CancellationProgress::Released
                        ) {
                            o.retain_conflict(ConflictFact::Cancellation(observation.clone()))?;
                            o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
                        }
                    }
                    _ => {
                        o.cancellation_readback = CancellationReadback::Inconclusive;
                        o.retain_conflict(ConflictFact::Cancellation(observation.clone()))?;
                        o.cancellation_uncertainty = CancellationUncertainty::OutcomeUncertain;
                    }
                }
            }
            Event::CancellationRetryResolved {
                server_number,
                evidence,
            } => {
                evidence.validate()?;
                require(
                    o.server_number == Some(*server_number)
                        && !o.conflict
                        && o.cancellation_uncertainty == CancellationUncertainty::OutcomeUncertain
                        && o.cancellation == CancellationProgress::DispatchRecorded
                        && o.cancellation_readback == CancellationReadback::NoCancellationScheduled,
                    "reviewed cancellation retry prerequisites",
                )?;
                o.cancellation_readback = CancellationReadback::ReviewedNotScheduled;
            }
            Event::BaselineAttributionResolved {
                transaction,
                evidence,
            } => {
                evidence.validate()?;
                transaction.validate()?;
                require(
                    o.conflict
                        && o.baseline_contradictions
                            .iter()
                            .any(|f| f.transaction_id == transaction.id)
                        && o.candidates.contains(transaction)
                        && transaction.compatible(&o.request)
                        && o.response_transaction_id
                            .as_ref()
                            .is_none_or(|id| id == &transaction.id)
                        && o.transaction
                            .as_ref()
                            .is_none_or(|t| t.id == transaction.id)
                        && o.server_number
                            .is_none_or(|n| transaction.server_number == Some(n)),
                    "baseline resolution cannot replace ownership or invent observations",
                )?;
                o.review_baseline(transaction, BaselineDisposition::Viable)?;
            }
            Event::BaselineCandidateDisqualified {
                transaction,
                evidence,
            } => {
                evidence.validate()?;
                transaction.validate()?;
                require(
                    o.candidates.contains(transaction)
                        && (o
                            .baseline_contradictions
                            .iter()
                            .any(|f| f.transaction_id == transaction.id)
                            || (o.allocation == AllocationProgress::DispatchRecorded
                                && o.allocation_uncertainty
                                    == AllocationUncertainty::RequiresReconciliation
                                && o.transaction.is_none()
                                && o.response_transaction_id.is_none()))
                        && o.server_number.is_none()
                        && o.transaction
                            .as_ref()
                            .is_none_or(|t| t.id == transaction.id && t.server_number.is_none()),
                    "disqualification requires exact unbound baseline or recovery candidate",
                )?;
                o.review_baseline(transaction, BaselineDisposition::Disqualified)?;
            }
            Event::HistoryConflictObserved { observation } => {
                let id = observation
                    .identity()
                    .ok_or(crate::Error("history conflict identity required"))?;
                if let TransactionResponse::Normalized(t) = observation {
                    t.validate()?;
                }
                o.remember_identity(id, false)?;
                if !o.history_conflicts.contains(observation) {
                    require(o.history_conflicts.len() < 1024, "history conflict bound")?;
                    o.history_conflicts.push(observation.clone());
                }
                o.allocation_uncertainty = AllocationUncertainty::RequiresReconciliation;
            }
            Event::AllocationResolved {
                transaction,
                evidence,
            } => {
                evidence.validate()?;
                require(
                    o.allocation_uncertainty == AllocationUncertainty::RequiresReconciliation
                        && o.transaction.is_none()
                        && transaction.compatible(&o.request)
                        && o.response_transaction_id
                            .as_ref()
                            .is_none_or(|id| id == &transaction.id),
                    "allocation resolution",
                )?;
                o.bind_reviewed_recovery(transaction)?;
            }
            Event::IdentityConflictResolved {
                transaction,
                evidence,
            } => {
                evidence.validate()?;
                transaction.validate()?;
                require(
                    o.conflict
                        && transaction.compatible(&o.request)
                        && o.transaction
                            .as_ref()
                            .is_some_and(|t| t.id == transaction.id)
                        && transaction.server_number == o.server_number,
                    "conflict resolution cannot rebind identity",
                )?;
                // This event reviews transaction facts only, never other sources.
                for retained in &mut o.retained_conflicts {
                    if let ConflictFact::Transaction(fact) = &retained.fact
                        && fact == transaction
                    {
                        retained.resolved = true;
                    }
                }
                if o.has_conflict() {
                    return Ok(());
                }
                o.allocation_uncertainty = AllocationUncertainty::None;
                o.transaction = Some(transaction.clone());
                if o.cancellation == CancellationProgress::Released {
                    o.cancellation = CancellationProgress::Acknowledged;
                }
                o.allocation =
                    if transaction.status == TransactionStatus::Ready && o.server_confirmed {
                        AllocationProgress::Allocated
                    } else {
                        AllocationProgress::Pending
                    };
            }
            Event::NonAllocationResolved { evidence } => {
                evidence.validate()?;
                require(
                    o.transaction.is_none()
                        && o.server_number.is_none()
                        && o.response_transaction_id.is_none(),
                    "known allocation cannot be cleared",
                )?;
                o.allocation = AllocationProgress::Rejected;
                o.allocation_uncertainty = AllocationUncertainty::None;
            }
            Event::ReleaseResolved {
                server_number,
                reservation_absent,
                ipv4_obligation_closed,
                evidence,
            } => {
                evidence.validate()?;
                require(
                    o.server_number == Some(*server_number)
                        && *reservation_absent
                        && *ipv4_obligation_closed
                        && !o.conflict
                        && o.cancellation != CancellationProgress::NotAuthorized,
                    "release evidence prerequisites",
                )?;
                o.cancellation = CancellationProgress::Released;
                o.cancellation_uncertainty = CancellationUncertainty::None;
            }
            _ => return Err(crate::Error("invalid operation event")),
        }
        Ok(())
    }
}

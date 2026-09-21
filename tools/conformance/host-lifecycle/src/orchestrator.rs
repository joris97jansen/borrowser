//! Durable commands, not a provider-generic framework. Read-only recovery has
//! no access to the mutation trait. All dispatch accounting precedes the call.
use crate::{Error, Result, canonical, model::*, provider::*, require, scheduling::*};
use crate::{identity::*, mutation::*};

pub(crate) mod sealed {
    pub(crate) trait Sealed {}
}
#[cfg(unix)]
impl sealed::Sealed for crate::journal::Journal {}
/// Production persistence is sealed; an external fake cannot mint durable permission.
/// ```compile_fail
/// use borrowser_host_lifecycle::{orchestrator::EventStore, model::{AccountState, Envelope}, identity::EventDigest, Result};
/// struct Fake;
/// impl EventStore for Fake {
/// fn state(&self) -> &AccountState { unimplemented!() }
/// fn append(&mut self, _: &Envelope) -> Result<EventDigest> { unimplemented!() }
/// fn admit_allocation(&self) -> Result<()> { Ok(()) }
/// }
/// ```
pub(crate) trait EventStore: sealed::Sealed {
    fn state(&self) -> &AccountState;
    fn append(&mut self, event: &Envelope) -> Result<EventDigest>;
    fn admit_allocation(&self) -> Result<()>;
}
#[cfg(unix)]
impl EventStore for crate::journal::Journal {
    fn state(&self) -> &AccountState {
        self.state()
    }
    fn append(&mut self, event: &Envelope) -> Result<EventDigest> {
        self.append(event)
    }
    fn admit_allocation(&self) -> Result<()> {
        self.admit_allocation()
    }
}
pub(crate) trait Clock {
    fn now(&self) -> Result<TimeSample>;
}
pub(crate) type ProviderResult<T> = std::result::Result<T, ProviderFailure>;
pub(crate) trait RobotReader {
    fn catalogue(&mut self, request: &AllocationRequest) -> ProviderResult<CatalogueQuote>;
    fn history(&mut self) -> ProviderResult<Vec<TransactionResponse>>;
    fn servers(&mut self) -> ProviderResult<Vec<ServerObservation>>;
    fn transaction(&mut self, id: &RobotTransactionId) -> ProviderResult<TransactionResponse>;
    fn server(&mut self, number: ServerNumber) -> ProviderResult<ServerObservation>;
    fn cancellation(&mut self, number: ServerNumber) -> ProviderResult<CancellationObservation>;
}
/// Raw requests/server numbers are not mutation authorization.
/// ```compile_fail
/// use borrowser_host_lifecycle::{orchestrator::*, provider::AllocationRequest};
/// fn bypass(p: &mut impl RobotMutator, request: &AllocationRequest) { p.allocate(request); }
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::{orchestrator::*, identity::ServerNumber};
/// fn bypass(p: &mut impl RobotMutator, server: ServerNumber) { p.cancel(server); }
/// ```
pub(crate) trait RobotMutator: RobotReader {
    fn allocate(
        &mut self,
        dispatch: DurableAllocationDispatch,
    ) -> ProviderResult<TransactionResponse>;
    fn cancel(
        &mut self,
        dispatch: DurableCancellationDispatch,
    ) -> ProviderResult<CancellationObservation>;
}
/// Consumed by the provider; no Clone, Deserialize, or public constructor.
/// ```compile_fail
/// use borrowser_host_lifecycle::orchestrator::DurableAllocationDispatch;
/// let forged = DurableAllocationDispatch { binding: todo!() };
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::{orchestrator::DurableAllocationDispatch, identity::OperationId};
/// fn rebind(cap: DurableAllocationDispatch, other: OperationId) { cap.binding().operation = other; }
/// ```
#[derive(Debug)]
pub(crate) struct DurableAllocationDispatch {
    binding: DispatchBinding,
    deadline: Deadline,
}
impl DurableAllocationDispatch {
    pub(crate) fn deadline(&self) -> &Deadline {
        &self.deadline
    }
    pub(crate) fn binding(&self) -> &DispatchBinding {
        &self.binding
    }
}
/// A cancellation capability cannot be substituted with an allocation capability.
/// ```compile_fail
/// use borrowser_host_lifecycle::orchestrator::*;
/// fn substitute(p: &mut impl RobotMutator, a: DurableAllocationDispatch) { p.cancel(a); }
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::{orchestrator::DurableCancellationDispatch, identity::ServerNumber, mutation::MutationDescriptor};
/// fn retarget(cap: DurableCancellationDispatch, server: ServerNumber) { cap.binding().descriptor = MutationDescriptor::cancellation(server).unwrap(); }
/// ```
#[derive(Debug)]
pub(crate) struct DurableCancellationDispatch {
    binding: DispatchBinding,
}
impl DurableCancellationDispatch {
    pub(crate) fn binding(&self) -> &DispatchBinding {
        &self.binding
    }
}
#[derive(Debug, serde::Serialize)]
pub(crate) struct WatchReport {
    pub(crate) attempted: Vec<OperationId>,
    pub(crate) failed_or_held: Vec<OperationId>,
    pub(crate) remaining: Vec<OperationId>,
    pub(crate) unresolved: Vec<OperationId>,
    pub(crate) remaining_due: Vec<OperationId>,
    pub(crate) complete_scan: bool,
    pub(crate) requires_operator_recovery: Vec<(OperationId, ObservationLimit)>,
}
pub(crate) struct Controller<S, C> {
    store: S,
    clock: C,
    tool: ToolIdentityV1,
}
impl<S: EventStore, C: Clock> Controller<S, C> {
    fn record(&mut self, id: Option<&OperationId>, event: Event) -> Result<()> {
        self.commit(id, event).map(|_| ())
    }
    fn commit(&mut self, id: Option<&OperationId>, event: Event) -> Result<EventDigest> {
        let s = self.store.state();
        let e = Envelope {
            account_id: s.account_id.clone().ok_or(Error("missing genesis"))?,
            authority: AUTHORITY.into(),
            authority_id: s.authority_id.clone().ok_or(Error("missing authority"))?,
            event,
            format: FORMAT.into(),
            operation_id: id.cloned(),
            previous_sha256: s.head.clone(),
            schema_version: 1,
            sequence: s.sequence,
            time: self.clock.now()?,
            tool: self.tool.clone(),
        };
        self.store.append(&e)
    }
    fn dispatch_binding(
        &mut self,
        id: &OperationId,
        descriptor: MutationDescriptor,
        kind: MutationKind,
    ) -> Result<DispatchBinding> {
        let account = self
            .store
            .state()
            .account_id
            .clone()
            .ok_or(Error("missing account"))?;
        let authority = self
            .store
            .state()
            .authority_id
            .clone()
            .ok_or(Error("missing authority"))?;
        let intent_sequence = self.store.state().sequence;
        let descriptor_sha256 = descriptor.fingerprint()?;
        let event = if kind == MutationKind::Allocation {
            Event::AllocationDispatchIntent {
                descriptor: descriptor.clone(),
            }
        } else {
            Event::CancellationDispatchIntent {
                descriptor: descriptor.clone(),
            }
        };
        let journal_head = self.commit(Some(id), event)?;
        require(
            self.store.state().head.as_ref() == Some(&journal_head),
            "dispatch receipt mismatch",
        )?;
        Ok(DispatchBinding {
            kind,
            authority,
            account,
            operation: id.clone(),
            intent_sequence,
            journal_head,
            descriptor,
            descriptor_sha256,
        })
    }
    fn retain_transaction_response(
        &mut self,
        id: &OperationId,
        response: TransactionResponse,
        source: TransactionSource,
    ) -> Result<()> {
        match response {
            TransactionResponse::Normalized(transaction) => self.record(
                Some(id),
                if source == TransactionSource::AllocationResponse {
                    Event::AllocationResponse { transaction }
                } else {
                    Event::TransactionObserved {
                        transaction,
                        source,
                    }
                },
            ),
            TransactionResponse::IdentityOnly {
                id: transaction_id,
                failure,
            } => {
                self.record(
                    Some(id),
                    Event::PartialTransactionIdentity {
                        id: transaction_id,
                        source,
                        failure,
                    },
                )?;
                Err(Error("partial provider transaction identity retained"))
            }
            TransactionResponse::NoIdentity(failure) => {
                self.record(
                    Some(id),
                    Event::FailureObserved {
                        endpoint: source.endpoint(),
                        failure,
                    },
                )?;
                Err(Error("no trustworthy provider transaction identity"))
            }
        }
    }
    fn charge(&mut self, id: &OperationId, endpoint: EndpointClass) -> Result<()> {
        if !matches!(
            endpoint,
            EndpointClass::Allocation | EndpointClass::Cancellation
        ) && let Some(previous) = self.store.state().pending_request.as_ref()
            && matches!(
                previous.endpoint,
                EndpointClass::Allocation | EndpointClass::Cancellation
            )
        {
            self.record(
                Some(id),
                Event::MutationResponseLost {
                    endpoint: previous.endpoint,
                },
            )?;
        }
        let now = self.clock.now()?;
        if self
            .store
            .state()
            .budgets
            .iter()
            .any(|b| b.endpoint == endpoint && b.boot_id != now.boot_id)
        {
            self.record(Some(id), Event::BudgetRebootHold { endpoint })?;
        }
        self.record(Some(id), Event::EndpointCharged { endpoint })
    }
    fn observed<T>(
        &mut self,
        id: &OperationId,
        endpoint: EndpointClass,
        value: ProviderResult<T>,
    ) -> Result<T> {
        match value {
            Ok(v) => {
                if !matches!(
                    endpoint,
                    EndpointClass::Allocation | EndpointClass::Cancellation
                ) {
                    self.record(Some(id), Event::ReadSucceeded { endpoint })?;
                }
                Ok(v)
            }
            Err(failure) => {
                self.record(Some(id), Event::FailureObserved { endpoint, failure })?;
                Err(Error(
                    "provider operation incomplete; inspect lifecycle state",
                ))
            }
        }
    }
    pub(crate) fn allocate(
        &mut self,
        id: &OperationId,
        request: AllocationRequest,
        authorization: String,
        robot: &mut impl RobotMutator,
    ) -> Result<()> {
        if let Ok(o) = self.store.state().operation(id) {
            require(o.request == request, "operation request changed")?;
            // A repeated caller never reconstructs dispatch permission, even if
            // an earlier invocation stopped while collecting the baseline.
            return Ok(());
        }
        self.store.admit_allocation()?;
        let deadline = Deadline::after(&self.clock.now()?, 120)?;
        self.record(
            Some(id),
            Event::OperationAuthorized {
                request: request.clone(),
                authorization,
                deadline,
            },
        )?;
        self.charge(id, EndpointClass::Catalogue)?;
        let result = robot.catalogue(&request);
        let quote = self.observed(id, EndpointClass::Catalogue, result)?;
        self.record(
            Some(id),
            Event::CatalogueObserved {
                quote: Box::new(quote),
            },
        )?;
        self.record(Some(id), Event::BaselineStarted)?;
        self.charge(id, EndpointClass::TransactionHistory)?;
        let result = robot.history();
        let mut history = self.observed(id, EndpointClass::TransactionHistory, result)?;
        require(history.len() <= 1024, "history bound")?;
        let mut history: Vec<Transaction> = history
            .drain(..)
            .map(|response| match response {
                TransactionResponse::Normalized(t) => Ok(t),
                _ => Err(Error("incomplete transaction baseline")),
            })
            .collect::<Result<_>>()?;
        history.sort_by(|a, b| a.id.cmp(&b.id));
        for t in &history {
            self.record(
                Some(id),
                Event::BaselineTransaction {
                    transaction: t.clone(),
                },
            )?;
        }
        self.charge(id, EndpointClass::Server)?;
        let result = robot.servers();
        let mut servers = self.observed(id, EndpointClass::Server, result)?;
        require(servers.len() <= 256, "server inventory bound")?;
        servers.sort_by_key(|s| s.number);
        for s in &servers {
            self.record(
                Some(id),
                Event::BaselineServer {
                    server_number: s.number,
                },
            )?;
        }
        self.record(
            Some(id),
            Event::BaselineCompleted {
                transactions: history.len() as u64,
                servers: servers.len() as u64,
            },
        )?;
        self.charge(id, EndpointClass::Allocation)?;
        self.store.admit_allocation()?;
        let descriptor = MutationDescriptor::allocation(&request)?;
        let binding = self.dispatch_binding(id, descriptor, MutationKind::Allocation)?;
        let deadline = self.store.state().operation(id)?.deadline.clone();
        let result = robot.allocate(DurableAllocationDispatch { binding, deadline });
        let transaction = self.observed(id, EndpointClass::Allocation, result)?;
        self.retain_transaction_response(id, transaction, TransactionSource::AllocationResponse)
    }
    pub(crate) fn reconcile(
        &mut self,
        id: &OperationId,
        robot: &mut impl RobotReader,
    ) -> Result<()> {
        let o = self.store.state().operation(id)?.clone();
        o.check_observation_due(&self.clock.now()?)?;
        if let Some(transaction_id) = o
            .transaction
            .as_ref()
            .map(|t| &t.id)
            .or(o.response_transaction_id.as_ref())
        {
            self.charge(id, EndpointClass::Transaction)?;
            self.record(Some(id), Event::ObservationRoundStarted)?;
            let result = robot.transaction(transaction_id);
            let transaction = self.observed(id, EndpointClass::Transaction, result)?;
            self.retain_transaction_response(id, transaction, TransactionSource::TransactionRead)?;
        } else {
            self.charge(id, EndpointClass::TransactionHistory)?;
            self.record(Some(id), Event::ObservationRoundStarted)?;
            let result = robot.history();
            let history = self.observed(id, EndpointClass::TransactionHistory, result)?;
            require(history.len() <= 1024, "history bound")?;
            // Group by trusted ID and canonical typed facts, never provider order.
            let mut groups = std::collections::BTreeMap::<
                RobotTransactionId,
                std::collections::BTreeMap<Vec<u8>, TransactionResponse>,
            >::new();
            let mut unidentified = vec![];
            for response in history {
                let bytes = canonical::encode(&response)?;
                if let Some(id) = response.identity() {
                    groups
                        .entry(id.clone())
                        .or_default()
                        .insert(bytes, response);
                } else {
                    unidentified.push((bytes, response));
                }
            }
            unidentified.sort_by(|a, b| a.0.cmp(&b.0));
            let mut incomplete = false;
            for (_, response) in unidentified {
                incomplete |= self
                    .retain_transaction_response(
                        id,
                        response,
                        TransactionSource::TransactionHistoryItem,
                    )
                    .is_err();
            }
            for (_, group) in groups {
                if group.len() == 1 {
                    let response = group
                        .into_values()
                        .next()
                        .ok_or(Error("empty history group"))?;
                    incomplete |= self
                        .retain_transaction_response(
                            id,
                            response,
                            TransactionSource::TransactionHistoryItem,
                        )
                        .is_err();
                } else {
                    incomplete = true;
                    for observation in group.into_values() {
                        self.record(Some(id), Event::HistoryConflictObserved { observation })?;
                    }
                }
            }
            if incomplete {
                return Err(Error("partial transaction history retained"));
            }
        }
        if let Some(n) = self.store.state().operation(id)?.server_number {
            self.charge(id, EndpointClass::Server)?;
            let result = robot.server(n);
            let s = self.observed(id, EndpointClass::Server, result)?;
            self.record(
                Some(id),
                Event::ServerObserved {
                    server_number: s.number,
                    product: s.product,
                    datacenter: s.datacenter,
                    status: s.status,
                    cancelled: s.cancelled,
                },
            )?;
            if self.store.state().operation(id)?.cancellation != CancellationProgress::NotAuthorized
            {
                self.charge(id, EndpointClass::CancellationRead)?;
                let result = robot.cancellation(n);
                let observation = self.observed(id, EndpointClass::CancellationRead, result)?;
                self.record(
                    Some(id),
                    Event::CancellationObserved {
                        observation,
                        readback: true,
                    },
                )?;
            }
        }
        Ok(())
    }
    pub(crate) fn cancel(
        &mut self,
        id: &OperationId,
        n: ServerNumber,
        authorization: String,
        robot: &mut impl RobotMutator,
    ) -> Result<()> {
        let o = self.store.state().operation(id)?;
        require(
            o.server_number == Some(n) && !o.conflict,
            "cancellation identity unresolved",
        )?;
        if o.cancellation == CancellationProgress::NotAuthorized {
            self.record(
                Some(id),
                Event::CancellationAuthorized {
                    server_number: n,
                    authorization: authorization.clone(),
                },
            )?;
        }
        self.charge(id, EndpointClass::CancellationRead)?;
        let result = robot.cancellation(n);
        let observation = self.observed(id, EndpointClass::CancellationRead, result)?;
        self.record(
            Some(id),
            Event::CancellationObserved {
                observation,
                readback: true,
            },
        )?;
        if self.store.state().operation(id)?.cancellation == CancellationProgress::DispatchRecorded
        {
            require(
                self.store.state().operation(id)?.cancellation_readback
                    == CancellationReadback::ReviewedNotScheduled,
                "uncertain cancellation requires reviewed retry resolution",
            )?;
            self.record(
                Some(id),
                Event::CancellationResubmissionAuthorized {
                    server_number: n,
                    authorization,
                },
            )?;
        }
        if self.store.state().operation(id)?.cancellation != CancellationProgress::Authorized {
            return Ok(());
        }
        require(
            matches!(
                self.store.state().operation(id)?.cancellation_readback,
                CancellationReadback::NoCancellationScheduled
                    | CancellationReadback::ReviewedNotScheduled
            ),
            "inconclusive cancellation readback",
        )?;
        self.charge(id, EndpointClass::Cancellation)?;
        let descriptor = MutationDescriptor::cancellation(n)?;
        let binding = self.dispatch_binding(id, descriptor, MutationKind::Cancellation)?;
        let result = robot.cancel(DurableCancellationDispatch { binding });
        let observation = self.observed(id, EndpointClass::Cancellation, result)?;
        // A POST response is acknowledgement only; release is a reviewed external fact.
        self.record(
            Some(id),
            Event::CancellationObserved {
                observation,
                readback: false,
            },
        )
    }
    pub(crate) fn watch(&mut self, robot: &mut impl RobotReader) -> Result<WatchReport> {
        let mut unresolved: Vec<_> = self
            .store
            .state()
            .operations
            .iter()
            .filter(|o| !o.closed())
            .map(|o| o.id.clone())
            .collect();
        unresolved.sort();
        let now = self.clock.now()?;
        let mut due: Vec<_> = unresolved
            .iter()
            .filter(|id| {
                self.store
                    .state()
                    .operation(id)
                    .is_ok_and(|o| o.observation_eligibility(&now) == ObservationEligibility::Due)
            })
            .cloned()
            .collect();
        // Sorted cyclic traversal, with a durable cursor even when an operation fails.
        // This prevents two unavailable old operations from starving later obligations.
        if let Some(after) = &self.store.state().watch_after {
            let split = due.partition_point(|id| id <= after);
            due.rotate_left(split);
        }
        let mut report = WatchReport {
            attempted: vec![],
            failed_or_held: vec![],
            remaining: vec![],
            unresolved,
            remaining_due: due.iter().skip(2).cloned().collect(),
            complete_scan: false,
            requires_operator_recovery: vec![],
        };
        for id in due.into_iter().take(2) {
            report.attempted.push(id.clone());
            if self.reconcile(&id, robot).is_err() {
                report.failed_or_held.push(id.clone());
            }
            if self
                .record(None, Event::WatchProgress { after: id.clone() })
                .is_err()
            {
                if !report.failed_or_held.contains(&id) {
                    report.failed_or_held.push(id);
                }
                break;
            }
        }
        report.remaining = report
            .unresolved
            .iter()
            .filter(|id| !report.attempted.contains(id) || report.failed_or_held.contains(id))
            .cloned()
            .collect();
        report.remaining_due = report
            .remaining
            .iter()
            .filter(|id| {
                self.store
                    .state()
                    .operation(id)
                    .is_ok_and(|o| o.observation_eligibility(&now) == ObservationEligibility::Due)
            })
            .cloned()
            .collect();
        report.requires_operator_recovery = report
            .unresolved
            .iter()
            .filter_map(|id| {
                match self
                    .store
                    .state()
                    .operation(id)
                    .ok()?
                    .observation_eligibility(&now)
                {
                    ObservationEligibility::RequiresOperatorRecovery(reason) => {
                        Some((id.clone(), reason))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        report.complete_scan = report.failed_or_held.is_empty()
            && report.remaining_due.is_empty()
            && report.requires_operator_recovery.is_empty();
        Ok(report)
    }
    pub(crate) fn resolve(
        &mut self,
        id: &OperationId,
        expected_head: &EventDigest,
        event: Event,
    ) -> Result<()> {
        canonical::digest(expected_head)?;
        require(
            self.store.state().head.as_ref() == Some(expected_head),
            "stale resolution",
        )?;
        require(
            matches!(
                event,
                Event::AllocationResolved { .. }
                    | Event::IdentityConflictResolved { .. }
                    | Event::NonAllocationResolved { .. }
                    | Event::ReleaseResolved { .. }
                    | Event::AuthenticationResolved { .. }
                    | Event::ProviderAccessResolved { .. }
                    | Event::BaselineAttributionResolved { .. }
                    | Event::BaselineCandidateDisqualified { .. }
                    | Event::CancellationRetryResolved { .. }
            ),
            "unsupported resolution",
        )?;
        self.record(Some(id), event)
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn synthetic_allocation_dispatch(
    request: &AllocationRequest,
) -> DurableAllocationDispatch {
    DurableAllocationDispatch {
        deadline: Deadline::after(&crate::linux::now().unwrap(), 120).unwrap(),
        binding: synthetic_binding(
            MutationDescriptor::allocation(request).unwrap(),
            MutationKind::Allocation,
        ),
    }
}
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn synthetic_cancellation_dispatch(server: ServerNumber) -> DurableCancellationDispatch {
    DurableCancellationDispatch {
        binding: synthetic_binding(
            MutationDescriptor::cancellation(server).unwrap(),
            MutationKind::Cancellation,
        ),
    }
}
#[cfg(all(test, target_os = "linux"))]
fn synthetic_binding(descriptor: MutationDescriptor, kind: MutationKind) -> DispatchBinding {
    DispatchBinding {
        kind,
        authority: "c".parse().unwrap(),
        account: "a".parse().unwrap(),
        operation: "synthetic-only".parse().unwrap(),
        intent_sequence: 1,
        journal_head: "1".repeat(64).parse().unwrap(),
        descriptor_sha256: descriptor.fingerprint().unwrap(),
        descriptor,
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn synthetic_expired_allocation_dispatch(
    request: &AllocationRequest,
) -> DurableAllocationDispatch {
    let mut dispatch = synthetic_allocation_dispatch(request);
    dispatch.deadline.expires_ns = 0;
    dispatch
}

#[cfg(test)]
#[path = "../tests/commands.rs"]
pub(crate) mod command_tests;

#[cfg(target_os = "linux")]
pub(crate) struct LinuxClock;
#[cfg(target_os = "linux")]
impl Clock for LinuxClock {
    fn now(&self) -> Result<TimeSample> {
        crate::linux::now()
    }
}
#[cfg(target_os = "linux")]
impl Controller<crate::journal::Journal, LinuxClock> {
    /// The production assembly path never accepts caller-supplied provenance or a clock.
    pub(crate) fn production(store: crate::journal::Journal) -> Result<Self> {
        let tool = crate::runtime::build_identity();
        require(
            tool.source_clean,
            "production commands require a clean committed tool build",
        )?;
        tool.validate()?;
        Ok(Self {
            store,
            clock: LinuxClock,
            tool,
        })
    }
    pub(crate) fn state(&self) -> &AccountState {
        self.store.state()
    }
    pub(crate) fn retain_evidence(&mut self, bytes: &[u8], digest: &str) -> Result<()> {
        self.store.retain_evidence(bytes, digest)
    }
}

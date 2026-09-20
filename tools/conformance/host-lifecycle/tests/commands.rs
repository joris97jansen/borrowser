use super::*;
use crate::approval::*;
use std::{cell::Cell, rc::Rc};
#[derive(Clone)]
struct TestClock(Rc<Cell<u64>>);
impl Clock for TestClock {
    fn now(&self) -> Result<TimeSample> {
        Ok(TimeSample {
            boot_id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            boottime_ns: self.0.get(),
            realtime_ns: self.0.get(),
            time_namespace: "time:[1]".into(),
        })
    }
}
struct Store {
    state: AccountState,
    events: Vec<Envelope>,
    fail_response: bool,
    fail_dispatch: bool,
    fail_cancel_dispatch: bool,
}
impl crate::orchestrator::sealed::Sealed for Store {}
impl EventStore for Store {
    fn state(&self) -> &AccountState {
        &self.state
    }
    fn admit_allocation(&self) -> Result<()> {
        Ok(())
    }
    fn append(&mut self, e: &Envelope) -> Result<EventDigest> {
        if (self.fail_response && matches!(e.event, Event::AllocationResponse { .. }))
            || (self.fail_dispatch && matches!(e.event, Event::AllocationDispatchIntent { .. }))
            || (self.fail_cancel_dispatch
                && matches!(e.event, Event::CancellationDispatchIntent { .. }))
        {
            return Err(Error("simulated fsync failure"));
        }
        self.state = self.state.apply(e)?;
        self.events.push(e.clone());
        Ok(self.state.head.clone().unwrap())
    }
}
fn request() -> AllocationRequest {
    AllocationRequest {
        product_id: "reviewed-id".parse().unwrap(),
        location: "FSN1".into(),
        addons: vec!["primary_ipv4".into()],
    }
}
fn transaction() -> Transaction {
    Transaction {
        id: "B123".parse().unwrap(),
        date: "2026-09-19T12:00:00+00:00".into(),
        status: TransactionStatus::Ready,
        server_number: Some(123.try_into().unwrap()),
        product_id: "reviewed-id".parse().unwrap(),
        location: Some("FSN1".into()),
        addons: vec!["primary_ipv4".into()],
    }
}
fn server() -> ServerObservation {
    ServerObservation {
        number: 123.try_into().unwrap(),
        product: "AX42-1".into(),
        datacenter: "FSN1-DC1".into(),
        status: ServerStatus::Ready,
        cancelled: false,
    }
}
fn cancellation(cancelled: bool) -> CancellationObservation {
    CancellationObservation {
        server_number: 123.try_into().unwrap(),
        cancelled,
        reservation_possible: true,
        reserved: false,
        cancellation_date: cancelled.then(|| "2026-09-19".into()),
    }
}
#[derive(Default)]
struct Robot {
    allocations: usize,
    cancellations: usize,
    reads: usize,
    lost_allocation: bool,
    lost_cancel: bool,
    history: Vec<Transaction>,
    history_override: Option<Vec<TransactionResponse>>,
    inventory: Vec<ServerObservation>,
    cancellation_override: Option<CancellationObservation>,
    read_failure: Option<ProviderFailure>,
    cancelled: bool,
    next_response: Option<TransactionResponse>,
    quote_override: Option<CatalogueQuote>,
    transactions: std::collections::BTreeMap<RobotTransactionId, TransactionResponse>,
    unavailable: Option<RobotTransactionId>,
    read_ids: Vec<RobotTransactionId>,
    dispatches: Vec<crate::mutation::DispatchBinding>,
}
impl RobotReader for Robot {
    fn catalogue(&mut self, _: &AllocationRequest) -> ProviderResult<CatalogueQuote> {
        self.reads += 1;
        Ok(self
            .quote_override
            .clone()
            .unwrap_or_else(|| quote("reviewed-id", "a", "synthetic AX42-1")))
    }
    fn history(&mut self) -> ProviderResult<Vec<TransactionResponse>> {
        self.reads += 1;
        if let Some(e) = &self.read_failure {
            Err(e.clone())
        } else if let Some(items) = &self.history_override {
            Ok(items.clone())
        } else {
            Ok(self
                .history
                .iter()
                .cloned()
                .map(TransactionResponse::Normalized)
                .collect())
        }
    }
    fn servers(&mut self) -> ProviderResult<Vec<ServerObservation>> {
        self.reads += 1;
        Ok(self.inventory.clone())
    }
    fn transaction(&mut self, id: &RobotTransactionId) -> ProviderResult<TransactionResponse> {
        self.reads += 1;
        self.read_ids.push(id.clone());
        if self.unavailable.as_ref() == Some(id) {
            return Err(ProviderFailure::Maintenance);
        }
        Ok(self
            .transactions
            .get(id)
            .cloned()
            .unwrap_or_else(|| TransactionResponse::Normalized(transaction())))
    }
    fn server(&mut self, number: ServerNumber) -> ProviderResult<ServerObservation> {
        self.reads += 1;
        let mut s = server();
        s.number = number;
        Ok(s)
    }
    fn cancellation(&mut self, number: ServerNumber) -> ProviderResult<CancellationObservation> {
        self.reads += 1;
        let mut c = self
            .cancellation_override
            .clone()
            .unwrap_or_else(|| cancellation(self.cancelled));
        c.server_number = number;
        Ok(c)
    }
}
impl RobotMutator for Robot {
    fn allocate(
        &mut self,
        dispatch: DurableAllocationDispatch,
    ) -> ProviderResult<TransactionResponse> {
        assert_eq!(
            dispatch.binding().descriptor,
            MutationDescriptor::allocation(&request()).unwrap()
        );
        self.dispatches.push(dispatch.binding().clone());
        self.allocations += 1;
        if self.lost_allocation {
            Err(ProviderFailure::TransmissionUncertain)
        } else {
            let response = self
                .next_response
                .take()
                .unwrap_or_else(|| TransactionResponse::Normalized(transaction()));
            if let TransactionResponse::Normalized(t) = &response {
                self.transactions.insert(t.id.clone(), response.clone());
            }
            Ok(response)
        }
    }
    fn cancel(
        &mut self,
        dispatch: DurableCancellationDispatch,
    ) -> ProviderResult<CancellationObservation> {
        let number: ServerNumber = dispatch
            .binding()
            .descriptor
            .endpoint()
            .split('/')
            .nth(2)
            .unwrap()
            .parse::<u64>()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            dispatch.binding().descriptor,
            MutationDescriptor::cancellation(number).unwrap()
        );
        self.dispatches.push(dispatch.binding().clone());
        self.cancellations += 1;
        self.cancelled = true;
        if self.lost_cancel {
            Err(ProviderFailure::TransmissionUncertain)
        } else {
            let mut observation = cancellation(true);
            observation.server_number = number;
            Ok(observation)
        }
    }
}
fn setup() -> Controller<Store, TestClock> {
    let clock = TestClock(Rc::new(Cell::new(1)));
    let tool = ToolIdentityV1 {
        package: "borrowser-host-lifecycle".into(),
        package_version: "0.1.0".into(),
        schema_version: 1,
        source_revision: "1".repeat(40),
        cargo_lock_sha256: "2".repeat(64),
        source_clean: true,
    };
    let e = Envelope {
        account_id: "a".parse().unwrap(),
        authority: AUTHORITY.into(),
        authority_id: "c".parse().unwrap(),
        event: Event::AuthorityInitialized,
        format: FORMAT.into(),
        operation_id: None,
        previous_sha256: None,
        schema_version: 1,
        sequence: 0,
        time: clock.now().unwrap(),
        tool: tool.clone(),
    };
    let state = AccountState::default().apply(&e).unwrap();
    Controller {
        store: Store {
            state,
            events: vec![e],
            fail_response: false,
            fail_dispatch: false,
            fail_cancel_dispatch: false,
        },
        clock,
        tool,
    }
}
#[test]
fn lost_response_and_restart_never_resubmit() {
    let mut c = setup();
    let mut r = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    let mut replay = AccountState::default();
    for e in &c.store.events {
        replay = replay.apply(e).unwrap();
    }
    c.store.state = replay;
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "authorization".into(),
        &mut r,
    )
    .unwrap();
    assert_eq!(r.allocations, 1);
    assert!(
        c.allocate(
            &"second".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    assert_eq!(r.allocations, 1);
}
#[test]
fn failed_prepublication_means_no_mutation() {
    let mut c = setup();
    c.store.fail_dispatch = true;
    let mut r = Robot::default();
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    assert_eq!(r.allocations, 0);
}
#[test]
fn received_but_unpublished_identity_stays_uncertain() {
    let mut c = setup();
    c.store.fail_response = true;
    let mut r = Robot::default();
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    assert_eq!(r.allocations, 1);
    assert!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .transaction
            .is_none()
    );
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "authorization".into(),
        &mut r,
    )
    .unwrap();
    assert_eq!(r.allocations, 1);
}
#[test]
fn identical_external_order_is_only_a_candidate() {
    let mut c = setup();
    let mut r = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    r.history = vec![transaction()];
    c.reconcile(&"op".parse().unwrap(), &mut r).unwrap();
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .candidates
            .len(),
        1
    );
    assert!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .transaction
            .is_none()
    );
    assert_eq!(r.allocations, 1);
}
#[test]
fn absent_history_never_proves_nonallocation() {
    let mut c = setup();
    let mut r = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    c.reconcile(&"op".parse().unwrap(), &mut r).unwrap();
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .phase(),
        "requires-reconciliation"
    );
}
#[test]
fn read_only_watch_never_cancels_even_when_authorized() {
    let mut c = setup();
    let mut r = Robot::default();
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "authorization".into(),
        &mut r,
    )
    .unwrap();
    c.record(
        Some(&"op".parse().unwrap()),
        Event::CancellationAuthorized {
            server_number: 123.try_into().unwrap(),
            authorization: "release".into(),
        },
    )
    .unwrap();
    c.watch(&mut r).unwrap();
    assert_eq!(r.allocations, 1);
    assert_eq!(r.cancellations, 0);
}
#[test]
fn lost_cancellation_response_is_read_back_without_repeating_mutation() {
    let mut c = setup();
    let mut r = Robot {
        lost_cancel: true,
        ..Default::default()
    };
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "authorization".into(),
        &mut r,
    )
    .unwrap();
    assert!(
        c.cancel(
            &"op".parse().unwrap(),
            123.try_into().unwrap(),
            "release".into(),
            &mut r
        )
        .is_err()
    );
    c.cancel(
        &"op".parse().unwrap(),
        123.try_into().unwrap(),
        "release".into(),
        &mut r,
    )
    .unwrap();
    assert_eq!(r.cancellations, 1);
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .cancellation,
        CancellationProgress::Acknowledged
    );
}
#[test]
fn authentication_failure_suppresses_subsequent_calls() {
    let mut c = setup();
    let mut r = Robot {
        read_failure: Some(ProviderFailure::Authentication),
        ..Default::default()
    };
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    let reads = r.reads;
    assert!(!c.watch(&mut r).unwrap().complete_scan);
    assert_eq!(r.reads, reads);
    assert_eq!(r.allocations, 0);
}

#[test]
fn interrupted_request_is_owned_by_its_operation() {
    let mut c = setup();
    let mut r = Robot::default();
    c.store.fail_response = true;
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    assert!(
        c.record(
            Some(&"different".parse().unwrap()),
            Event::MutationResponseLost {
                endpoint: EndpointClass::Allocation
            }
        )
        .is_err()
    );
    assert_eq!(
        c.store.state.pending_request.as_ref().unwrap().operation_id,
        "op".parse().unwrap()
    );
    c.store.fail_response = false;
    r.history = vec![transaction()];
    c.reconcile(&"op".parse().unwrap(), &mut r).unwrap();
    assert_eq!(r.allocations, 1);
    assert!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .transaction
            .is_none()
    );
    assert!(c.store.state.pending_request.is_none());
}

#[test]
fn provider_rate_hold_survives_replay_and_is_not_generic_backoff() {
    let mut c = setup();
    let mut r = Robot {
        read_failure: Some(ProviderFailure::RateLimited {
            max_request: 2,
            interval_seconds: 3600,
        }),
        ..Default::default()
    };
    assert!(
        c.allocate(
            &"op".parse().unwrap(),
            request(),
            "authorization".into(),
            &mut r
        )
        .is_err()
    );
    let mut replay = AccountState::default();
    for event in &c.store.events {
        replay = replay.apply(event).unwrap();
    }
    c.store.state = replay;
    r.read_failure = None;
    c.clock.0.set(301_000_000_000);
    let reads = r.reads;
    assert!(c.reconcile(&"op".parse().unwrap(), &mut r).is_err());
    assert_eq!(r.reads, reads);
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .observation_rounds,
        0
    );
    c.clock.0.set(3_601_000_000_000);
    c.reconcile(&"op".parse().unwrap(), &mut r).unwrap();
    assert_eq!(r.reads, reads + 1);
    assert_eq!(r.allocations, 0);
    c.record(
        Some(&"op".parse().unwrap()),
        Event::EndpointCharged {
            endpoint: EndpointClass::TransactionHistory,
        },
    )
    .unwrap();
    c.record(
        Some(&"op".parse().unwrap()),
        Event::ReadSucceeded {
            endpoint: EndpointClass::TransactionHistory,
        },
    )
    .unwrap();
    assert!(
        c.record(
            Some(&"op".parse().unwrap()),
            Event::EndpointCharged {
                endpoint: EndpointClass::TransactionHistory
            }
        )
        .is_err()
    );
}

fn evidence() -> Evidence {
    Evidence {
        sha256: "3".repeat(64),
        bytes: 10,
        provider_reference: "provider-case-123".into(),
        reviewer: "operator".into(),
        rationale: "reviewed provider confirmation for this account and server".into(),
    }
}

#[test]
fn release_requires_exact_reviewed_facts_and_never_settles_billing() {
    let mut c = setup();
    let mut r = Robot::default();
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "authorization".into(),
        &mut r,
    )
    .unwrap();
    c.reconcile(&"op".parse().unwrap(), &mut r).unwrap();
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .phase(),
        "allocated"
    );
    c.cancel(
        &"op".parse().unwrap(),
        123.try_into().unwrap(),
        "release".into(),
        &mut r,
    )
    .unwrap();
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .phase(),
        "cancellation-pending"
    );
    for (number, reservation, ipv4) in [(124, true, true), (123, false, true), (123, true, false)] {
        let head = c.store.state.head.clone().unwrap();
        assert!(
            c.resolve(
                &"op".parse().unwrap(),
                &head,
                Event::ReleaseResolved {
                    server_number: number.try_into().unwrap(),
                    reservation_absent: reservation,
                    ipv4_obligation_closed: ipv4,
                    evidence: evidence()
                }
            )
            .is_err()
        );
    }
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &"op".parse().unwrap(),
        &head,
        Event::ReleaseResolved {
            server_number: 123.try_into().unwrap(),
            reservation_absent: true,
            ipv4_obligation_closed: true,
            evidence: evidence(),
        },
    )
    .unwrap();
    let o = c.store.state.operation(&"op".parse().unwrap()).unwrap();
    assert_eq!(o.phase(), "released");
    assert!(!o.billing_settled);
    c.record(
        Some(&"op".parse().unwrap()),
        Event::CancellationObserved {
            observation: cancellation(false),
            readback: true,
        },
    )
    .unwrap();
    let o = c.store.state.operation(&"op".parse().unwrap()).unwrap();
    assert_eq!(o.phase(), "requires-reconciliation");
    assert_eq!(o.server_number, Some(123.try_into().unwrap()));
}

#[test]
fn conflicting_provider_identity_cannot_replace_retained_ownership() {
    let mut c = setup();
    let mut r = Robot::default();
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "authorization".into(),
        &mut r,
    )
    .unwrap();
    c.record(
        Some(&"op".parse().unwrap()),
        Event::ServerObserved {
            server_number: 123.try_into().unwrap(),
            product: "AX42-1".into(),
            datacenter: "NBG1-DC1".into(),
            status: ServerStatus::Ready,
            cancelled: false,
        },
    )
    .unwrap();
    assert_eq!(
        c.store
            .state
            .operation(&"op".parse().unwrap())
            .unwrap()
            .phase(),
        "requires-reconciliation"
    );
    let mut conflicting = transaction();
    conflicting.id = "external-order".parse().unwrap();
    conflicting.server_number = Some(456.try_into().unwrap());
    c.record(
        Some(&"op".parse().unwrap()),
        Event::TransactionObserved {
            source: TransactionSource::TransactionRead,
            transaction: conflicting,
        },
    )
    .unwrap();
    let o = c.store.state.operation(&"op".parse().unwrap()).unwrap();
    assert_eq!(o.server_number, Some(123.try_into().unwrap()));
    assert_eq!(o.transaction.as_ref().unwrap().id.as_str(), "B123");
    assert_eq!(o.phase(), "requires-reconciliation");
    assert!(
        c.cancel(
            &"op".parse().unwrap(),
            123.try_into().unwrap(),
            "release".into(),
            &mut r
        )
        .is_err()
    );
    assert_eq!(r.cancellations, 0);
}

pub(crate) fn approval(product: &str, account: &str, name: &str) -> ProductApproval {
    ProductApproval {
        schema_version: 1,
        account_scope: account.parse().unwrap(),
        hardware_class: "AX42-1".into(),
        catalogue: CatalogueIdentity {
            product_id: product.parse().unwrap(),
            name: name.into(),
            description: vec!["synthetic reviewed hardware".into()],
        },
        server_product: "AX42-1".into(),
        location: "FSN1".into(),
        addon: "primary_ipv4".into(),
        monthly_gross_ceiling: 1,
        setup_gross_ceiling: 0,
        attested_account_currency: "EUR".into(),
        reviewer: "synthetic-reviewer".into(),
        provider_reference: "synthetic-catalogue-review".into(),
    }
}
fn quote(product: &str, account: &str, name: &str) -> CatalogueQuote {
    let approval = approval(product, account, name);
    CatalogueQuote {
        catalogue_evidence_sha256: approval.digest().unwrap(),
        live_identity: approval.catalogue.clone(),
        approval,
        available_locations: vec!["FSN1".into()],
        ipv4: Ipv4Capability {
            id: "primary_ipv4".parse().unwrap(),
            minimum: 0,
            maximum: 1,
            location: None,
            price_location: "FSN1".into(),
        },
        product_id: product.parse().unwrap(),
        name: name.into(),
        location: "FSN1".into(),
        monthly_gross_units: 1,
        setup_gross_units: 0,
        approved_monthly_gross_units: 1,
        approved_setup_gross_units: 0,
    }
}

#[test]
fn bad_reviewed_product_or_ipv4_capability_blocks_billable_dispatch() {
    let good = quote("reviewed-id", "a", "synthetic AX42-1");
    let mut bad = vec![];
    let mut q = good.clone();
    q.live_identity.product_id = "other-id".parse().unwrap();
    bad.push(q);
    let mut q = good.clone();
    q.live_identity.description = vec!["different hardware".into()];
    bad.push(q);
    let mut q = good.clone();
    q.catalogue_evidence_sha256 = "a".repeat(64).parse().unwrap();
    bad.push(q);
    let mut q = good.clone();
    q.available_locations = vec!["NBG1".into()];
    bad.push(q);
    let mut q = good.clone();
    q.ipv4.id = "other-addon".into();
    bad.push(q);
    let mut q = good.clone();
    q.ipv4.minimum = 2;
    bad.push(q);
    let mut q = good.clone();
    q.ipv4.maximum = 0;
    bad.push(q);
    let mut q = good.clone();
    q.ipv4.location = Some("NBG1".into());
    bad.push(q);
    let mut q = good.clone();
    q.ipv4.price_location = "NBG1".into();
    bad.push(q);
    let mut q = good;
    q.monthly_gross_units = 2;
    bad.push(q);
    for quote in bad {
        let mut c = setup();
        let mut r = Robot {
            quote_override: Some(quote),
            ..Default::default()
        };
        assert!(
            c.allocate(&"op".parse().unwrap(), request(), "approved".into(), &mut r)
                .is_err()
        );
        assert_eq!(r.allocations, 0);
        assert!(r.dispatches.is_empty());
    }
}

#[test]
fn server_confirmation_is_only_compatible_provider_resource_corroboration() {
    for (product, dc, status, cancelled) in [
        ("wrong-product", "FSN1-DC1", ServerStatus::Ready, false),
        ("AX42-1", "NBG1-DC1", ServerStatus::Ready, false),
        ("AX42-1", "FSN1-DC1", ServerStatus::InProcess, false),
        ("AX42-1", "FSN1-DC1", ServerStatus::Ready, true),
    ] {
        let mut c = setup();
        let mut r = Robot::default();
        let id = "op".parse().unwrap();
        c.allocate(&id, request(), "approved".into(), &mut r)
            .unwrap();
        c.record(
            Some(&id),
            Event::ServerObserved {
                server_number: 123.try_into().unwrap(),
                product: product.into(),
                datacenter: dc.into(),
                status,
                cancelled,
            },
        )
        .unwrap();
        let o = c.store.state.operation(&id).unwrap();
        assert!(!o.server_confirmed);
        assert_ne!(o.phase(), "allocated");
        assert_eq!(o.server_number, Some(123.try_into().unwrap()));
        assert!(o.last_server_observation.is_some());
    }
}

#[test]
fn dispatch_capabilities_bind_the_exact_durable_event_and_projection() {
    let mut c = setup();
    let mut r = Robot::default();
    let id = "op".parse().unwrap();
    c.allocate(&id, request(), "approved".into(), &mut r)
        .unwrap();
    c.cancel(&id, 123.try_into().unwrap(), "release".into(), &mut r)
        .unwrap();
    assert_eq!(r.dispatches.len(), 2);
    for binding in &r.dispatches {
        let event = &c.store.events[binding.intent_sequence as usize];
        assert_eq!(event.operation_id.as_ref(), Some(&binding.operation));
        assert_eq!(event.account_id, binding.account);
        assert_eq!(event.authority_id, binding.authority);
        assert_eq!(
            crate::canonical::sha256(&crate::canonical::encode(event).unwrap()),
            binding.journal_head.as_str()
        );
        let descriptor = match &event.event {
            Event::AllocationDispatchIntent { descriptor }
            | Event::CancellationDispatchIntent { descriptor } => descriptor,
            _ => panic!("wrong receipt"),
        };
        assert_eq!(descriptor, &binding.descriptor);
        assert_eq!(descriptor.fingerprint().unwrap(), binding.descriptor_sha256);
        assert_eq!(
            crate::canonical::sha256(descriptor.body().as_bytes()),
            descriptor.body_sha256().as_str()
        );
        assert_eq!(binding.operation, id);
        assert_ne!(binding.operation, "another-operation".parse().unwrap());
    }
    assert!(
        r.dispatches[0]
            .descriptor
            .validate_allocation(&AllocationRequest {
                product_id: "different".parse().unwrap(),
                ..request()
            })
            .is_err()
    );
    assert!(
        r.dispatches[1]
            .descriptor
            .validate_cancellation(456.try_into().unwrap())
            .is_err()
    );
}

#[test]
fn failed_cancellation_intent_never_creates_a_dispatch_capability() {
    let mut c = setup();
    let mut r = Robot::default();
    let id = "op".parse().unwrap();
    c.allocate(&id, request(), "approved".into(), &mut r)
        .unwrap();
    c.store.fail_cancel_dispatch = true;
    assert!(
        c.cancel(&id, 123.try_into().unwrap(), "release".into(), &mut r)
            .is_err()
    );
    assert_eq!(r.cancellations, 0);
    assert_eq!(r.dispatches.len(), 1);
}

#[test]
fn partial_response_identity_survives_restart_and_guides_targeted_reads() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut r = Robot {
        next_response: Some(TransactionResponse::IdentityOnly {
            id: "B123".parse().unwrap(),
            failure: ProviderFailure::Malformed,
        }),
        ..Default::default()
    };
    assert!(
        c.allocate(&id, request(), "approved".into(), &mut r)
            .is_err()
    );
    let o = c.store.state.operation(&id).unwrap();
    assert_eq!(o.response_transaction_id.as_ref().unwrap().as_str(), "B123");
    assert!(o.transaction.is_none() && o.server_number.is_none() && o.uncertain);
    let mut replay = AccountState::default();
    for event in &c.store.events {
        replay = replay.apply(event).unwrap();
    }
    c.store.state = replay;
    c.reconcile(&id, &mut r).unwrap();
    assert_eq!(r.read_ids, ["B123".parse().unwrap()]);
    assert_eq!(r.allocations, 1);
    assert!(c.store.state.operation(&id).unwrap().server_confirmed);
}

#[test]
fn reopened_release_and_active_allocation_are_both_watched_without_mutation() {
    let mut c = setup();
    let mut r = Robot::default();
    let a = "A".parse().unwrap();
    let b = "B".parse().unwrap();
    c.allocate(&a, request(), "approved A".into(), &mut r)
        .unwrap();
    c.cancel(&a, 123.try_into().unwrap(), "release A".into(), &mut r)
        .unwrap();
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &a,
        &head,
        Event::ReleaseResolved {
            server_number: 123.try_into().unwrap(),
            reservation_absent: true,
            ipv4_obligation_closed: true,
            evidence: evidence(),
        },
    )
    .unwrap();
    let mut second = transaction();
    second.id = "B456".parse().unwrap();
    second.server_number = Some(456.try_into().unwrap());
    r.next_response = Some(TransactionResponse::Normalized(second));
    c.allocate(&b, request(), "approved B".into(), &mut r)
        .unwrap();
    c.record(
        Some(&a),
        Event::CancellationObserved {
            observation: cancellation(false),
            readback: true,
        },
    )
    .unwrap();
    r.unavailable = Some("B123".parse().unwrap());
    let scan = c.watch(&mut r).unwrap();
    assert_eq!(scan.unresolved, [a.clone(), b.clone()]);
    assert_eq!(scan.attempted, [a.clone(), b.clone()]);
    assert!(scan.failed_or_held.contains(&a));
    assert!(!scan.complete_scan);
    assert_eq!(
        c.store.state.operation(&a).unwrap().server_number,
        Some(123.try_into().unwrap())
    );
    assert_eq!(
        c.store.state.operation(&b).unwrap().server_number,
        Some(456.try_into().unwrap())
    );
    assert!(
        c.allocate(
            &"C".parse().unwrap(),
            request(),
            "not permitted".into(),
            &mut r
        )
        .is_err()
    );
    assert_eq!((r.allocations, r.cancellations), (2, 1));
}

#[test]
fn watch_cursor_bounds_work_and_does_not_starve_remaining_obligations() {
    let mut c = setup();
    let mut r = Robot::default();
    let mut ids = vec![];
    for (name, number) in [("A", 123), ("B", 456), ("C", 789)] {
        let id: OperationId = name.parse().unwrap();
        let number: ServerNumber = number.try_into().unwrap();
        let mut t = transaction();
        t.id = format!("T{number}").parse().unwrap();
        t.server_number = Some(number);
        r.next_response = Some(TransactionResponse::Normalized(t));
        r.cancelled = false;
        c.allocate(&id, request(), "explicit allocation".into(), &mut r)
            .unwrap();
        c.cancel(&id, number, "explicit release".into(), &mut r)
            .unwrap();
        let head = c.store.state.head.clone().unwrap();
        c.resolve(
            &id,
            &head,
            Event::ReleaseResolved {
                server_number: number,
                reservation_absent: true,
                ipv4_obligation_closed: true,
                evidence: evidence(),
            },
        )
        .unwrap();
        ids.push((id, number));
    }
    for (id, number) in &ids {
        let mut observation = cancellation(false);
        observation.server_number = *number;
        c.record(
            Some(id),
            Event::CancellationObserved {
                observation,
                readback: true,
            },
        )
        .unwrap();
    }
    let first = c.watch(&mut r).unwrap();
    assert_eq!(first.attempted, [ids[0].0.clone(), ids[1].0.clone()]);
    assert_eq!(first.remaining_due, [ids[2].0.clone()]);
    assert!(!first.complete_scan);
    let second = c.watch(&mut r).unwrap();
    assert_eq!(second.attempted, [ids[2].0.clone()]);
    assert_eq!(second.unresolved.len(), 3);
    assert_eq!((r.allocations, r.cancellations), (3, 3));
}

#[test]
fn reviewed_provider_server_spelling_is_not_inferred_from_hardware_class() {
    let mut q = quote("reviewed-id", "a", "synthetic AX42-1");
    q.approval.server_product = "reviewed Robot server spelling".into();
    q.catalogue_evidence_sha256 = q.approval.digest().unwrap();
    let mut c = setup();
    let mut r = Robot {
        quote_override: Some(q),
        ..Default::default()
    };
    let id = "op".parse().unwrap();
    c.allocate(&id, request(), "explicit approval".into(), &mut r)
        .unwrap();
    c.record(
        Some(&id),
        Event::ServerObserved {
            server_number: 123.try_into().unwrap(),
            product: "reviewed Robot server spelling".into(),
            datacenter: "FSN1-DC1".into(),
            status: ServerStatus::Ready,
            cancelled: false,
        },
    )
    .unwrap();
    assert!(c.store.state.operation(&id).unwrap().server_confirmed);
    assert_eq!(c.store.state.operation(&id).unwrap().phase(), "allocated");
}

#[test]
fn incompatible_complete_response_retains_identity_without_binding_resource() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut t = transaction();
    t.product_id = "other-product".parse().unwrap();
    let mut r = Robot {
        next_response: Some(TransactionResponse::Normalized(t)),
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut r)
        .unwrap();
    let o = c.store.state.operation(&id).unwrap();
    assert_eq!(o.response_transaction_id.as_ref().unwrap().as_str(), "B123");
    assert!(o.transaction.is_none() && o.server_number.is_none() && o.conflict);
    assert_eq!(o.phase(), "requires-reconciliation");
    c.reconcile(&id, &mut r).unwrap();
    assert_eq!(r.read_ids, ["B123".parse().unwrap()]);
    assert_eq!(r.allocations, 1);
}

#[cfg(unix)]
pub(crate) fn allocated_events() -> Vec<Envelope> {
    let mut c = setup();
    c.allocate(
        &"op".parse().unwrap(),
        request(),
        "reviewed".into(),
        &mut Robot::default(),
    )
    .unwrap();
    c.store.events
}
#[cfg(unix)]
pub(crate) fn watch_journal(
    journal: crate::journal::Journal,
) -> (crate::journal::Journal, WatchReport, usize) {
    let fixture = setup();
    fixture.clock.0.set(120_000_000_001);
    let mut c = Controller {
        store: journal,
        clock: fixture.clock,
        tool: fixture.tool,
    };
    let mut robot = Robot::default();
    let report = c.watch(&mut robot).unwrap();
    assert_eq!(robot.allocations + robot.cancellations, 0);
    (c.store, report, robot.reads)
}

#[test]
fn transaction_protocol_failures_keep_their_actual_endpoint_provenance() {
    for source in [
        TransactionSource::AllocationResponse,
        TransactionSource::TransactionRead,
        TransactionSource::TransactionHistoryItem,
    ] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let mut robot = Robot::default();
        let malformed = TransactionResponse::NoIdentity(ProviderFailure::Malformed);
        if source == TransactionSource::AllocationResponse {
            robot.next_response = Some(malformed);
            assert!(
                c.allocate(&id, request(), "approved".into(), &mut robot)
                    .is_err()
            );
        } else {
            robot.lost_allocation = source == TransactionSource::TransactionHistoryItem;
            let _ = c.allocate(&id, request(), "approved".into(), &mut robot);
            let before = c.store.state.clone();
            robot
                .transactions
                .insert("B123".parse().unwrap(), malformed.clone());
            robot.history_override = Some(vec![malformed]);
            assert!(c.reconcile(&id, &mut robot).is_err());
            // Only the endpoint which actually supplied the malformed response changes.
            for endpoint in [
                EndpointClass::Allocation,
                EndpointClass::Transaction,
                EndpointClass::TransactionHistory,
            ] {
                if endpoint != source.endpoint() {
                    assert_eq!(
                        canonical_endpoint_state(&before, endpoint),
                        canonical_endpoint_state(&c.store.state, endpoint)
                    );
                }
            }
            assert!(c.store.events.iter().any(|e| matches!(e.event, Event::ReadSucceeded { endpoint } if endpoint == source.endpoint())));
        }
        assert!(
            c.store
                .state
                .failures
                .iter()
                .any(|(endpoint, count, _)| *endpoint == source.endpoint() && *count == 1)
        );
        assert!(
            matches!(c.store.events.last().unwrap().event, Event::FailureObserved { endpoint, failure: ProviderFailure::Malformed } if endpoint == source.endpoint())
        );
        assert!(!c.store.state.authentication_blocked);
        assert!(c.store.state.provider_holds.is_empty());
    }
}
fn canonical_endpoint_state(state: &AccountState, endpoint: EndpointClass) -> Vec<u8> {
    crate::canonical::encode(&(
        state
            .budgets
            .iter()
            .filter(|b| b.endpoint == endpoint)
            .collect::<Vec<_>>(),
        state
            .failures
            .iter()
            .filter(|(e, _, _)| *e == endpoint)
            .collect::<Vec<_>>(),
        state
            .provider_holds
            .iter()
            .filter(|(e, _, _)| *e == endpoint)
            .collect::<Vec<_>>(),
        state
            .observed_limits
            .iter()
            .filter(|(e, _)| *e == endpoint)
            .collect::<Vec<_>>(),
    ))
    .unwrap()
}
#[test]
fn partial_transaction_sources_survive_replay_without_history_attribution() {
    for source in [
        TransactionSource::AllocationResponse,
        TransactionSource::TransactionRead,
        TransactionSource::TransactionHistoryItem,
    ] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let partial = TransactionResponse::IdentityOnly {
            id: "B123".parse().unwrap(),
            failure: ProviderFailure::Malformed,
        };
        let mut robot = Robot::default();
        if source == TransactionSource::AllocationResponse {
            robot.next_response = Some(partial);
            assert!(
                c.allocate(&id, request(), "approved".into(), &mut robot)
                    .is_err()
            );
        } else {
            robot.lost_allocation = source == TransactionSource::TransactionHistoryItem;
            let _ = c.allocate(&id, request(), "approved".into(), &mut robot);
            robot
                .transactions
                .insert("B123".parse().unwrap(), partial.clone());
            robot.history_override = Some(vec![partial]);
            assert!(c.reconcile(&id, &mut robot).is_err());
        }
        assert!(
            matches!(c.store.events.last().unwrap().event, Event::PartialTransactionIdentity { source: actual, .. } if actual == source)
        );
        let mut replay = AccountState::default();
        for e in &c.store.events {
            replay = replay.apply(e).unwrap();
        }
        let operation = replay.operation(&id).unwrap();
        assert!(
            operation
                .retained_transaction_ids
                .contains(&"B123".parse().unwrap())
        );
        assert!(operation.uncertain);
        if source == TransactionSource::TransactionHistoryItem {
            assert!(operation.response_transaction_id.is_none());
            assert!(operation.server_number.is_none());
        }
    }
}

#[test]
fn publication_priority_allowlist_is_explicit() {
    use crate::publication::PublicationClass::{Ordinary, Recovery};
    let ordinary = [
        Event::AuthorityInitialized,
        Event::OperationAuthorized {
            request: request(),
            authorization: "reviewed".into(),
            deadline: Deadline::after(&setup().clock.now().unwrap(), 120).unwrap(),
        },
        Event::CatalogueObserved {
            quote: Box::new(quote("reviewed-id", "a", "synthetic AX42-1")),
        },
        Event::BaselineStarted,
        Event::BaselineTransaction {
            transaction: transaction(),
        },
        Event::BaselineServer {
            server_number: 123.try_into().unwrap(),
        },
        Event::BaselineCompleted {
            transactions: 0,
            servers: 0,
        },
        Event::AllocationDispatchIntent {
            descriptor: MutationDescriptor::allocation(&request()).unwrap(),
        },
        Event::ObservationRoundStarted,
        Event::WatchProgress {
            after: "op".parse().unwrap(),
        },
    ];
    for event in ordinary {
        assert_eq!(event.publication_class(), Ordinary, "{event:?}");
    }
    let recovery = [
        Event::BaselineCandidateDisqualified {
            transaction: transaction(),
            evidence: evidence(),
        },
        Event::BaselineAttributionResolved {
            transaction: transaction(),
            evidence: evidence(),
        },
        Event::CancellationRetryResolved {
            server_number: 123.try_into().unwrap(),
            evidence: evidence(),
        },
        Event::HistoryConflictObserved {
            observation: TransactionResponse::Normalized(transaction()),
        },
        Event::AllocationResponse {
            transaction: transaction(),
        },
        Event::PartialTransactionIdentity {
            id: "B123".parse().unwrap(),
            source: TransactionSource::TransactionHistoryItem,
            failure: ProviderFailure::Malformed,
        },
        Event::TransactionObserved {
            transaction: transaction(),
            source: TransactionSource::TransactionRead,
        },
        Event::ServerObserved {
            server_number: 123.try_into().unwrap(),
            product: "reviewed".into(),
            datacenter: "FSN1".into(),
            status: ServerStatus::Ready,
            cancelled: false,
        },
        Event::CancellationAuthorized {
            server_number: 123.try_into().unwrap(),
            authorization: "reviewed".into(),
        },
        Event::CancellationDispatchIntent {
            descriptor: MutationDescriptor::cancellation(123.try_into().unwrap()).unwrap(),
        },
        Event::CancellationObserved {
            observation: cancellation(true),
            readback: true,
        },
        Event::CancellationResubmissionAuthorized {
            server_number: 123.try_into().unwrap(),
            authorization: "reviewed".into(),
        },
        Event::AllocationResolved {
            transaction: transaction(),
            evidence: evidence(),
        },
        Event::IdentityConflictResolved {
            transaction: transaction(),
            evidence: evidence(),
        },
        Event::NonAllocationResolved {
            evidence: evidence(),
        },
        Event::ReleaseResolved {
            server_number: 123.try_into().unwrap(),
            reservation_absent: true,
            ipv4_obligation_closed: true,
            evidence: evidence(),
        },
        Event::AuthenticationResolved {
            evidence: evidence(),
        },
        Event::ProviderAccessResolved {
            endpoint: EndpointClass::Transaction,
            evidence: evidence(),
        },
        Event::MutationResponseLost {
            endpoint: EndpointClass::Allocation,
        },
    ];
    for event in recovery {
        assert_eq!(event.publication_class(), Recovery, "{event:?}");
    }
    for endpoint in [
        EndpointClass::Allocation,
        EndpointClass::Cancellation,
        EndpointClass::TransactionHistory,
        EndpointClass::Transaction,
        EndpointClass::Server,
        EndpointClass::CancellationRead,
        EndpointClass::Catalogue,
    ] {
        let cancellation = matches!(
            endpoint,
            EndpointClass::Cancellation | EndpointClass::CancellationRead
        );
        for event in [
            Event::EndpointCharged { endpoint },
            Event::ReadSucceeded { endpoint },
            Event::BudgetRebootHold { endpoint },
        ] {
            assert_eq!(
                event.publication_class(),
                if cancellation { Recovery } else { Ordinary }
            );
        }
        assert_eq!(
            Event::FailureObserved {
                endpoint,
                failure: ProviderFailure::Malformed
            }
            .publication_class(),
            if cancellation || endpoint == EndpointClass::Allocation {
                Recovery
            } else {
                Ordinary
            }
        );
    }
}

fn replay_state(c: &Controller<Store, TestClock>) -> AccountState {
    c.store
        .events
        .iter()
        .fold(AccountState::default(), |state, e| state.apply(e).unwrap())
}
#[test]
fn baseline_identity_conflicts_cover_all_direct_binding_paths() {
    for route in [
        "post-transaction",
        "post-server",
        "pending-server",
        "partial-server",
        "partial-transaction",
    ] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let mut robot = Robot::default();
        if route.ends_with("transaction") {
            robot.history = vec![transaction()];
        } else {
            robot.inventory = vec![server()];
        }
        if route.starts_with("partial") {
            robot.next_response = Some(TransactionResponse::IdentityOnly {
                id: transaction().id,
                failure: ProviderFailure::Malformed,
            });
        } else if route == "pending-server" {
            let mut t = transaction();
            t.status = TransactionStatus::InProcess;
            t.server_number = None;
            robot.next_response = Some(TransactionResponse::Normalized(t));
        }
        let _ = c.allocate(&id, request(), "explicit acquisition".into(), &mut robot);
        if route.starts_with("partial") || route == "pending-server" {
            robot.transactions.insert(
                transaction().id,
                TransactionResponse::Normalized(transaction()),
            );
            c.reconcile(&id, &mut robot).unwrap();
        }
        let o = c.store.state.operation(&id).unwrap();
        assert_eq!(o.phase(), "requires-reconciliation", "{route}");
        assert!(o.conflict && o.uncertain);
        assert!(o.server_number.is_none());
        assert!(o.retained_transaction_ids.contains(&transaction().id));
        assert!(o.candidates.contains(&transaction()));
        assert!(!o.baseline_contradictions.is_empty());
        assert!(
            c.cancel(&id, server().number, "not attributable".into(), &mut robot)
                .is_err()
        );
        assert_eq!(robot.cancellations, 0);
        assert_eq!(c.store.state, replay_state(&c));
        // Repeated compatible direct reads cannot silently clear baseline conflicts.
        c.clock.0.set(c.clock.0.get() + 301_000_000_000);
        c.reconcile(&id, &mut robot).unwrap();
        assert!(
            c.store
                .state
                .operation(&id)
                .unwrap()
                .server_number
                .is_none()
        );
        let head = c.store.state.head.clone().unwrap();
        c.resolve(
            &id,
            &head,
            Event::BaselineAttributionResolved {
                transaction: transaction(),
                evidence: evidence(),
            },
        )
        .unwrap();
        let o = c.store.state.operation(&id).unwrap();
        assert_eq!(o.server_number, Some(server().number));
        assert!(!o.conflict);
        assert!(!o.baseline_contradictions.is_empty());
        assert_eq!(c.store.state, replay_state(&c));
        let mut other = transaction();
        other.server_number = Some(456.try_into().unwrap());
        c.record(
            Some(&id),
            Event::TransactionObserved {
                transaction: other.clone(),
                source: TransactionSource::TransactionRead,
            },
        )
        .unwrap();
        let head = c.store.state.head.clone().unwrap();
        assert!(
            c.resolve(
                &id,
                &head,
                Event::BaselineAttributionResolved {
                    transaction: other,
                    evidence: evidence()
                }
            )
            .is_err()
        );
        assert_eq!(
            c.store.state.operation(&id).unwrap().server_number,
            Some(server().number)
        );
    }
}
#[test]
fn baseline_exception_does_not_approve_a_later_baseline_server_or_history_candidate() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut pending = transaction();
    pending.status = TransactionStatus::InProcess;
    pending.server_number = None;
    let mut robot = Robot {
        history: vec![pending.clone()],
        inventory: vec![server()],
        next_response: Some(TransactionResponse::Normalized(pending.clone())),
        ..Default::default()
    };
    c.allocate(&id, request(), "reviewed".into(), &mut robot)
        .unwrap();
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &head,
        Event::BaselineAttributionResolved {
            transaction: pending,
            evidence: evidence(),
        },
    )
    .unwrap();
    robot.transactions.insert(
        transaction().id,
        TransactionResponse::Normalized(transaction()),
    );
    c.reconcile(&id, &mut robot).unwrap();
    assert!(
        c.store
            .state
            .operation(&id)
            .unwrap()
            .server_number
            .is_none()
    );
    assert!(c.store.state.operation(&id).unwrap().conflict);
    let mut other = transaction();
    other.id = "external".parse().unwrap();
    c.record(
        Some(&id),
        Event::TransactionObserved {
            transaction: other.clone(),
            source: TransactionSource::TransactionHistoryItem,
        },
    )
    .unwrap();
    let head = c.store.state.head.clone().unwrap();
    assert!(
        c.resolve(
            &id,
            &head,
            Event::BaselineAttributionResolved {
                transaction: other,
                evidence: evidence()
            }
        )
        .is_err()
    );
    assert!(
        c.store
            .state
            .operation(&id)
            .unwrap()
            .server_number
            .is_none()
    );
}

#[test]
fn cancellation_readback_semantics_never_authorize_unreviewed_resubmission() {
    for (cancelled, date, reserved) in [
        (true, None, false),
        (false, Some("2026-09-20"), false),
        (false, None, true),
        (true, Some("2026-09-20"), false),
        (false, None, false),
    ] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let n = server().number;
        let mut robot = Robot {
            lost_cancel: true,
            ..Default::default()
        };
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .unwrap();
        assert!(
            c.cancel(&id, n, "first authorization".into(), &mut robot)
                .is_err()
        );
        c.clock.0.set(301_000_000_001);
        robot.cancellation_override = Some(CancellationObservation {
            server_number: n,
            cancelled,
            cancellation_date: date.map(str::to_owned),
            reserved,
            reservation_possible: true,
        });
        let result = c.cancel(&id, n, "fresh authorization".into(), &mut robot);
        assert_eq!(robot.cancellations, 1);
        assert_eq!(
            robot
                .dispatches
                .iter()
                .filter(|d| d.kind == MutationKind::Cancellation)
                .count(),
            1
        );
        let o = c.store.state.operation(&id).unwrap();
        let acknowledged = cancelled && date.is_some() && !reserved;
        let negative = !cancelled && date.is_none() && !reserved;
        assert_eq!(
            o.cancellation_readback,
            if acknowledged {
                CancellationReadback::Acknowledged
            } else if negative {
                CancellationReadback::NoCancellationScheduled
            } else {
                CancellationReadback::Inconclusive
            }
        );
        assert_eq!(result.is_ok(), acknowledged);
        assert_eq!(c.store.state, replay_state(&c));
        // Direct reducer attempts cannot bypass the semantic readback condition.
        assert!(
            c.record(
                Some(&id),
                Event::CancellationResubmissionAuthorized {
                    server_number: n,
                    authorization: "not reviewed".into()
                }
            )
            .is_err()
        );
        let head = c.store.state.head.clone().unwrap();
        let review = c.resolve(
            &id,
            &head,
            Event::CancellationRetryResolved {
                server_number: n,
                evidence: evidence(),
            },
        );
        assert_eq!(review.is_ok(), negative);
        if negative {
            robot.lost_cancel = false;
            c.cancel(&id, n, "fresh reviewed retry".into(), &mut robot)
                .unwrap();
            assert_eq!(robot.cancellations, 2);
            assert_eq!(
                c.store.state.operation(&id).unwrap().cancellation,
                CancellationProgress::Acknowledged
            );
            assert_eq!(c.store.state, replay_state(&c));
        }
    }
}
#[test]
fn reviewed_cancellation_retry_is_invalidated_by_later_inconclusive_readback() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let n = server().number;
    let mut robot = Robot {
        lost_cancel: true,
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    assert!(c.cancel(&id, n, "first".into(), &mut robot).is_err());
    c.clock.0.set(301_000_000_001);
    robot.cancellation_override = Some(cancellation(false));
    assert!(c.cancel(&id, n, "retry".into(), &mut robot).is_err());
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &head,
        Event::CancellationRetryResolved {
            server_number: n,
            evidence: evidence(),
        },
    )
    .unwrap();
    robot.cancellation_override.as_mut().unwrap().cancelled = true;
    assert!(c.cancel(&id, n, "retry".into(), &mut robot).is_err());
    assert_eq!(robot.cancellations, 1);
}

fn advance_rounds(c: &mut Controller<Store, TestClock>, id: &OperationId, rounds: u64) {
    for _ in 0..rounds {
        c.clock.0.set(c.clock.0.get() + 301_000_000_000);
        c.record(Some(id), Event::ObservationRoundStarted).unwrap();
    }
    c.clock.0.set(c.clock.0.get() + 301_000_000_000);
}
#[test]
fn observation_exhaustion_is_distinct_from_due_and_not_due_at_exact_limits() {
    for (known, limit, reason) in [
        (false, 4, ObservationLimit::HistoryRounds),
        (true, 576, ObservationLimit::TransactionRounds),
    ] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let mut robot = Robot {
            lost_allocation: !known,
            ..Default::default()
        };
        let _ = c.allocate(&id, request(), "approved".into(), &mut robot);
        advance_rounds(&mut c, &id, limit - 1);
        assert_eq!(
            c.store
                .state
                .operation(&id)
                .unwrap()
                .observation_eligibility(&c.clock.now().unwrap()),
            ObservationEligibility::Due
        );
        let before = robot.reads;
        let last_round = c.watch(&mut robot).unwrap();
        assert!(robot.reads > before);
        assert_eq!(last_round.attempted, vec![id.clone()]);
        assert_eq!(
            last_round.requires_operator_recovery,
            vec![(id.clone(), reason)]
        );
        assert!(!last_round.complete_scan);
        assert_eq!(
            c.store.state.operation(&id).unwrap().observation_rounds,
            limit
        );
        c.store.state = replay_state(&c);
        let before = robot.reads;
        let state = c.store.state.clone();
        let mut previous = None;
        for _ in 0..3 {
            let report = c.watch(&mut robot).unwrap();
            assert!(!report.complete_scan);
            assert_eq!(
                report.requires_operator_recovery,
                vec![(id.clone(), reason)]
            );
            assert_eq!(report.unresolved, vec![id.clone()]);
            assert!(report.attempted.is_empty());
            assert_eq!(robot.reads, before);
            assert_eq!(c.store.state, state);
            let bytes = canonical::encode(&report).unwrap();
            if let Some(previous) = previous {
                assert_eq!(previous, bytes);
            }
            previous = Some(bytes);
        }
    }
    // A routine scheduling hold does not demand intervention.
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot::default();
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    c.record(Some(&id), Event::ObservationRoundStarted).unwrap();
    let before = robot.reads;
    let report = c.watch(&mut robot).unwrap();
    assert!(report.complete_scan && report.requires_operator_recovery.is_empty());
    assert!(report.attempted.is_empty());
    assert_eq!(robot.reads, before);
}
#[test]
fn exhausted_obligation_does_not_prevent_another_due_obligation_from_being_observed() {
    let mut c = setup();
    let a = "A".parse().unwrap();
    let b = "B".parse().unwrap();
    let mut robot = Robot::default();
    c.allocate(&a, request(), "A".into(), &mut robot).unwrap();
    advance_rounds(&mut c, &a, 576);
    c.cancel(&a, server().number, "release A".into(), &mut robot)
        .unwrap();
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &a,
        &head,
        Event::ReleaseResolved {
            server_number: server().number,
            reservation_absent: true,
            ipv4_obligation_closed: true,
            evidence: evidence(),
        },
    )
    .unwrap();
    let mut t = transaction();
    t.id = "B456".parse().unwrap();
    t.server_number = Some(456.try_into().unwrap());
    robot.next_response = Some(TransactionResponse::Normalized(t));
    robot.cancelled = false;
    c.allocate(&b, request(), "B".into(), &mut robot).unwrap();
    c.record(
        Some(&a),
        Event::CancellationObserved {
            observation: cancellation(false),
            readback: true,
        },
    )
    .unwrap();
    let report = c.watch(&mut robot).unwrap();
    assert_eq!(report.attempted, vec![b.clone()]);
    assert_eq!(
        report.requires_operator_recovery,
        vec![(a.clone(), ObservationLimit::TransactionRounds)]
    );
    assert_eq!(report.unresolved, vec![a, b]);
    assert!(!report.complete_scan);
    assert_eq!((robot.allocations, robot.cancellations), (2, 1));
    assert_eq!(c.store.state, replay_state(&c));
}

fn history_run(items: Vec<TransactionResponse>) -> (AccountState, Vec<Envelope>) {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .is_err()
    );
    robot.history_override = Some(items);
    let _ = c.reconcile(&id, &mut robot);
    assert_eq!(c.store.state, replay_state(&c));
    assert_eq!(robot.allocations, 1);
    assert_eq!(robot.cancellations, 0);
    (c.store.state, c.store.events)
}
#[test]
fn conflicting_history_groups_are_permutation_independent_and_retain_every_fact() {
    let a = transaction();
    let mut variants = vec![];
    let mut b = a.clone();
    b.server_number = Some(456.try_into().unwrap());
    variants.push(TransactionResponse::Normalized(b));
    let mut b = a.clone();
    b.product_id = "other-product".parse().unwrap();
    variants.push(TransactionResponse::Normalized(b));
    let mut b = a.clone();
    b.status = TransactionStatus::InProcess;
    variants.push(TransactionResponse::Normalized(b));
    let mut b = a.clone();
    b.location = Some("NBG1".into());
    variants.push(TransactionResponse::Normalized(b));
    let mut b = a.clone();
    b.addons.clear();
    variants.push(TransactionResponse::Normalized(b));
    variants.push(TransactionResponse::IdentityOnly {
        id: a.id.clone(),
        failure: ProviderFailure::Malformed,
    });
    for b in variants {
        let a = TransactionResponse::Normalized(a.clone());
        let mut unrelated = transaction();
        unrelated.id = "unrelated".parse().unwrap();
        let unrelated = TransactionResponse::Normalized(unrelated);
        let invalid = TransactionResponse::NoIdentity(ProviderFailure::Malformed);
        for extra in [vec![], vec![unrelated, invalid]] {
            let mut first = vec![a.clone(), b.clone()];
            first.extend(extra.clone());
            let mut second = extra;
            second.extend([b.clone(), a.clone()]);
            second.reverse();
            // Also compare the simple reverse for the two-item group.
            let mut reverse = first.clone();
            reverse.reverse();
            let (state, events) = history_run(first);
            for permutation in [second, reverse] {
                let (other_state, other_events) = history_run(permutation);
                assert_eq!(state, other_state);
                assert_eq!(events, other_events);
            }
            let o = state.operation(&"op".parse().unwrap()).unwrap();
            assert_eq!(o.history_conflicts.len(), 2);
            assert!(o.history_conflicts.contains(&a) && o.history_conflicts.contains(&b));
            assert!(o.retained_transaction_ids.contains(a.identity().unwrap()));
            assert!(
                o.server_number.is_none()
                    && o.transaction.is_none()
                    && o.response_transaction_id.is_none()
            );
            assert!(o.conflict && o.uncertain);
            assert_eq!(o.phase(), "requires-reconciliation");
        }
    }
}
#[test]
fn identical_history_duplicates_coalesce_without_gaining_attribution() {
    let t = TransactionResponse::Normalized(transaction());
    let single = history_run(vec![t.clone()]);
    let duplicate = history_run(vec![t.clone(), t]);
    assert_eq!(single, duplicate);
    let o = single.0.operation(&"op".parse().unwrap()).unwrap();
    assert_eq!(o.candidates.len(), 1);
    assert!(!o.conflict && o.uncertain && o.transaction.is_none());
}
#[cfg(target_os = "linux")]
pub(crate) fn assert_no_http_cancellation_retry(robot: &mut impl RobotMutator) {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut fake = Robot {
        lost_cancel: true,
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut fake)
        .unwrap();
    assert!(
        c.cancel(&id, server().number, "first".into(), &mut fake)
            .is_err()
    );
    c.clock.0.set(301_000_000_001);
    let _ = c.cancel(&id, server().number, "fresh authorization".into(), robot);
    assert_eq!(
        c.store
            .events
            .iter()
            .filter(|e| matches!(e.event, Event::CancellationDispatchIntent { .. }))
            .count(),
        1
    );
}

#[test]
fn baseline_review_never_clears_independent_conflicts_in_either_observation_order() {
    for source in ["history", "server", "cancellation", "identity"] {
        let mut results = vec![];
        for reverse in [false, true] {
            let mut c = setup();
            let id = "op".parse().unwrap();
            let mut robot = Robot {
                history: vec![transaction()],
                next_response: Some(TransactionResponse::IdentityOnly {
                    id: transaction().id,
                    failure: ProviderFailure::Malformed,
                }),
                ..Default::default()
            };
            assert!(
                c.allocate(&id, request(), "approved".into(), &mut robot)
                    .is_err()
            );
            let baseline = Event::TransactionObserved {
                transaction: transaction(),
                source: TransactionSource::TransactionRead,
            };
            let other = match source {
                "history" => Event::HistoryConflictObserved {
                    observation: TransactionResponse::Normalized(transaction()),
                },
                "server" => Event::ServerObserved {
                    server_number: server().number,
                    product: "incompatible".into(),
                    datacenter: "NBG1".into(),
                    status: ServerStatus::Ready,
                    cancelled: false,
                },
                "cancellation" => Event::CancellationObserved {
                    observation: cancellation(false),
                    readback: true,
                },
                _ => Event::PartialTransactionIdentity {
                    id: "other-id".parse().unwrap(),
                    source: TransactionSource::TransactionRead,
                    failure: ProviderFailure::Malformed,
                },
            };
            let events = if reverse {
                [other, baseline]
            } else {
                [baseline, other]
            };
            for event in events {
                c.record(Some(&id), event).unwrap();
            }
            let head = c.store.state.head.clone().unwrap();
            c.resolve(
                &id,
                &head,
                Event::BaselineAttributionResolved {
                    transaction: transaction(),
                    evidence: evidence(),
                },
            )
            .unwrap();
            let o = c.store.state.operation(&id).unwrap().clone();
            assert_eq!(o.reviewed_baseline_attributions.len(), 1);
            assert!(o.conflict && o.has_conflict() && o.uncertain, "{source}");
            assert_eq!(o.phase(), "requires-reconciliation");
            assert!(o.server_number.is_none() && o.transaction.is_none());
            assert!(
                c.cancel(&id, server().number, "release".into(), &mut robot)
                    .is_err()
            );
            assert_eq!(robot.cancellations, 0);
            assert_eq!(c.store.state, replay_state(&c));
            results.push((
                o.retained_conflicts,
                o.history_conflicts,
                o.baseline_contradictions,
            ));
        }
        assert_eq!(results[0], results[1], "{source}");
    }
}

#[test]
fn reviewing_one_baseline_server_does_not_resolve_another_for_the_same_transaction() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        history: vec![transaction()],
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    let mut other = transaction();
    other.server_number = Some(456.try_into().unwrap());
    c.record(
        Some(&id),
        Event::TransactionObserved {
            transaction: other.clone(),
            source: TransactionSource::TransactionRead,
        },
    )
    .unwrap();
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &head,
        Event::BaselineAttributionResolved {
            transaction: transaction(),
            evidence: evidence(),
        },
    )
    .unwrap();
    let o = c.store.state.operation(&id).unwrap();
    assert_eq!(o.baseline_contradictions.len(), 2);
    assert_eq!(o.reviewed_baseline_attributions.len(), 1);
    assert!(o.conflict && o.uncertain && o.server_number.is_none());
    assert!(o.candidates.contains(&other));
    assert_eq!(c.store.state, replay_state(&c));
}

#[test]
fn identity_review_is_exact_and_cannot_clear_history_server_or_cancellation_conflicts() {
    for source in ["history", "server", "cancellation", "transaction"] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let mut robot = Robot::default();
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .unwrap();
        let mut reviewed = transaction();
        reviewed.status = TransactionStatus::InProcess;
        c.record(
            Some(&id),
            Event::TransactionObserved {
                transaction: reviewed.clone(),
                source: TransactionSource::TransactionRead,
            },
        )
        .unwrap();
        let event = match source {
            "history" => Event::HistoryConflictObserved {
                observation: TransactionResponse::Normalized(transaction()),
            },
            "server" => Event::ServerObserved {
                server_number: server().number,
                product: "wrong".into(),
                datacenter: "NBG1".into(),
                status: ServerStatus::Ready,
                cancelled: false,
            },
            "cancellation" => {
                let mut observation = cancellation(false);
                observation.reserved = true;
                Event::CancellationObserved {
                    observation,
                    readback: true,
                }
            }
            _ => {
                let mut other = reviewed.clone();
                other.product_id = "wrong-product".parse().unwrap();
                Event::TransactionObserved {
                    transaction: other,
                    source: TransactionSource::TransactionRead,
                }
            }
        };
        c.record(Some(&id), event).unwrap();
        let head = c.store.state.head.clone().unwrap();
        c.resolve(
            &id,
            &head,
            Event::IdentityConflictResolved {
                transaction: reviewed.clone(),
                evidence: evidence(),
            },
        )
        .unwrap();
        let o = c.store.state.operation(&id).unwrap();
        assert!(
            o.retained_conflicts
                .iter()
                .any(|f| f.fact == ConflictFact::Transaction(reviewed.clone()) && f.resolved)
        );
        assert!(o.conflict && o.uncertain);
        assert_eq!(o.server_number, Some(server().number));
        assert_eq!(o.phase(), "requires-reconciliation");
        assert_eq!(c.store.state, replay_state(&c));
    }
}

#[test]
fn exact_identity_review_resolves_only_matching_fact_and_recurrence_reopens_it() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot::default();
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    let mut t = transaction();
    t.status = TransactionStatus::InProcess;
    let observation = Event::TransactionObserved {
        transaction: t.clone(),
        source: TransactionSource::TransactionRead,
    };
    c.record(Some(&id), observation.clone()).unwrap();
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &head,
        Event::IdentityConflictResolved {
            transaction: t,
            evidence: evidence(),
        },
    )
    .unwrap();
    assert!(!c.store.state.operation(&id).unwrap().conflict);
    assert_eq!(c.store.state, replay_state(&c));
    c.record(
        Some(&id),
        Event::TransactionObserved {
            transaction: transaction(),
            source: TransactionSource::TransactionRead,
        },
    )
    .unwrap();
    c.record(Some(&id), observation).unwrap();
    assert!(c.store.state.operation(&id).unwrap().conflict);
    for event in [
        Event::AuthenticationResolved {
            evidence: evidence(),
        },
        Event::ProviderAccessResolved {
            endpoint: EndpointClass::Transaction,
            evidence: evidence(),
        },
    ] {
        let head = c.store.state.head.clone().unwrap();
        c.resolve(&id, &head, event).unwrap();
        assert!(c.store.state.operation(&id).unwrap().conflict);
        assert_eq!(c.store.state, replay_state(&c));
    }
}

fn restart_replayed(c: &mut Controller<Store, TestClock>) {
    let replayed = replay_state(c);
    assert_eq!(c.store.state, replayed);
    c.store.state = replayed;
}
#[test]
fn three_reviewed_attributions_require_exact_disposition_independent_of_review_order() {
    let permutations = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut ambiguous = None;
    let mut recovered = None;
    for order in permutations {
        for disposal_order in [[0, 1], [1, 0]] {
            let mut c = setup();
            let id = "op".parse().unwrap();
            let mut robot = Robot {
                history: vec![transaction()],
                ..Default::default()
            };
            c.allocate(&id, request(), "approved".into(), &mut robot)
                .unwrap();
            let candidates: Vec<_> = [123, 456, 789]
                .into_iter()
                .map(|n| {
                    let mut t = transaction();
                    t.server_number = Some(n.try_into().unwrap());
                    t
                })
                .collect();
            for t in &candidates[1..] {
                c.record(
                    Some(&id),
                    Event::TransactionObserved {
                        transaction: t.clone(),
                        source: TransactionSource::TransactionRead,
                    },
                )
                .unwrap();
                restart_replayed(&mut c);
            }
            for i in order {
                let head = c.store.state.head.clone().unwrap();
                c.resolve(
                    &id,
                    &head,
                    Event::BaselineAttributionResolved {
                        transaction: candidates[i].clone(),
                        evidence: evidence(),
                    },
                )
                .unwrap();
                restart_replayed(&mut c);
                let o = c.store.state.operation(&id).unwrap();
                assert!(o.conflict && o.server_number.is_none() && o.transaction.is_none());
                assert!(
                    c.cancel(
                        &id,
                        candidates[i].server_number.unwrap(),
                        "not unique".into(),
                        &mut robot
                    )
                    .is_err()
                );
            }
            let o = c.store.state.operation(&id).unwrap().clone();
            if let Some(ref expected) = ambiguous {
                assert_eq!(&o, expected);
            } else {
                ambiguous = Some(o);
            }
            for (step, i) in disposal_order.into_iter().enumerate() {
                let head = c.store.state.head.clone().unwrap();
                let mut wrong = candidates[i].clone();
                wrong.date = "different evidence subject".into();
                assert!(
                    c.resolve(
                        &id,
                        &head,
                        Event::BaselineCandidateDisqualified {
                            transaction: wrong,
                            evidence: evidence()
                        }
                    )
                    .is_err()
                );
                c.resolve(
                    &id,
                    &head,
                    Event::BaselineCandidateDisqualified {
                        transaction: candidates[i].clone(),
                        evidence: evidence(),
                    },
                )
                .unwrap();
                restart_replayed(&mut c);
                let o = c.store.state.operation(&id).unwrap();
                assert_eq!(o.baseline_contradictions.len(), 3);
                assert_eq!(o.candidates.len(), 3);
                if step == 0 {
                    assert!(o.conflict && o.server_number.is_none());
                }
            }
            let o = c.store.state.operation(&id).unwrap().clone();
            assert!(!o.conflict && !o.uncertain);
            assert_eq!(o.server_number, candidates[2].server_number);
            if let Some(ref expected) = recovered {
                assert_eq!(&o, expected);
            } else {
                recovered = Some(o);
            }
            let head = c.store.state.head.clone().unwrap();
            assert!(
                c.resolve(
                    &id,
                    &head,
                    Event::BaselineCandidateDisqualified {
                        transaction: candidates[2].clone(),
                        evidence: evidence()
                    }
                )
                .is_err()
            );
            c.cancel(
                &id,
                candidates[2].server_number.unwrap(),
                "unique reviewed owner".into(),
                &mut robot,
            )
            .unwrap();
            assert_eq!(robot.cancellations, 1);
            restart_replayed(&mut c);
        }
    }
}

#[test]
fn disqualification_cannot_dismiss_an_unrelated_history_conflict_or_revive_candidate() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        history: vec![transaction()],
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    let mut other = transaction();
    other.server_number = Some(456.try_into().unwrap());
    c.record(
        Some(&id),
        Event::TransactionObserved {
            transaction: other.clone(),
            source: TransactionSource::TransactionRead,
        },
    )
    .unwrap();
    c.record(
        Some(&id),
        Event::HistoryConflictObserved {
            observation: TransactionResponse::Normalized(other.clone()),
        },
    )
    .unwrap();
    let stale = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &stale,
        Event::BaselineAttributionResolved {
            transaction: transaction(),
            evidence: evidence(),
        },
    )
    .unwrap();
    assert!(
        c.resolve(
            &id,
            &stale,
            Event::BaselineCandidateDisqualified {
                transaction: other.clone(),
                evidence: evidence()
            }
        )
        .is_err()
    );
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &head,
        Event::BaselineCandidateDisqualified {
            transaction: other.clone(),
            evidence: evidence(),
        },
    )
    .unwrap();
    let o = c.store.state.operation(&id).unwrap();
    assert!(o.conflict && o.server_number.is_none() && !o.history_conflicts.is_empty());
    let head = c.store.state.head.clone().unwrap();
    assert!(
        c.resolve(
            &id,
            &head,
            Event::BaselineAttributionResolved {
                transaction: other,
                evidence: evidence()
            }
        )
        .is_err()
    );
    restart_replayed(&mut c);
}

#[test]
fn cancellation_retry_survives_reconciliation_before_and_after_review_and_restart() {
    let mut final_semantics = None;
    for review_first in [false, true] {
        for use_watch in [false, true] {
            let mut c = setup();
            let id = "op".parse().unwrap();
            let n = server().number;
            let mut robot = Robot {
                lost_cancel: true,
                ..Default::default()
            };
            c.allocate(&id, request(), "approved".into(), &mut robot)
                .unwrap();
            assert!(c.cancel(&id, n, "first".into(), &mut robot).is_err());
            restart_replayed(&mut c);
            robot.cancelled = false;
            c.clock.0.set(c.clock.0.get() + 301_000_000_000);
            if review_first {
                assert!(
                    c.cancel(&id, n, "negative readback only".into(), &mut robot)
                        .is_err()
                );
                restart_replayed(&mut c);
                let head = c.store.state.head.clone().unwrap();
                c.resolve(
                    &id,
                    &head,
                    Event::CancellationRetryResolved {
                        server_number: n,
                        evidence: evidence(),
                    },
                )
                .unwrap();
                restart_replayed(&mut c);
            }
            for _ in 0..3 {
                if use_watch {
                    assert!(c.watch(&mut robot).unwrap().complete_scan);
                } else {
                    c.reconcile(&id, &mut robot).unwrap();
                }
                restart_replayed(&mut c);
                let o = c.store.state.operation(&id).unwrap();
                assert_eq!(
                    o.cancellation_uncertainty,
                    CancellationUncertainty::OutcomeUncertain
                );
                assert_eq!(o.allocation_uncertainty, AllocationUncertainty::None);
                assert_eq!(
                    o.cancellation_readback,
                    if review_first {
                        CancellationReadback::ReviewedNotScheduled
                    } else {
                        CancellationReadback::NoCancellationScheduled
                    }
                );
                assert!(o.uncertain && o.server_confirmed);
                assert_eq!(robot.cancellations, 1);
                c.clock.0.set(c.clock.0.get() + 301_000_000_000);
            }
            if !review_first {
                let head = c.store.state.head.clone().unwrap();
                c.resolve(
                    &id,
                    &head,
                    Event::CancellationRetryResolved {
                        server_number: n,
                        evidence: evidence(),
                    },
                )
                .unwrap();
                restart_replayed(&mut c);
            }
            let o = c.store.state.operation(&id).unwrap();
            let semantics = (
                o.cancellation.clone(),
                o.cancellation_readback.clone(),
                o.cancellation_uncertainty,
            );
            if let Some(ref expected) = final_semantics {
                assert_eq!(&semantics, expected);
            } else {
                final_semantics = Some(semantics);
            }
            robot.lost_cancel = false;
            c.cancel(&id, n, "fresh explicit reviewed retry".into(), &mut robot)
                .unwrap();
            restart_replayed(&mut c);
            assert_eq!(robot.cancellations, 2);
            let o = c.store.state.operation(&id).unwrap();
            assert_eq!(o.cancellation, CancellationProgress::Acknowledged);
            assert_eq!(o.cancellation_uncertainty, CancellationUncertainty::None);
            assert!(!o.closed() && !o.billing_settled);
            c.cancel(&id, n, "already acknowledged".into(), &mut robot)
                .unwrap();
            assert_eq!(robot.cancellations, 2);
        }
    }
}

#[test]
fn allocation_conflict_blocks_retry_without_erasing_cancellation_review() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let n = server().number;
    let mut robot = Robot {
        lost_cancel: true,
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    assert!(c.cancel(&id, n, "first".into(), &mut robot).is_err());
    robot.cancelled = false;
    c.clock.0.set(c.clock.0.get() + 301_000_000_000);
    assert!(c.cancel(&id, n, "readback".into(), &mut robot).is_err());
    let head = c.store.state.head.clone().unwrap();
    c.resolve(
        &id,
        &head,
        Event::CancellationRetryResolved {
            server_number: n,
            evidence: evidence(),
        },
    )
    .unwrap();
    let mut wrong = transaction();
    wrong.product_id = "wrong".parse().unwrap();
    c.record(
        Some(&id),
        Event::TransactionObserved {
            transaction: wrong,
            source: TransactionSource::TransactionRead,
        },
    )
    .unwrap();
    restart_replayed(&mut c);
    let o = c.store.state.operation(&id).unwrap();
    assert!(o.conflict && o.uncertain);
    assert_eq!(
        o.cancellation_readback,
        CancellationReadback::ReviewedNotScheduled
    );
    assert_eq!(
        o.cancellation_uncertainty,
        CancellationUncertainty::OutcomeUncertain
    );
    assert!(c.cancel(&id, n, "blocked".into(), &mut robot).is_err());
    assert_eq!(robot.cancellations, 1);
    c.record(
        Some(&id),
        Event::CancellationObserved {
            observation: cancellation(true),
            readback: true,
        },
    )
    .unwrap();
    let o = c.store.state.operation(&id).unwrap();
    assert!(o.conflict && o.uncertain);
    assert_eq!(o.cancellation_uncertainty, CancellationUncertainty::None);
    c.record(
        Some(&id),
        Event::CancellationObserved {
            observation: cancellation(false),
            readback: true,
        },
    )
    .unwrap();
    assert_eq!(
        c.store
            .state
            .operation(&id)
            .unwrap()
            .cancellation_uncertainty,
        CancellationUncertainty::OutcomeUncertain
    );
    restart_replayed(&mut c);
}

#[test]
fn unreviewed_nonbaseline_alternative_also_blocks_reviewed_baseline_selection() {
    let mut states = vec![];
    for reverse in [false, true] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let mut robot = Robot {
            inventory: vec![server()],
            ..Default::default()
        };
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .unwrap();
        let mut alternative = transaction();
        alternative.server_number = Some(456.try_into().unwrap());
        c.record(
            Some(&id),
            Event::TransactionObserved {
                transaction: alternative.clone(),
                source: TransactionSource::TransactionRead,
            },
        )
        .unwrap();
        for t in if reverse {
            [alternative.clone(), transaction()]
        } else {
            [transaction(), alternative.clone()]
        } {
            let head = c.store.state.head.clone().unwrap();
            c.resolve(
                &id,
                &head,
                Event::BaselineAttributionResolved {
                    transaction: t,
                    evidence: evidence(),
                },
            )
            .unwrap();
            assert!(
                c.store
                    .state
                    .operation(&id)
                    .unwrap()
                    .server_number
                    .is_none()
            );
            restart_replayed(&mut c);
        }
        let head = c.store.state.head.clone().unwrap();
        c.resolve(
            &id,
            &head,
            Event::BaselineCandidateDisqualified {
                transaction: alternative,
                evidence: evidence(),
            },
        )
        .unwrap();
        assert_eq!(
            c.store.state.operation(&id).unwrap().server_number,
            Some(server().number)
        );
        states.push(c.store.state.operation(&id).unwrap().clone());
        restart_replayed(&mut c);
    }
    assert_eq!(states[0], states[1]);
}

#[test]
fn exact_snapshot_dispositions_are_permutation_independent_and_replayed() {
    let mut a_prime = transaction();
    a_prime.status = TransactionStatus::Cancelled;
    let mut b = transaction();
    b.server_number = Some(456.try_into().unwrap());
    let snapshots = [transaction(), a_prime, b];
    let mut expected = None;
    for observation_order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for review_order in [[0, 2], [2, 0]] {
            for disposition_order in [[0, 1], [1, 0]] {
                let mut c = setup();
                let id = "op".parse().unwrap();
                let mut robot = Robot {
                    history: vec![transaction()],
                    next_response: Some(TransactionResponse::Normalized(
                        snapshots[observation_order[0]].clone(),
                    )),
                    ..Default::default()
                };
                c.allocate(&id, request(), "approved".into(), &mut robot)
                    .unwrap();
                restart_replayed(&mut c);
                for i in &observation_order[1..] {
                    c.record(
                        Some(&id),
                        Event::TransactionObserved {
                            transaction: snapshots[*i].clone(),
                            source: TransactionSource::TransactionRead,
                        },
                    )
                    .unwrap();
                    restart_replayed(&mut c);
                }
                for i in review_order {
                    let head = c.store.state.head.clone().unwrap();
                    c.resolve(
                        &id,
                        &head,
                        Event::BaselineAttributionResolved {
                            transaction: snapshots[i].clone(),
                            evidence: evidence(),
                        },
                    )
                    .unwrap();
                    restart_replayed(&mut c);
                }
                for (step, i) in disposition_order.into_iter().enumerate() {
                    assert!(
                        c.cancel(&id, 123.try_into().unwrap(), "ambiguous".into(), &mut robot)
                            .is_err()
                    );
                    assert_eq!(robot.cancellations, 0);
                    assert!(
                        !c.store
                            .events
                            .iter()
                            .any(|e| matches!(e.event, Event::CancellationDispatchIntent { .. }))
                    );
                    let head = c.store.state.head.clone().unwrap();
                    c.resolve(
                        &id,
                        &head,
                        Event::BaselineCandidateDisqualified {
                            transaction: snapshots[i].clone(),
                            evidence: evidence(),
                        },
                    )
                    .unwrap();
                    restart_replayed(&mut c);
                    let o = c.store.state.operation(&id).unwrap();
                    assert_eq!(o.candidates.len(), 3);
                    if step == 0 {
                        assert!(o.has_conflict() && o.server_number.is_none());
                        let other = 1 - i;
                        assert!(
                            !o.reviewed_baseline_attributions
                                .iter()
                                .any(|r| r.transaction == snapshots[other]
                                    && r.disposition == BaselineDisposition::Disqualified)
                        );
                    }
                }
                let o = c.store.state.operation(&id).unwrap().clone();
                assert!(!o.has_conflict());
                assert_eq!(o.transaction.as_ref(), Some(&snapshots[2]));
                if let Some(ref expected) = expected {
                    assert_eq!(&o, expected);
                } else {
                    expected = Some(o);
                }
                c.cancel(&id, 456.try_into().unwrap(), "unique".into(), &mut robot)
                    .unwrap();
                assert_eq!(robot.cancellations, 1);
                restart_replayed(&mut c);
            }
        }
    }
}

#[test]
fn same_resource_snapshots_require_exact_disposition_including_identity_only_snapshot() {
    for identity_only in [false, true] {
        let mut c = setup();
        let id = "op".parse().unwrap();
        let mut robot = Robot {
            history: vec![transaction()],
            ..Default::default()
        };
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .unwrap();
        let mut other = transaction();
        other.status = TransactionStatus::Cancelled;
        if identity_only {
            other.server_number = None;
        }
        c.record(
            Some(&id),
            Event::TransactionObserved {
                transaction: other.clone(),
                source: TransactionSource::TransactionRead,
            },
        )
        .unwrap();
        restart_replayed(&mut c);
        let head = c.store.state.head.clone().unwrap();
        c.resolve(
            &id,
            &head,
            Event::BaselineAttributionResolved {
                transaction: transaction(),
                evidence: evidence(),
            },
        )
        .unwrap();
        restart_replayed(&mut c);
        assert!(c.store.state.operation(&id).unwrap().has_conflict());
        assert!(
            c.store
                .state
                .operation(&id)
                .unwrap()
                .server_number
                .is_none()
        );
        assert!(
            c.cancel(&id, 123.try_into().unwrap(), "ambiguous".into(), &mut robot)
                .is_err()
        );
        assert_eq!(robot.cancellations, 0);
        let head = c.store.state.head.clone().unwrap();
        c.resolve(
            &id,
            &head,
            Event::BaselineCandidateDisqualified {
                transaction: other,
                evidence: evidence(),
            },
        )
        .unwrap();
        restart_replayed(&mut c);
        assert_eq!(
            c.store.state.operation(&id).unwrap().transaction.as_ref(),
            Some(&transaction())
        );
    }
}

#[test]
fn synthesized_snapshot_cannot_be_reviewed_or_disqualified() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        history: vec![transaction()],
        ..Default::default()
    };
    c.allocate(&id, request(), "approved".into(), &mut robot)
        .unwrap();
    let mut variants = vec![transaction(); 6];
    variants[0].status = TransactionStatus::Cancelled;
    variants[1].date = "another date".into();
    variants[2].product_id = "another-product".parse().unwrap();
    variants[3].location = Some("NBG1".into());
    variants[4].addons.clear();
    variants[5].server_number = Some(456.try_into().unwrap());
    for t in variants {
        let before = c.store.state.clone();
        let head = before.head.clone().unwrap();
        for event in [
            Event::BaselineAttributionResolved {
                transaction: t.clone(),
                evidence: evidence(),
            },
            Event::BaselineCandidateDisqualified {
                transaction: t.clone(),
                evidence: evidence(),
            },
        ] {
            assert!(c.resolve(&id, &head, event).is_err());
            assert_eq!(c.store.state, before);
            restart_replayed(&mut c);
        }
    }
}

#[test]
fn cross_scan_candidates_survive_resolution_permutations_and_exact_disposition() {
    let mut cancelled = transaction();
    cancelled.status = TransactionStatus::Cancelled;
    let mut other_server = transaction();
    other_server.server_number = Some(456.try_into().unwrap());
    let snapshots = [transaction(), cancelled, other_server];
    for baseline_present in [false, true] {
        for survivor in [0, 2] {
            let mut expected_ambiguous = None;
            let mut expected_recovered = None;
            for order in [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ] {
                let mut c = setup();
                let id = "op".parse().unwrap();
                let mut second = server();
                second.number = 456.try_into().unwrap();
                let mut robot = Robot {
                    lost_allocation: true,
                    inventory: if baseline_present {
                        vec![server(), second]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                };
                assert!(
                    c.allocate(&id, request(), "approved".into(), &mut robot)
                        .is_err()
                );
                restart_replayed(&mut c);
                for (round, i) in order.into_iter().enumerate() {
                    robot.history = vec![snapshots[i].clone()];
                    c.clock.0.set(c.clock.0.get() + 301_000_000_000);
                    c.reconcile(&id, &mut robot).unwrap();
                    restart_replayed(&mut c);
                    let o = c.store.state.operation(&id).unwrap();
                    assert_eq!(o.candidates.len(), round + 1);
                    assert!(o.transaction.is_none() && o.server_number.is_none());
                }
                let mut reviews = vec![Event::AllocationResolved {
                    transaction: snapshots[survivor].clone(),
                    evidence: evidence(),
                }];
                if baseline_present {
                    reviews.push(Event::BaselineAttributionResolved {
                        transaction: snapshots[survivor].clone(),
                        evidence: evidence(),
                    });
                }
                for event in reviews {
                    let head = c.store.state.head.clone().unwrap();
                    c.resolve(&id, &head, event).unwrap();
                    restart_replayed(&mut c);
                    let o = c.store.state.operation(&id).unwrap();
                    assert_eq!(o.candidates.len(), 3);
                    assert!(
                        o.has_conflict() && o.transaction.is_none() && o.server_number.is_none()
                    );
                    assert_eq!(o.phase(), "requires-reconciliation");
                    assert!(
                        c.cancel(
                            &id,
                            snapshots[survivor].server_number.unwrap(),
                            "not unique".into(),
                            &mut robot
                        )
                        .is_err()
                    );
                    assert_eq!(robot.cancellations, 0);
                    assert!(
                        !c.store
                            .events
                            .iter()
                            .any(|e| matches!(e.event, Event::CancellationDispatchIntent { .. }))
                    );
                }
                let state = c.store.state.operation(&id).unwrap().clone();
                if let Some(ref expected) = expected_ambiguous {
                    assert_eq!(&state, expected);
                } else {
                    expected_ambiguous = Some(state);
                }
                let discarded = if survivor == 2 { [0, 1] } else { [2, 1] };
                for (step, i) in discarded.into_iter().enumerate() {
                    let head = c.store.state.head.clone().unwrap();
                    c.resolve(
                        &id,
                        &head,
                        Event::BaselineCandidateDisqualified {
                            transaction: snapshots[i].clone(),
                            evidence: evidence(),
                        },
                    )
                    .unwrap();
                    restart_replayed(&mut c);
                    let o = c.store.state.operation(&id).unwrap();
                    assert_eq!(o.candidates.len(), 3);
                    if step == 0 {
                        assert!(o.has_conflict() && o.server_number.is_none());
                        assert!(
                            !o.reviewed_baseline_attributions
                                .iter()
                                .any(|r| r.transaction == snapshots[1]
                                    && r.disposition == BaselineDisposition::Disqualified)
                        );
                        assert!(
                            c.cancel(
                                &id,
                                snapshots[survivor].server_number.unwrap(),
                                "still ambiguous".into(),
                                &mut robot
                            )
                            .is_err()
                        );
                        assert_eq!(robot.cancellations, 0);
                    }
                }
                if !baseline_present {
                    // Disqualification is not allocation authority: the earlier blocked
                    // review cannot later select an owner without a fresh explicit review.
                    assert!(
                        c.store
                            .state
                            .operation(&id)
                            .unwrap()
                            .server_number
                            .is_none()
                    );
                    let head = c.store.state.head.clone().unwrap();
                    c.resolve(
                        &id,
                        &head,
                        Event::AllocationResolved {
                            transaction: snapshots[survivor].clone(),
                            evidence: evidence(),
                        },
                    )
                    .unwrap();
                    restart_replayed(&mut c);
                }
                let o = c.store.state.operation(&id).unwrap().clone();
                assert!(!o.has_conflict());
                assert_eq!(o.transaction.as_ref(), Some(&snapshots[survivor]));
                if let Some(ref expected) = expected_recovered {
                    assert_eq!(&o, expected);
                } else {
                    expected_recovered = Some(o);
                }
                c.cancel(
                    &id,
                    snapshots[survivor].server_number.unwrap(),
                    "unique".into(),
                    &mut robot,
                )
                .unwrap();
                assert_eq!(robot.cancellations, 1);
                restart_replayed(&mut c);
            }
        }
    }
}

#[test]
fn cross_scan_exact_duplicates_coalesce_without_ambiguity() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .is_err()
    );
    robot.history = vec![transaction()];
    for _ in 0..3 {
        c.clock.0.set(c.clock.0.get() + 301_000_000_000);
        c.reconcile(&id, &mut robot).unwrap();
        restart_replayed(&mut c);
        let o = c.store.state.operation(&id).unwrap();
        assert_eq!(o.candidates, vec![transaction()]);
        assert!(!o.has_conflict());
        assert!(o.transaction.is_none()); // A unique history candidate is still not attribution.
    }
}

#[test]
fn candidate_capacity_preserves_all_snapshots_and_accepts_exact_duplicates() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .is_err()
    );
    let observation = |i| {
        let mut t = transaction();
        t.date = format!("snapshot-{i:04}");
        Event::TransactionObserved {
            transaction: t,
            source: TransactionSource::TransactionHistoryItem,
        }
    };
    for i in 0..1024 {
        c.record(Some(&id), observation(i)).unwrap();
    }
    restart_replayed(&mut c);
    let candidates = c.store.state.operation(&id).unwrap().candidates.clone();
    assert_eq!(candidates.len(), 1024);
    c.record(Some(&id), observation(0)).unwrap();
    restart_replayed(&mut c);
    assert_eq!(c.store.state.operation(&id).unwrap().candidates, candidates);
    let before = c.store.state.clone();
    let event_count = c.store.events.len();
    assert!(c.record(Some(&id), observation(1024)).is_err());
    assert_eq!(c.store.state, before);
    assert_eq!(c.store.events.len(), event_count);
    restart_replayed(&mut c);
}

#[test]
fn unique_reviewed_recovery_requires_exact_retained_snapshot_but_not_baseline_review() {
    let mut c = setup();
    let id = "op".parse().unwrap();
    let mut robot = Robot {
        lost_allocation: true,
        ..Default::default()
    };
    assert!(
        c.allocate(&id, request(), "approved".into(), &mut robot)
            .is_err()
    );
    let head = c.store.state.head.clone().unwrap();
    assert!(
        c.resolve(
            &id,
            &head,
            Event::AllocationResolved {
                transaction: transaction(),
                evidence: evidence(),
            }
        )
        .is_err()
    );
    robot.history = vec![transaction()];
    c.clock.0.set(c.clock.0.get() + 301_000_000_000);
    c.reconcile(&id, &mut robot).unwrap();
    restart_replayed(&mut c);
    assert!(c.store.state.operation(&id).unwrap().transaction.is_none());
    let head = c.store.state.head.clone().unwrap();
    let mut synthetic = transaction();
    synthetic.date = "unobserved date".into();
    assert!(
        c.resolve(
            &id,
            &head,
            Event::AllocationResolved {
                transaction: synthetic,
                evidence: evidence(),
            }
        )
        .is_err()
    );
    c.resolve(
        &id,
        &head,
        Event::AllocationResolved {
            transaction: transaction(),
            evidence: evidence(),
        },
    )
    .unwrap();
    restart_replayed(&mut c);
    let o = c.store.state.operation(&id).unwrap();
    assert_eq!(o.transaction.as_ref(), Some(&transaction()));
    assert_eq!(o.server_number, transaction().server_number);
    assert!(!o.server_confirmed);
    assert_eq!(o.allocation, AllocationProgress::Pending);
    assert_eq!(o.cancellation, CancellationProgress::NotAuthorized);
    assert!(o.reviewed_baseline_attributions.is_empty());
    assert!(o.baseline_contradictions.is_empty());
}

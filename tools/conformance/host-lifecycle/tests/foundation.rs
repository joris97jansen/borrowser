use borrowser_host_lifecycle::{
    approval::*, canonical, identity::*, model::*, mutation::*, provider::*, scheduling::*,
};
fn time() -> TimeSample {
    TimeSample {
        boot_id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
        boottime_ns: 1,
        realtime_ns: 1,
        time_namespace: "time:[1]".into(),
    }
}
fn tool() -> ToolIdentityV1 {
    ToolIdentityV1 {
        package: "borrowser-host-lifecycle".into(),
        package_version: "0.1.0".into(),
        schema_version: 1,
        source_revision: "1".repeat(40),
        cargo_lock_sha256: "2".repeat(64),
        source_clean: true,
    }
}
fn append(s: &mut AccountState, event: Event) {
    let e = Envelope {
        account_id: "account".parse().unwrap(),
        authority: AUTHORITY.into(),
        authority_id: "controller".parse().unwrap(),
        event,
        format: FORMAT.into(),
        operation_id: if s.sequence == 0 {
            None
        } else {
            Some("operation".parse().unwrap())
        },
        previous_sha256: s.head.clone(),
        schema_version: 1,
        sequence: s.sequence,
        time: time(),
        tool: tool(),
    };
    *s = s.apply(&e).unwrap();
}
fn prepared() -> AccountState {
    let mut s = AccountState::default();
    append(&mut s, Event::AuthorityInitialized);
    append(
        &mut s,
        Event::OperationAuthorized {
            request: request(),
            authorization: "approved".into(),
            deadline: Deadline::after(&time(), 120).unwrap(),
        },
    );
    s
}
fn request() -> AllocationRequest {
    AllocationRequest {
        product_id: "catalogue-pin".parse().unwrap(),
        location: "FSN1".into(),
        addons: vec!["primary_ipv4".into()],
    }
}
fn transaction() -> Transaction {
    Transaction {
        id: "B-order".parse().unwrap(),
        date: "2026-09-19T12:00:00+00:00".into(),
        status: TransactionStatus::Ready,
        server_number: Some(42.try_into().unwrap()),
        product_id: "catalogue-pin".parse().unwrap(),
        location: Some("FSN1".into()),
        addons: vec!["primary_ipv4".into()],
    }
}
fn dispatched() -> AccountState {
    let mut s = prepared();
    append(
        &mut s,
        Event::CatalogueObserved {
            quote: Box::new(quote("catalogue-pin", "account", "synthetic AX42-1")),
        },
    );
    append(&mut s, Event::BaselineStarted);
    append(
        &mut s,
        Event::BaselineCompleted {
            transactions: 0,
            servers: 0,
        },
    );
    append(
        &mut s,
        Event::EndpointCharged {
            endpoint: EndpointClass::Allocation,
        },
    );
    append(
        &mut s,
        Event::AllocationDispatchIntent {
            descriptor: MutationDescriptor::allocation(&request()).unwrap(),
        },
    );
    s
}
#[test]
fn exact_projection_has_no_provisioning_fields() {
    assert_eq!(
        request().form().unwrap(),
        "product_id=catalogue-pin&location=FSN1&addon%5B%5D=primary_ipv4"
    );
    assert_eq!(
        CANCELLATION_FORM,
        "cancellation_date=now&reserve_location=false"
    );
    assert!("a&dist=Debian".parse::<ProductId>().is_err());
    assert!(
        serde_json::from_str::<AllocationRequest>(
            r#"{"product_id":"x","location":"FSN1","addons":["primary_ipv4"],"dist":"Debian"}"#
        )
        .is_err()
    );
}
#[test]
fn canonical_encoding_is_explicit() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct V {
        z: u64,
        a: String,
    }
    let v = V {
        z: 2,
        a: "é\n\"\\/".into(),
    };
    let expected = b"{\"a\":\"\xc3\xa9\\u000a\\\"\\\\/\",\"z\":2}\n";
    assert_eq!(canonical::encode(&v).unwrap(), expected);
    assert_eq!(canonical::decode::<V>(expected).unwrap(), v);
    for b in [
        b"{\"a\":\"x\",\"z\":2}\n\n".as_slice(),
        b"{\"a\":\"x\",\"a\":\"x\",\"z\":2}\n",
        b"{\"z\":2,\"a\":\"x\"}\n",
    ] {
        assert!(canonical::decode::<V>(b).is_err());
    }
    assert_eq!(
        canonical::sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
#[test]
fn unique_candidate_is_not_attribution() {
    let mut s = dispatched();
    append(
        &mut s,
        Event::TransactionObserved {
            source: TransactionSource::TransactionHistoryItem,
            transaction: transaction(),
        },
    );
    let o = s.operation(&"operation".parse().unwrap()).unwrap();
    assert!(o.transaction.is_none());
    assert_eq!(o.candidates.len(), 1);
    assert_eq!(o.phase(), "requires-reconciliation");
}
#[test]
fn acknowledged_is_not_released_and_identity_survives_failure() {
    let mut s = dispatched();
    append(
        &mut s,
        Event::AllocationResponse {
            transaction: transaction(),
        },
    );
    append(
        &mut s,
        Event::ServerObserved {
            server_number: 42.try_into().unwrap(),
            product: "AX42-1".into(),
            datacenter: "FSN1-DC1".into(),
            status: ServerStatus::Ready,
            cancelled: false,
        },
    );
    assert_eq!(
        s.operation(&"operation".parse().unwrap()).unwrap().phase(),
        "allocated"
    );
    append(
        &mut s,
        Event::CancellationAuthorized {
            server_number: 42.try_into().unwrap(),
            authorization: "release".into(),
        },
    );
    let mut without_readback_state = s.clone();
    append(
        &mut without_readback_state,
        Event::EndpointCharged {
            endpoint: EndpointClass::Cancellation,
        },
    );
    let without_readback = Envelope {
        account_id: "account".parse().unwrap(),
        authority: AUTHORITY.into(),
        authority_id: "controller".parse().unwrap(),
        event: Event::CancellationDispatchIntent {
            descriptor: MutationDescriptor::cancellation(42.try_into().unwrap()).unwrap(),
        },
        format: FORMAT.into(),
        operation_id: Some("operation".parse().unwrap()),
        previous_sha256: without_readback_state.head.clone(),
        schema_version: 1,
        sequence: without_readback_state.sequence,
        time: time(),
        tool: tool(),
    };
    assert!(without_readback_state.apply(&without_readback).is_err());
    append(
        &mut s,
        Event::CancellationObserved {
            readback: true,
            observation: CancellationObservation {
                server_number: 42.try_into().unwrap(),
                cancelled: false,
                reservation_possible: false,
                reserved: false,
                cancellation_date: None,
            },
        },
    );
    append(
        &mut s,
        Event::EndpointCharged {
            endpoint: EndpointClass::Cancellation,
        },
    );
    append(
        &mut s,
        Event::CancellationDispatchIntent {
            descriptor: MutationDescriptor::cancellation(42.try_into().unwrap()).unwrap(),
        },
    );
    append(
        &mut s,
        Event::FailureObserved {
            endpoint: EndpointClass::Cancellation,
            failure: ProviderFailure::TransmissionUncertain,
        },
    );
    assert_eq!(
        s.operation(&"operation".parse().unwrap())
            .unwrap()
            .server_number,
        Some(42.try_into().unwrap())
    );
    append(
        &mut s,
        Event::CancellationObserved {
            readback: true,
            observation: CancellationObservation {
                server_number: 42.try_into().unwrap(),
                cancelled: true,
                reservation_possible: false,
                reserved: false,
                cancellation_date: Some("2026-09-19".into()),
            },
        },
    );
    assert_eq!(
        s.operation(&"operation".parse().unwrap()).unwrap().phase(),
        "cancellation-pending"
    );
    assert!(
        !s.operation(&"operation".parse().unwrap())
            .unwrap()
            .billing_settled
    );
}
#[test]
fn process_restart_preserves_deadline_and_reboot_expires_it() {
    let d = Deadline::after(&time(), 10).unwrap();
    let bytes = canonical::encode(&d).unwrap();
    let d: Deadline = canonical::decode(&bytes).unwrap();
    assert!(d.permits(&time()));
    let mut t = time();
    t.boot_id = "00000000-0000-0000-0000-000000000002".into();
    assert!(!d.permits(&t));
    t = time();
    t.boottime_ns = d.expires_ns;
    assert!(!d.permits(&t));
}
#[test]
fn dispatch_cannot_be_replayed_as_a_second_attempt() {
    let s = dispatched();
    let e = Envelope {
        account_id: "account".parse().unwrap(),
        authority: AUTHORITY.into(),
        authority_id: "controller".parse().unwrap(),
        event: Event::AllocationDispatchIntent {
            descriptor: MutationDescriptor::allocation(&request()).unwrap(),
        },
        format: FORMAT.into(),
        operation_id: Some("operation".parse().unwrap()),
        previous_sha256: s.head.clone(),
        schema_version: 1,
        sequence: s.sequence,
        time: time(),
        tool: tool(),
    };
    assert!(s.apply(&e).is_err());
    let mut wrong = e;
    wrong.event = Event::TransactionObserved {
        source: TransactionSource::TransactionHistoryItem,
        transaction: transaction(),
    };
    wrong.previous_sha256 = Some("9".repeat(64).parse().unwrap());
    assert!(s.apply(&wrong).is_err());
}
#[test]
fn independently_authored_event_digest_vector() {
    let bytes = include_bytes!("fixtures/genesis-v1.json");
    let expected = include_str!("fixtures/genesis-v1.sha256").trim();
    let e: Envelope = canonical::decode(bytes).unwrap();
    assert_eq!(canonical::sha256(bytes), expected);
    assert_eq!(canonical::encode(&e).unwrap(), bytes);
    let state = AccountState::default().apply(&e).unwrap();
    assert_eq!(state.head.as_deref(), Some(expected));
    let mut corrupt = e.clone();
    corrupt.schema_version = 2;
    assert!(AccountState::default().apply(&corrupt).is_err());
    let mut upgraded = e;
    upgraded.tool.package_version = "0.2.0".into();
    assert!(AccountState::default().apply(&upgraded).is_ok());
}
#[test]
fn endpoint_limits_are_separate_and_reboot_does_not_reset_them() {
    assert_eq!(EndpointClass::Allocation.quota().requests, 20);
    assert_eq!(EndpointClass::Catalogue.quota().requests, 500);
    assert_eq!(EndpointClass::Cancellation.quota().requests, 200);
    let mut b = BudgetWindow {
        endpoint: EndpointClass::Allocation,
        boot_id: time().boot_id,
        time_namespace: time().time_namespace,
        start_ns: 1,
        spent: 20,
        charges_ns: vec![1; 20],
    };
    assert!(b.charge(&time()).is_err());
    let mut reboot = time();
    reboot.boot_id = "00000000-0000-0000-0000-000000000002".into();
    assert!(b.charge(&reboot).is_err());
}
#[test]
fn exact_money_never_rounds_or_accepts_exponents() {
    assert_eq!(euro_units("12.3456").unwrap(), 123456);
    for price in ["12.34567", "-1.0000", "1e3", "12.3", "NaN"] {
        assert!(euro_units(price).is_err());
    }
}

fn approval(product: &str, account: &str, name: &str) -> ProductApproval {
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
fn identifiers_validate_once_without_changing_scalar_wire_bytes() {
    assert!("".parse::<OperationId>().is_err());
    assert!("account/other".parse::<AccountScopeId>().is_err());
    assert!("id?query=x".parse::<RobotTransactionId>().is_err());
    assert!("0".repeat(63).parse::<EventDigest>().is_err());
    assert!("A".repeat(64).parse::<RequestFingerprint>().is_err());
    assert!(ServerNumber::try_from(0).is_err());
    assert_eq!(
        canonical::encode(&"controller".parse::<AuthorityId>().unwrap()).unwrap(),
        b"\"controller\"\n"
    );
    assert_eq!(
        canonical::encode(&ServerNumber::try_from(123).unwrap()).unwrap(),
        b"123\n"
    );
    assert!(serde_json::from_str::<ServerNumber>("0").is_err());
    assert!(serde_json::from_str::<ProductId>("\"bad&field\"").is_err());
}

#[test]
fn product_approval_evidence_digest_is_an_active_admission_input() {
    let a = approval("catalogue-pin", "account", "reviewed catalogue spelling");
    let bytes = canonical::encode(&a).unwrap();
    assert_eq!(
        ProductApproval::verify_bytes(&bytes, &a.digest().unwrap()).unwrap(),
        a
    );
    let mut changed = a.clone();
    changed.catalogue.description = vec!["different processor".into()];
    assert!(
        ProductApproval::verify_bytes(&canonical::encode(&changed).unwrap(), &a.digest().unwrap())
            .is_err()
    );
    changed = a.clone();
    changed.hardware_class = "another class".into();
    assert!(changed.validate().is_err());
}

#[test]
fn independent_mutation_descriptor_byte_and_digest_goldens() {
    for (descriptor, bytes, digest) in [
        (
            MutationDescriptor::allocation(&request()).unwrap(),
            include_bytes!("fixtures/allocation-descriptor-v1.json").as_slice(),
            include_str!("fixtures/allocation-descriptor-v1.sha256"),
        ),
        (
            MutationDescriptor::cancellation(42.try_into().unwrap()).unwrap(),
            include_bytes!("fixtures/cancellation-descriptor-v1.json").as_slice(),
            include_str!("fixtures/cancellation-descriptor-v1.sha256"),
        ),
    ] {
        assert_eq!(canonical::encode(&descriptor).unwrap(), bytes);
        assert_eq!(descriptor.fingerprint().unwrap().as_str(), digest.trim());
        assert_eq!(
            canonical::decode::<MutationDescriptor>(bytes).unwrap(),
            descriptor
        );
    }
}

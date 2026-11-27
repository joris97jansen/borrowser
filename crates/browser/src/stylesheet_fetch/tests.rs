use super::*;
fn request() -> FetchRequest {
    FetchRequest::try_new(
        core_types::RequestId::default(),
        addressing::ResourceAddress::new(
            BrowserUrl::parse("https://example.test/style.css#original")
                .unwrap()
                .url,
            addressing::ResourceKind::Stylesheet,
        ),
        net::RequestMethod::Get,
        net::HttpHeaders::new(),
        None,
        None,
    )
    .unwrap()
}
fn accumulator() -> StylesheetFetchAccumulator {
    let request = request();
    let issued_key = request.address().retrieval_key();
    StylesheetFetchAccumulator {
        request,
        issued_key,
        origin_allowance: 0,
        response: None,
        policy: None,
        bytes: Vec::new(),
        observed_bytes: 0,
    }
}
#[test]
fn aj7_body_exact_boundary_one_over_moves_original_allocation() {
    let mut a = accumulator();
    a.append(&vec![7; BODY_BYTES]).unwrap();
    let pointer = a.bytes.as_ptr();
    let response = net::NetworkResponseInfo::http(
        a.request.address(),
        a.issued_key.as_str().into(),
        Some(a.issued_key.as_str().into()),
        net::HttpStatus::try_from(200).unwrap(),
        net::HttpHeaders::new(),
    )
    .unwrap();
    a.response = Some(response);
    a.policy = Some(StylesheetResponsePolicy {
        provenance: None,
        status: None,
        mime: None,
    });
    let r = a.finish(
        StylesheetFetchOutcome::Ready,
        None,
        StylesheetCollectionStop::Protocol,
    );
    assert_eq!(r.body().bytes().unwrap().as_ptr(), pointer);
    let mut a = accumulator();
    a.response = r.response.response().cloned();
    a.append(&vec![7; BODY_BYTES]).unwrap();
    assert_eq!(a.append(&[9]), Err(StylesheetRetentionFailure::BodyLimit));
    let r = a.finish(
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
            StylesheetRetentionFailure::BodyLimit,
        )),
        None,
        StylesheetCollectionStop::Retention,
    );
    let StylesheetBodyEvidence::Partial {
        bytes,
        observed_bytes,
        unretained_event_bytes,
        ..
    } = r.body
    else {
        panic!("partial")
    };
    assert_eq!(bytes.len(), BODY_BYTES);
    assert_eq!(observed_bytes, BODY_BYTES + 1);
    assert_eq!(unretained_event_bytes, 1);
}
#[test]
fn aj7_metadata_counts_shared_response_once_separate_base_key_abort_and_origin() {
    let q = request();
    let original = q.original_url().as_str().len();
    let base = metadata_charge(&q, 77, None).unwrap();
    assert_eq!(base, request_charge(&q, 77).unwrap() + 2 * original);
    let mut headers = net::HttpHeaders::new();
    headers
        .append("content-type", b"text/css; charset=utf-8")
        .unwrap();
    let r = net::NetworkResponseInfo::http(
        q.address(),
        q.address().retrieval_key().as_str().into(),
        Some("https://cdn.test/site.css".into()),
        net::HttpStatus::try_from(200).unwrap(),
        headers,
    )
    .unwrap();
    assert_eq!(
        metadata_charge(&q, 77, Some(&r)).unwrap(),
        base + response_charge(&r).unwrap() + outcome_charge(r.response_url().unwrap()).unwrap()
    );
    let clone = r.clone();
    assert_eq!(
        metadata_charge(&q, 77, Some(&clone)),
        metadata_charge(&q, 77, Some(&r))
    );
    assert!(
        request_charge_before_copy(q.original_url(), None, true).unwrap()
            >= metadata_charge(&q, 0, None).unwrap()
    );
}
#[test]
fn aj7_metadata_exact_boundary_and_one_over() {
    let q = request();
    let headers = net::HttpHeaders::new();
    let base = metadata_charge(
        &q,
        0,
        Some(
            &net::NetworkResponseInfo::http(
                q.address(),
                q.address().retrieval_key().as_str().into(),
                Some("https://cdn.test/s".into()),
                net::HttpStatus::try_from(200).unwrap(),
                headers.clone(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    // A direct constructor can retain a long reported URL even though ordinary
    // producer acquisition has independent response-head bounds. Charge admission
    // includes both reported and parsed URL owners, plus the installed-base clone.
    let r = net::NetworkResponseInfo::http(
        q.address(),
        q.address().retrieval_key().as_str().into(),
        Some("https://cdn.test/s".into()),
        net::HttpStatus::try_from(200).unwrap(),
        headers,
    )
    .unwrap();
    assert_eq!(
        metadata_charge(&q, METADATA_BYTES - base, Some(&r)),
        Some(METADATA_BYTES)
    );
    assert_eq!(
        metadata_charge(&q, METADATA_BYTES - base + 1, Some(&r)),
        Some(METADATA_BYTES + 1)
    );
    assert!(sum([usize::MAX, 1]).is_none());
}
#[test]
fn aj7_absent_empty_partial_and_redacted_debug_are_distinct() {
    let a = accumulator();
    let r = a.finish(
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(NetworkErrorKind::Timeout)),
        None,
        StylesheetCollectionStop::Network,
    );
    assert!(matches!(r.body(), StylesheetBodyEvidence::NotStarted));
    assert_ne!(r.body().bytes(), Some([].as_slice()));
    let r = StylesheetFetchResult::unissued(
        StylesheetRequestEvidence::NotConstructed,
        StylesheetFetchFailure::RequestDeliveryUnavailable,
    );
    assert!(matches!(r.body(), StylesheetBodyEvidence::Absent));
    for text in [format!("{r:?}"), format!("{r:#?}")] {
        assert!(text.len() < 2048);
        assert!(!text.contains("example.test"));
    }
}

#[test]
fn aj7_constructed_request_metadata_boundary_omits_over_budget_ownership() {
    let q = request();
    let charge = metadata_charge(&q, 0, None).unwrap();
    let admitted = StylesheetFetchAccumulator::try_new(request(), METADATA_BYTES - charge).unwrap();
    assert_eq!(
        metadata_charge(&admitted.request, admitted.origin_allowance, None),
        Some(METADATA_BYTES)
    );
    let refused = StylesheetFetchAccumulator::try_new(request(), METADATA_BYTES - charge + 1)
        .err()
        .unwrap();
    assert_eq!(
        refused.outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
            StylesheetRetentionFailure::RequestMetadataLimit
        ))
    );
    assert!(matches!(
        refused.request(),
        StylesheetRequestEvidence::ConstructedOmitted(StylesheetEvidenceOmission::MetadataLimit)
    ));
    assert!(refused.request().request().is_none());
    assert!(refused.request().issued_key().is_none());
    assert!(refused.coherent_evidence());
}

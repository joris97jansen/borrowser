use super::*;
use bytes::Bytes;
use http_body::{Frame, SizeHint};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll},
};

enum Step {
    Data(usize),
    Error,
    Pending,
    Trailer,
}
struct ProbeBody {
    steps: VecDeque<Step>,
    polls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
    hint: u64,
}
impl Drop for ProbeBody {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
impl Body for ProbeBody {
    type Data = Bytes;
    type Error = Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>>>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        match self.steps.pop_front() {
            Some(Step::Data(n)) => Poll::Ready(Some(Ok(Frame::data(Bytes::from(vec![b'x'; n]))))),
            Some(Step::Error) => Poll::Ready(Some(Err(Error("synthetic body error")))),
            Some(Step::Trailer) => Poll::Ready(Some(Ok(Frame::trailers(http::HeaderMap::new())))),
            Some(Step::Pending) => {
                self.steps.push_front(Step::Pending);
                Poll::Pending
            }
            None => Poll::Ready(None),
        }
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.hint)
    }
}
fn streamed(
    status: u16,
    steps: Vec<Step>,
    headers: &[(&str, &str)],
    hint: u64,
) -> (HttpResponse, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let polls = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let body = SdkBody::from_body_1_x(ProbeBody {
        steps: steps.into(),
        polls: polls.clone(),
        dropped: dropped.clone(),
        hint,
    });
    let mut response = http::Response::builder().status(status);
    for (key, value) in headers {
        response = response.header(*key, *value);
    }
    (
        response.body(body).unwrap().try_into().unwrap(),
        polls,
        dropped,
    )
}
#[derive(Debug)]
struct SyntheticConnector(Mutex<Option<HttpResponse>>);
impl HttpConnector for SyntheticConnector {
    fn call(&self, _: HttpRequest) -> HttpConnectorFuture {
        let response = self
            .0
            .lock()
            .unwrap()
            .take()
            .expect("exactly one invocation");
        HttpConnectorFuture::new(async move { Ok(response) })
    }
}
fn connector(response: HttpResponse, round: ObservationRound) -> BoundedConnector {
    BoundedConnector {
        inner: SharedHttpConnector::new(SyntheticConnector(Mutex::new(Some(response)))),
        round,
    }
}
fn request() -> HttpRequest {
    http::Request::builder()
        .uri("https://synthetic.invalid")
        .body(SdkBody::empty())
        .unwrap()
        .try_into()
        .unwrap()
}

#[tokio::test]
async fn success_and_error_are_fully_buffered_before_response_escapes() {
    for status in [200, 400, 403, 500] {
        for headers in [
            vec![],
            vec![("content-length", "1")],
            vec![("transfer-encoding", "chunked")],
        ] {
            let (response, polls, dropped) = streamed(
                status,
                vec![Step::Data(RESPONSE_BYTES as usize - 1), Step::Data(1)],
                &headers,
                0,
            );
            let round = ObservationRound::test();
            let response = connector(response, round.clone())
                .call(request())
                .await
                .unwrap();
            // The pinned SdkBody 1.x/0.4 adapter polls once for data EOF and
            // once for trailers EOF. Neither poll consumes another data frame.
            assert_eq!(polls.load(Ordering::SeqCst), 4);
            assert!(dropped.load(Ordering::SeqCst));
            assert_eq!(
                response.body().bytes().unwrap().len(),
                RESPONSE_BYTES as usize
            );
            assert_eq!(
                round.state.lock().unwrap().accounting.response_bytes(),
                RESPONSE_BYTES
            );
        }
    }
}
#[tokio::test]
async fn offending_frame_is_not_copied_and_rejected_stream_is_not_drained() {
    for status in [200, 400, 500] {
        let (response, polls, dropped) = streamed(
            status,
            vec![
                Step::Data(RESPONSE_BYTES as usize),
                Step::Data(1),
                Step::Data(100),
            ],
            &[("content-length", "1")],
            0,
        );
        let round = ObservationRound::test();
        let mut deserializer_entered = false;
        let result = connector(response, round.clone()).call(request()).await;
        if result.is_ok() {
            deserializer_entered = true;
        }
        assert!(!deserializer_entered);
        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(
            round.state.lock().unwrap().accounting.response_bytes(),
            RESPONSE_BYTES
        );
        assert_eq!(
            round.state.lock().unwrap().accounting.failure(),
            Some(LimitKind::ResponseBytes)
        );
    }
}
#[tokio::test]
async fn shared_round_budget_survives_distinct_connectors_and_service_responses() {
    let round = ObservationRound::test();
    for _ in 0..8 {
        let (response, _, _) = streamed(200, vec![Step::Data(RESPONSE_BYTES as usize)], &[], 0);
        connector(response, round.clone())
            .call(request())
            .await
            .unwrap();
    }
    let (response, polls, dropped) = streamed(500, vec![Step::Data(1), Step::Data(200)], &[], 0);
    assert!(
        connector(response, round.clone())
            .call(request())
            .await
            .is_err()
    );
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(
        round.state.lock().unwrap().accounting.response_bytes(),
        ROUND_RESPONSE_BYTES
    );
    assert_eq!(round.state.lock().unwrap().accounting.requests(), 9);
}
#[tokio::test]
async fn metadata_never_grants_acceptance_or_allocation() {
    for headers in [
        vec![("content-length", "-1")],
        vec![("content-length", "abc")],
        vec![("content-length", "18446744073709551616")],
        vec![("content-length", "1048577")],
        vec![("content-length", "1"), ("content-length", "1")],
        vec![("content-length", "1"), ("transfer-encoding", "chunked")],
        vec![("content-encoding", "gzip")],
    ] {
        let (response, polls, dropped) = streamed(200, vec![Step::Data(1)], &headers, 0);
        assert!(
            connector(response, ObservationRound::test())
                .call(request())
                .await
                .is_err()
        );
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert!(dropped.load(Ordering::SeqCst));
    }
    let (response, polls, _) = streamed(200, vec![Step::Data(1)], &[], u64::MAX);
    assert!(
        connector(response, ObservationRound::test())
            .call(request())
            .await
            .is_err()
    );
    assert_eq!(polls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn eof_is_required_and_cancellation_body_errors_and_trailers_drop_stream() {
    for last in [Step::Error, Step::Trailer] {
        let (response, polls, dropped) =
            streamed(200, vec![Step::Data(12), last, Step::Data(50)], &[], 0);
        let round = ObservationRound::test();
        assert!(
            connector(response, round.clone())
                .call(request())
                .await
                .is_err()
        );
        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(round.state.lock().unwrap().accounting.response_bytes(), 12);
    }
    let (response, polls, dropped) = streamed(
        200,
        vec![Step::Data(RESPONSE_BYTES as usize), Step::Pending],
        &[],
        0,
    );
    let round = ObservationRound::test();
    let future = connector(response, round.clone()).call(request());
    let mut future = Box::pin(future);
    let waker = std::task::Waker::noop();
    let mut cx = Context::from_waker(waker);
    assert!(std::future::Future::poll(future.as_mut(), &mut cx).is_pending());
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert!(!dropped.load(Ordering::SeqCst));
    drop(future);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::Cancelled)
    );
}
#[test]
fn clock_domain_deadline_session_and_no_refund_contract() {
    struct Clock(Mutex<TimeSample>);
    impl ObservationClock for Clock {
        fn sample(&self) -> Result<TimeSample> {
            Ok(self.0.lock().unwrap().clone())
        }
    }
    let clock = Arc::new(Clock(Mutex::new(crate::test_support::genesis().time)));
    let round = ObservationRound::with_clock(clock.clone()).unwrap();
    clock.0.lock().unwrap().boottime_ns += OBSERVATION_NS - 1;
    assert_eq!(round.remaining().unwrap(), Duration::from_nanos(1));
    clock.0.lock().unwrap().boottime_ns += 1;
    assert!(round.remaining().is_err());
    let round = ObservationRound::test();
    assert!(round.bind_expiration(SystemTime::UNIX_EPOCH).is_err());
    assert!(
        round
            .bind_expiration(SystemTime::now() + Duration::from_secs(1000))
            .is_err()
    );
    let clock = Arc::new(Clock(Mutex::new(crate::test_support::genesis().time)));
    let round = ObservationRound::with_clock(clock.clone()).unwrap();
    clock.0.lock().unwrap().time_namespace = "time:[other]".into();
    assert!(round.remaining().is_err());
    clock.0.lock().unwrap().boottime_ns = u64::MAX;
    assert!(ObservationRound::with_clock(clock).is_err());
}

#[tokio::test(start_paused = true)]
async fn pending_body_times_out_and_is_dropped_without_a_response() {
    let (response, polls, dropped) = streamed(200, vec![Step::Data(7), Step::Pending], &[], 0);
    let round = ObservationRound::test();
    let result = connector(response, round.clone()).call(request()).await;
    assert!(result.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert!(polls.load(Ordering::SeqCst) >= 2);
    assert_eq!(round.state.lock().unwrap().accounting.response_bytes(), 7);
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::Elapsed)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn production_round_uses_linux_boot_clock() {
    let round = ObservationRound::start().unwrap();
    assert!(round.remaining().unwrap() <= Duration::from_secs(300));
    assert!(!round.start.boot_id.is_empty());
    assert!(round.start.time_namespace.starts_with("time:["));
}

#[test]
fn invalid_normalized_record_latches_the_shared_round() {
    use crate::provider::observation::{
        EvidenceList, ObservationDataV1, ObservationRecordV1, Observed, Tag,
    };
    let mut record = ObservationRecordV1::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    // Closed typed data can still exceed its canonical record budget.
    record.data = ObservationDataV1::Volume {
        id: "vol-01".parse().unwrap(),
        zone: Observed::Absent,
        snapshot: Observed::Absent,
        volume_type: Observed::Absent,
        size_gib: Observed::Absent,
        iops: Observed::Absent,
        throughput_mib_s: Observed::Absent,
        encrypted: Observed::Absent,
        key: Observed::Absent,
        multi_attach: Observed::Absent,
        tags: Observed::Present(
            EvidenceList::try_from(vec![
                Tag {
                    key: "x".repeat(2048).try_into().unwrap(),
                    value: "x".repeat(2048).try_into().unwrap(),
                };
                4
            ])
            .unwrap(),
        ),
    };
    let round = ObservationRound::test();
    assert!(round.canonical_record(&record).is_err());
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::RecordBytes)
    );
    assert!(round.request().is_err());
    assert!(round.records(0).is_err());
}

#[tokio::test]
async fn rejected_iam_attribute_retains_charges_and_cannot_reuse_capture() {
    use crate::aws::{
        configuration,
        iam_presence::capture_instance_profile,
        identity_observation_tests::iam_xml,
        tests::{replay, response, secret},
    };
    use aws_smithy_runtime_api::client::interceptors::SharedInterceptor;
    let body =
        iam_xml("<InstanceProfile><Arn xmlns:x=\"&bogus;\">returned</Arn></InstanceProfile>");
    let transport = replay(vec![response(200, &body, None)]);
    let round = ObservationRound::test();
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(SharedHttpClient::new(transport.clone()), round.clone()),
    )
    .unwrap();
    let client = aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
    let (hook, receiver) = capture_instance_profile(round.clone());
    let hook = SharedInterceptor::new(hook);
    for _ in 0..2 {
        assert!(
            client
                .get_instance_profile()
                .instance_profile_name("reviewed")
                .customize()
                .interceptor(hook.clone())
                .send()
                .await
                .is_err()
        );
        assert_eq!(transport.actual_requests().count(), 1);
        let state = round.state.lock().unwrap();
        assert_eq!(state.accounting.requests(), 1);
        assert_eq!(state.accounting.response_bytes(), body.len() as u64);
    }
    assert!(receiver.take().is_err());
    let state = round.state.lock().unwrap();
    assert_eq!(state.accounting.requests(), 1);
    assert_eq!(state.accounting.response_bytes(), body.len() as u64);
}

#[test]
fn v2_canonical_records_share_bounds_and_latch_without_refund() {
    use crate::provider::{
        observation::{Observed, Tag},
        observation_v2::{ObservationDataV2, ObservationRecordV2},
    };
    let valid = ObservationRecordV2::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v2/instance.json"
    ))
    .unwrap();
    let round = ObservationRound::test();
    assert_eq!(
        round.canonical_record_v2(&valid).unwrap(),
        valid.canonical_bytes().unwrap()
    );
    let mut invalid = valid.clone();
    if let ObservationDataV2::Instance { tags, .. } = &mut invalid.data {
        *tags = Observed::Present(
            vec![
                Tag {
                    key: "x".repeat(2048).try_into().unwrap(),
                    value: "x".repeat(2048).try_into().unwrap()
                };
                4
            ]
            .try_into()
            .unwrap(),
        );
    }
    assert!(round.canonical_record_v2(&invalid).is_err());
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::RecordBytes)
    );
    assert!(round.canonical_record_v2(&valid).is_err());
    assert!(round.records(0).is_err());

    let round = ObservationRound::test();
    let v1 = crate::provider::observation::ObservationRecordV1::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    round.canonical_record(&v1).unwrap();
    let remaining = NORMALIZED_BYTES as usize - v1.canonical_bytes().unwrap().len();
    let count = remaining / valid.canonical_bytes().unwrap().len();
    for _ in 0..count {
        round.canonical_record_v2(&valid).unwrap();
    }
    assert!(round.canonical_record_v2(&valid).is_err());
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::NormalizedBytes)
    );
    assert!(round.canonical_record(&v1).is_err());
}

#[test]
fn v3_canonical_accounting_shares_v1_v2_budgets_and_failure_latches() {
    use crate::provider::{
        observation::ObservationRecordV1, observation_v2::ObservationRecordV2,
        observation_v3::ObservationRecordV3,
    };
    let v1 = ObservationRecordV1::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    let v2 = ObservationRecordV2::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v2/instance.json"
    ))
    .unwrap();
    let v3 = ObservationRecordV3::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v3/profile.json"
    ))
    .unwrap();
    let round = ObservationRound::test();
    let initial =
        round.canonical_record(&v1).unwrap().len() + round.canonical_record_v2(&v2).unwrap().len();
    let len = v3.canonical_bytes().unwrap().len();
    let capacity = (NORMALIZED_BYTES as usize - initial) / len;
    for _ in 0..capacity {
        round.canonical_record_v3(&v3).unwrap();
    }
    assert!(round.canonical_record_v3(&v3).is_err());
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::NormalizedBytes)
    );
    assert!(round.canonical_record(&v1).is_err());
    assert!(round.canonical_record_v2(&v2).is_err());
    assert!(round.request().is_err());
    let round = ObservationRound::test();
    let mut invalid = v3;
    invalid.schema_version = 2;
    assert!(round.canonical_record_v3(&invalid).is_err());
    assert_eq!(
        round.state.lock().unwrap().accounting.failure(),
        Some(LimitKind::RecordBytes)
    );
    assert!(round.records(0).is_err());
}

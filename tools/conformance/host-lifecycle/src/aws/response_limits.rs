//! Bounded PRE-deserialization buffering. Smithy never sees the original stream.
use crate::{Error, Result, provider::limits::*, scheduling::TimeSample};
use aws_smithy_runtime_api::client::{
    http::{
        HttpClient, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpClient,
        SharedHttpConnector,
    },
    orchestrator::{HttpRequest, HttpResponse},
    result::ConnectorError,
    runtime_components::{RuntimeComponents, RuntimeComponentsBuilder},
};
use aws_smithy_types::{body::SdkBody, config_bag::ConfigBag};
use http_body::Body;
use http_body_util::BodyExt;
use std::{
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

pub(super) trait ObservationClock: Send + Sync {
    fn sample(&self) -> Result<TimeSample>;
}
struct ControllerClock;
impl ObservationClock for ControllerClock {
    fn sample(&self) -> Result<TimeSample> {
        #[cfg(target_os = "linux")]
        {
            crate::linux::now()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error("observation requires Linux clock"))
        }
    }
}
struct State {
    accounting: ObservationAccounting,
    last: u64,
    expires: Option<SystemTime>,
}
#[derive(Clone)]
pub(super) struct ObservationRound {
    clock: Arc<dyn ObservationClock>,
    start: TimeSample,
    state: Arc<Mutex<State>>,
}
impl fmt::Debug for ObservationRound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObservationRound")
    }
}
impl ObservationRound {
    /// E2 charges decoded occurrences before filtering/deduplication. No SDK normalization here.
    pub(super) fn records(&self, count: u64) -> Result<()> {
        self.remaining()?;
        self.state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?
            .accounting
            .records(count)
    }
    pub(super) fn canonical_record(
        &self,
        record: &crate::provider::observation::ObservationRecordV1,
    ) -> Result<Vec<u8>> {
        self.remaining()?;
        record
            .canonical_bytes()
            .inspect_err(|_| self.fail(LimitKind::RecordBytes))?;
        let bytes = self
            .state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?
            .accounting
            .canonical_record(record)?;
        self.remaining()?;
        Ok(bytes)
    }
    /// Separate validated successor path; V1 accounting and its validator stay frozen.
    pub(super) fn canonical_record_v2(
        &self,
        record: &crate::provider::observation_v2::ObservationRecordV2,
    ) -> Result<Vec<u8>> {
        self.remaining()?;
        record
            .canonical_bytes()
            .inspect_err(|_| self.fail(LimitKind::RecordBytes))?;
        let bytes = self
            .state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?
            .accounting
            .canonical_record(record)?;
        self.remaining()?;
        Ok(bytes)
    }
    /// Narrow successor path sharing all existing round counters and failure latches.
    pub(super) fn canonical_record_v3(
        &self,
        record: &crate::provider::observation_v3::ObservationRecordV3,
    ) -> Result<Vec<u8>> {
        self.remaining()?;
        record
            .canonical_bytes()
            .inspect_err(|_| self.fail(LimitKind::RecordBytes))?;
        let bytes = self
            .state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?
            .accounting
            .canonical_record(record)?;
        self.remaining()?;
        Ok(bytes)
    }
    pub(super) fn start() -> Result<Self> {
        Self::with_clock(Arc::new(ControllerClock))
    }
    fn with_clock(clock: Arc<dyn ObservationClock>) -> Result<Self> {
        let start = clock.sample()?;
        start.validate()?;
        start
            .boottime_ns
            .checked_add(OBSERVATION_NS)
            .ok_or(Error("observation deadline overflow"))?;
        Ok(Self {
            clock,
            state: Arc::new(Mutex::new(State {
                accounting: ObservationAccounting::default(),
                last: start.boottime_ns,
                expires: None,
            })),
            start,
        })
    }
    pub(super) fn bind_expiration(&self, expiration: SystemTime) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?;
        state.expires = Some(state.expires.map_or(expiration, |old| old.min(expiration)));
        drop(state);
        self.remaining().map(|_| ())
    }
    pub(super) fn remaining(&self) -> Result<Duration> {
        let now = self
            .clock
            .sample()
            .inspect_err(|_| self.fail(LimitKind::Clock))?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?;
        if !now.same_clock(&self.start) || now.boottime_ns < state.last {
            state.accounting.fail(LimitKind::Clock);
        }
        let elapsed = now
            .boottime_ns
            .checked_sub(self.start.boottime_ns)
            .unwrap_or(u64::MAX);
        if elapsed >= OBSERVATION_NS {
            state.accounting.fail(LimitKind::Elapsed);
        }
        state.last = now.boottime_ns;
        let session = state.expires.map(|e| e.duration_since(SystemTime::now()));
        if session
            .as_ref()
            .is_some_and(|r| r.as_ref().map_or(true, |d| d.is_zero()))
        {
            state.accounting.fail(LimitKind::Session);
        }
        crate::require(
            state.accounting.failure().is_none(),
            "observation round unavailable",
        )?;
        let remaining = Duration::from_nanos(OBSERVATION_NS - elapsed);
        Ok(session
            .and_then(|r| r.ok())
            .map_or(remaining, |d| remaining.min(d)))
    }
    fn request(&self) -> Result<()> {
        self.remaining()?;
        self.state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?
            .accounting
            .request()
    }
    fn frame(&self, response_bytes: u64, count: u64) -> Result<()> {
        self.remaining()?;
        self.state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?
            .accounting
            .frame(response_bytes, count)
    }
    fn fail(&self, failure: LimitKind) {
        if let Ok(mut state) = self.state.lock() {
            state.accounting.fail(failure);
        }
    }
    fn available_body_bytes(&self) -> Result<u64> {
        let state = self
            .state
            .lock()
            .map_err(|_| Error("observation state poisoned"))?;
        Ok(RESPONSE_BYTES.min(ROUND_RESPONSE_BYTES - state.accounting.response_bytes()))
    }
    #[cfg(test)]
    pub(super) fn test() -> Self {
        struct Clock(std::time::Instant);
        impl ObservationClock for Clock {
            fn sample(&self) -> Result<TimeSample> {
                let mut t = crate::test_support::genesis().time;
                t.boottime_ns = self.0.elapsed().as_nanos().try_into().unwrap();
                Ok(t)
            }
        }
        Self::with_clock(Arc::new(Clock(std::time::Instant::now()))).unwrap()
    }
}
fn connector_error(error: Error) -> ConnectorError {
    ConnectorError::other(Box::new(error), None)
}
struct CancellationGuard {
    round: ObservationRound,
    finished: bool,
}
impl Drop for CancellationGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.round.fail(LimitKind::Cancelled);
        }
    }
}

/// Only this wrapper can enter application SDK configuration. The underlying client
/// cannot escape, and all services share the same round state.
pub(super) struct BoundedHttp {
    inner: SharedHttpClient,
    round: ObservationRound,
}
impl BoundedHttp {
    pub(super) fn new(inner: SharedHttpClient, round: ObservationRound) -> Self {
        Self { inner, round }
    }
    pub(super) fn into_shared(self) -> SharedHttpClient {
        SharedHttpClient::new(self)
    }
    pub(super) fn round(&self) -> &ObservationRound {
        &self.round
    }
}
impl fmt::Debug for BoundedHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundedHttp")
    }
}
impl HttpClient for BoundedHttp {
    fn http_connector(
        &self,
        settings: &HttpConnectorSettings,
        components: &RuntimeComponents,
    ) -> SharedHttpConnector {
        SharedHttpConnector::new(BoundedConnector {
            inner: self.inner.http_connector(settings, components),
            round: self.round.clone(),
        })
    }
    fn validate_base_client_config(
        &self,
        components: &RuntimeComponentsBuilder,
        config: &ConfigBag,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.validate_base_client_config(components, config)
    }
    fn validate_final_config(
        &self,
        components: &RuntimeComponents,
        config: &ConfigBag,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.validate_final_config(components, config)
    }
    fn connector_metadata(
        &self,
    ) -> Option<aws_smithy_runtime_api::client::connector_metadata::ConnectorMetadata> {
        self.inner.connector_metadata()
    }
}
#[derive(Debug)]
struct BoundedConnector {
    inner: SharedHttpConnector,
    round: ObservationRound,
}
impl HttpConnector for BoundedConnector {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let inner = self.inner.clone();
        let round = self.round.clone();
        HttpConnectorFuture::new(async move {
            round.request().map_err(connector_error)?;
            let mut guard = CancellationGuard {
                round: round.clone(),
                finished: false,
            };
            let duration = round
                .remaining()
                .map_err(connector_error)?
                .min(Duration::from_secs(30));
            let operation = async {
                let response = inner.call(request).await?;
                buffer_response(response, &round)
                    .await
                    .map_err(connector_error)
            };
            let result = match tokio::time::timeout(duration, operation).await {
                Ok(result) => result,
                Err(_) => {
                    round.fail(LimitKind::Elapsed);
                    Err(connector_error(Error("bounded response timeout")))
                }
            };
            if result.is_err() {
                round.fail(LimitKind::Body);
            }
            guard.finished = true;
            result
        })
    }
}

async fn buffer_response(
    mut response: HttpResponse,
    round: &ObservationRound,
) -> Result<HttpResponse> {
    round.remaining()?;
    let headers = response.headers();
    let lengths: Vec<_> = headers.get_all("content-length").collect();
    crate::require(lengths.len() <= 1, "duplicate Content-Length")?;
    if let Some(length) = lengths.first() {
        crate::require(
            !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()),
            "invalid Content-Length",
        )?;
        let length: u64 = length
            .parse()
            .map_err(|_| Error("invalid Content-Length"))?;
        if length > round.available_body_bytes()? {
            round.fail(LimitKind::ResponseBytes);
            return Err(Error("response declared too large"));
        }
    }
    crate::require(
        headers.get_all("content-encoding").all(|v| v == "identity"),
        "unsupported content encoding",
    )?;
    crate::require(
        headers
            .get_all("transfer-encoding")
            .all(|v| v.eq_ignore_ascii_case("chunked")),
        "unsupported transfer encoding",
    )?;
    crate::require(
        lengths.is_empty() || headers.get("transfer-encoding").is_none(),
        "ambiguous response framing",
    )?;
    let mut body = std::mem::replace(response.body_mut(), SdkBody::taken());
    // A size hint is used only to reject, never to reserve or accept.
    if body.size_hint().lower() > round.available_body_bytes()? {
        round.fail(LimitKind::ResponseBytes);
        return Err(Error("response minimum too large"));
    }
    let mut accepted = Vec::new();
    let mut frames = 0u8;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| Error("response stream failed"))?;
        let data = frame
            .into_data()
            .map_err(|_| Error("response trailers unsupported"))?;
        round.frame(accepted.len() as u64, data.len() as u64)?;
        let needed = accepted
            .len()
            .checked_add(data.len())
            .ok_or(Error("response length overflow"))?;
        if needed > accepted.capacity() {
            let capacity = needed
                .max(accepted.capacity().saturating_mul(2))
                .min(RESPONSE_BYTES as usize);
            accepted
                .try_reserve_exact(capacity - accepted.len())
                .map_err(|_| Error("response allocation failed"))?;
        }
        accepted.extend_from_slice(&data);
        frames += 1;
        if frames == 64 {
            frames = 0;
            tokio::task::yield_now().await;
        }
    }
    round.remaining()?;
    // This is the first point at which Smithy may receive any response body.
    *response.body_mut() = SdkBody::from(accepted);
    Ok(response)
}

#[cfg(test)]
#[path = "read_transport_tests.rs"]
mod tests;

//! One-row Fetch collection. Producer closure is not a worker join.
use super::{bounds, input::Route, observations::*};
use crate::model::HarnessError;
use net::{
    NetEvent,
    fake::{FakeAcquisitionEvent, FakeAcquisitionResult, FakeNetwork},
};
use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProducerClosure {
    Closed,
    Unresolved,
}
#[derive(Debug)]
pub(crate) struct Failure {
    pub primary: HarnessError,
    pub closure: ProducerClosure,
}
impl From<HarnessError> for Failure {
    fn from(primary: HarnessError) -> Self {
        Self {
            primary,
            closure: ProducerClosure::Closed,
        }
    }
}

#[derive(Debug)]
pub(super) enum Message {
    Entered(RequestObservation),
    Returned(AcquisitionResult),
    Start {
        request_id: u64,
        response: ResponseObservation,
    },
    Chunk {
        request_id: u64,
        url: String,
        bytes: Vec<u8>,
    },
    Done {
        request_id: u64,
        response: ResponseObservation,
        progress: Progress,
    },
    Error {
        request_id: u64,
        url: String,
        error: NetworkError,
        response: Available<ResponseObservation>,
        progress: Available<Progress>,
    },
}
impl Message {
    fn acquisition(event: FakeAcquisitionEvent) -> Result<Self, HarnessError> {
        Ok(match event {
            FakeAcquisitionEvent::Entered { request, route } => {
                Self::Entered(RequestObservation::project(&request, route.into())?)
            }
            FakeAcquisitionEvent::Returned { result } => Self::Returned(match result {
                FakeAcquisitionResult::Response(r) => {
                    AcquisitionResult::Response(Box::new(ResponseObservation::project(&r)?))
                }
                FakeAcquisitionResult::NetworkError(e) => AcquisitionResult::NetworkError(e.into()),
                FakeAcquisitionResult::MissingRoute => AcquisitionResult::MissingRoute,
                FakeAcquisitionResult::ResponseConstructionRejected(e) => {
                    AcquisitionResult::ResponseConstructionRejected(e.into())
                }
            }),
        })
    }
    fn event(e: NetEvent) -> Result<Self, HarnessError> {
        Ok(match e {
            NetEvent::Start {
                request_id,
                response,
            } => Self::Start {
                request_id,
                response: ResponseObservation::project(&response)?,
            },
            NetEvent::Chunk {
                request_id,
                url,
                chunk,
            } => {
                if url.len().saturating_add(chunk.capacity()) > bounds::RECORD {
                    return Err(error("chunk record budget exceeded"));
                }
                Self::Chunk {
                    request_id,
                    url,
                    bytes: chunk,
                }
            }
            NetEvent::Done {
                request_id,
                response,
                body,
            } => Self::Done {
                request_id,
                response: ResponseObservation::project(&response)?,
                progress: body.into(),
            },
            NetEvent::Error {
                request_id,
                url,
                error_kind,
                response,
                body,
                ..
            } => {
                if url.len() > 4096 {
                    return Err(error("event URL budget exceeded"));
                }
                Self::Error {
                    request_id,
                    url,
                    error: error_kind.into(),
                    response: response
                        .as_ref()
                        .map(ResponseObservation::project)
                        .transpose()?
                        .map_or_else(
                            || Available::Unavailable("no_acquired_response".into()),
                            Available::Present,
                        ),
                    progress: body.map_or_else(
                        || Available::Unavailable("no_body_progress".into()),
                        |b| Available::Present(b.into()),
                    ),
                }
            }
        })
    }
    fn storage(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::Entered(r) => request_storage(r),
                Self::Returned(AcquisitionResult::Response(r)) => response_storage(r),
                Self::Returned(_) => 0,
                Self::Start { response, .. } | Self::Done { response, .. } => {
                    response_storage(response)
                }
                Self::Chunk { url, bytes, .. } => url.capacity() + bytes.capacity(),
                Self::Error {
                    url,
                    response,
                    progress,
                    ..
                } => {
                    url.capacity()
                        + available_response_storage(response)
                        + available_progress_storage(progress)
                }
            }
    }
}

/// Each callback exclusively owns one endpoint. Only these endpoints own lifetime
/// senders; no message is ever sent through the independent lifetime channel.
pub(super) struct Endpoint {
    pub data: mpsc::SyncSender<Message>,
    pub alive: mpsc::Sender<Infallible>,
    pub cancel: Arc<AtomicBool>,
    pub delivery: Arc<AtomicU8>,
}
impl Endpoint {
    pub(super) fn send(&self, message: Result<Message, HarnessError>) {
        let _lifetime = &self.alive;
        let message = match message {
            Ok(m) if m.storage() <= bounds::RECORD => m,
            _ => {
                self.fail(1);
                return;
            }
        };
        if self.data.send(message).is_err() {
            self.fail(2);
        }
    }
    fn fail(&self, code: u8) {
        let _ = self
            .delivery
            .compare_exchange(0, code, Ordering::AcqRel, Ordering::Acquire);
        self.cancel.store(true, Ordering::Release);
    }
}

pub(super) struct Channels {
    pub data: mpsc::Receiver<Message>,
    pub alive: mpsc::Receiver<Infallible>,
    pub cancel: Arc<AtomicBool>,
    pub delivery: Arc<AtomicU8>,
}
pub(super) fn channels(cancel: Arc<AtomicBool>) -> (Channels, Endpoint, Endpoint) {
    let (tx, rx) = mpsc::sync_channel(bounds::QUEUE);
    let (alive, closed) = mpsc::channel();
    let delivery = Arc::new(AtomicU8::new(0));
    let endpoint = Endpoint {
        data: tx.clone(),
        alive: alive.clone(),
        cancel: cancel.clone(),
        delivery: delivery.clone(),
    };
    let observer = Endpoint {
        data: tx,
        alive,
        cancel: cancel.clone(),
        delivery: delivery.clone(),
    };
    (
        Channels {
            data: rx,
            alive: closed,
            cancel,
            delivery,
        },
        endpoint,
        observer,
    )
}

pub(super) fn closure_until(rx: &mpsc::Receiver<Infallible>, deadline: Instant) -> ProducerClosure {
    match rx.try_recv() {
        Err(mpsc::TryRecvError::Disconnected) => return ProducerClosure::Closed,
        Ok(v) => match v {},
        Err(mpsc::TryRecvError::Empty) => {}
    }
    let now = Instant::now();
    if now >= deadline {
        return ProducerClosure::Unresolved;
    }
    match rx.recv_timeout(deadline - now) {
        Err(mpsc::RecvTimeoutError::Disconnected) => ProducerClosure::Closed,
        Err(mpsc::RecvTimeoutError::Timeout) => ProducerClosure::Unresolved,
        Ok(v) => match v {},
    }
}

fn failed(ch: Channels, primary: HarnessError) -> Failure {
    ch.cancel.store(true, Ordering::Release);
    drop(ch.data); // Releases every blocked synchronous send, without draining.
    let closure = closure_until(&ch.alive, Instant::now() + Duration::from_secs(1));
    Failure { primary, closure }
}
fn deadline(deadline: Instant) -> Result<(), HarnessError> {
    if Instant::now() >= deadline {
        Err(HarnessError::new(
            "networking.deadline",
            "absolute collection deadline exceeded",
        ))
    } else {
        Ok(())
    }
}

pub(super) struct Collector {
    pub request: RequestObservation,
    pub request_id: u64,
    pub issued: String,
    attempts: Vec<Attempt>,
    pending: Option<RequestObservation>,
    start: Option<ResponseObservation>,
    terminal: Option<Terminal>,
    body: BodyAccumulator,
    storage: usize,
    storage_limit: usize,
}
impl Collector {
    pub(super) fn new(
        request: RequestObservation,
        request_id: u64,
        issued: String,
        storage_limit: usize,
    ) -> Self {
        let storage = size_of::<Self>() + request_storage(&request) + issued.capacity();
        Self {
            request,
            request_id,
            issued,
            attempts: Vec::new(),
            pending: None,
            start: None,
            terminal: None,
            body: BodyAccumulator::new(),
            storage,
            storage_limit,
        }
    }
    fn consume(&mut self, m: Message) -> Result<(), HarnessError> {
        if self.terminal.is_some() {
            return Err(error("record after terminal event"));
        }
        // Charge before moving the record into retained observations. Chunk bytes
        // are streamed into the summary and never retained as whole-body data.
        if !matches!(m, Message::Chunk { .. }) {
            self.storage = self
                .storage
                .checked_add(m.storage())
                .ok_or_else(|| error("row storage overflow"))?;
            if self.storage > self.storage_limit {
                return Err(error("row storage budget exceeded"));
            }
        }
        match m {
            Message::Entered(r) => {
                if self.pending.is_some() || self.start.is_some() {
                    return Err(error("acquisition entry ordering"));
                }
                if self.attempts.len() >= 64 {
                    return Err(error("acquisition count budget exceeded"));
                }
                self.pending = Some(r);
            }
            Message::Returned(result) => {
                let request = self
                    .pending
                    .take()
                    .ok_or_else(|| error("acquisition return without entry"))?;
                self.attempts.push(Attempt { request, result });
            }
            Message::Start {
                request_id,
                response,
            } => {
                self.correlate(request_id, None)?;
                if self.pending.is_some()
                    || self.start.is_some()
                    || !matches!(
                        self.attempts.last().map(|a| &a.result),
                        Some(AcquisitionResult::Response(_))
                    )
                {
                    return Err(error("response Start ordering"));
                }
                self.start = Some(response);
            }
            Message::Chunk {
                request_id,
                url,
                bytes,
            } => {
                self.correlate(request_id, Some(&url))?;
                if self.start.is_none() {
                    return Err(error("chunk before Start"));
                }
                self.body.update(&bytes)?;
                let cap = net::BodyProgress::try_new(self.request.route.into(), 0)
                    .map_err(|_| error("invalid route"))?
                    .limit_bytes();
                if self.body.len() > cap {
                    return Err(error("delivered body exceeds production route cap"));
                }
            }
            Message::Done {
                request_id,
                response,
                progress,
            } => {
                self.correlate(request_id, None)?;
                self.progress(&progress)?;
                if self.pending.is_some() || self.start.as_ref() != Some(&response) {
                    return Err(error("Done without immutable Start"));
                }
                self.terminal = Some(Terminal::Done { response, progress });
            }
            Message::Error {
                request_id,
                url,
                error: kind,
                response,
                progress,
            } => {
                self.correlate(request_id, Some(&url))?;
                if self.pending.is_some() {
                    return Err(error("terminal before acquisition return"));
                }
                if let Some(start) = &self.start
                    && !matches!(&response,Available::Present(r) if r==start)
                {
                    return Err(error("Error changed Start metadata"));
                }
                if let Available::Present(p) = &progress {
                    self.progress(p)?;
                } else if self.start.is_some() || self.body.len() != 0 {
                    return Err(error("missing delivered body progress"));
                }
                self.terminal = Some(Terminal::Error {
                    error: kind,
                    response,
                    progress,
                });
            }
        }
        Ok(())
    }
    fn correlate(&self, id: u64, url: Option<&str>) -> Result<(), HarnessError> {
        if id != self.request_id || url.is_some_and(|u| u != self.issued) {
            Err(error("event issuance correlation mismatch"))
        } else {
            Ok(())
        }
    }
    fn progress(&self, p: &Progress) -> Result<(), HarnessError> {
        if p.route != self.request.route || p.delivered_bytes != self.body.len() {
            Err(error("body progress mismatch"))
        } else {
            Ok(())
        }
    }
    fn finish(self) -> Result<FetchObservation, HarnessError> {
        if self.pending.is_some() {
            return Err(error("missing acquisition return"));
        }
        let terminal = self
            .terminal
            .ok_or_else(|| error("missing terminal event"))?;
        let final_copy = self
            .start
            .as_ref()
            .and_then(|r| match &r.response_url {
                ResponseUrl::Parsed { url, .. } => Some(url.len()),
                _ => None,
            })
            .unwrap_or(64);
        if self.storage.saturating_add(final_copy) > self.storage_limit {
            return Err(error(
                "row storage budget exceeded before final URL projection",
            ));
        }
        let selected_final_url = self.start.as_ref().map_or_else(
            || Available::Unavailable("no_selected_response_start".into()),
            |r| match &r.response_url {
                ResponseUrl::Parsed { url, .. } => Available::Present(url.clone()),
                _ => Available::Unavailable("selected_response_url_unavailable".into()),
            },
        );
        let row = FetchObservation {
            request: Available::Present(self.request),
            acquisitions: self.attempts,
            start: self
                .start
                .map_or(Available::Inapplicable, Available::Present),
            terminal,
            selected_final_url,
            body: self.body.finish(),
        };
        row.validate()?;
        Ok(row)
    }
}

pub(super) fn collect(
    ch: Channels,
    mut state: Collector,
    until: Instant,
) -> Result<FetchObservation, Failure> {
    let result = (|| {
        loop {
            deadline(until)?;
            if ch.delivery.load(Ordering::Acquire) != 0 {
                return Err(error("callback projection or delivery failed"));
            }
            match ch
                .data
                .recv_timeout(until.saturating_duration_since(Instant::now()))
            {
                Ok(m) => {
                    deadline(until)?;
                    state.consume(m)?;
                    deadline(until)?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(HarnessError::new(
                        "networking.deadline",
                        "absolute collection deadline exceeded",
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        // Data disconnection occurs only after all queued records were consumed.
        deadline(until)?;
        if closure_until(&ch.alive, until) != ProducerClosure::Closed {
            return Err(error("producer lifetime did not close"));
        }
        if ch.delivery.load(Ordering::Acquire) != 0 {
            return Err(error("callback delivery failed"));
        }
        deadline(until)?;
        let row = state.finish()?;
        deadline(until)?;
        Ok(row)
    })();
    match result {
        Ok(row) => Ok(row),
        Err(e) => Err(failed(ch, e)),
    }
}

pub(super) fn run(
    network: FakeNetwork,
    request: net::FetchRequest,
    route: Route,
    cancel: Arc<AtomicBool>,
    expected_storage: usize,
) -> Result<FetchObservation, Failure> {
    let projected = RequestObservation::project(&request, route)?;
    let issued = request.legacy_issued_url(route.into());
    let request_id = request.request_id();
    let (ch, events, attempts) = channels(cancel.clone());
    let until = Instant::now() + Duration::from_secs(10);
    let dispatch = network.fetch_stream_observed(
        request,
        route.into(),
        cancel,
        Arc::new(move |e| events.send(Message::event(e))),
        Arc::new(move |a| attempts.send(Message::acquisition(a))),
    );
    // Both callbacks were moved, not cloned, into dispatch. No runner-owned sender remains.
    if let Err(e) = dispatch {
        if !matches!(ch.data.try_recv(), Err(mpsc::TryRecvError::Disconnected))
            || !matches!(ch.alive.try_recv(), Err(mpsc::TryRecvError::Disconnected))
            || ch.delivery.load(Ordering::Acquire) != 0
        {
            return Err(failed(
                ch,
                error("rejected dispatch retained producers or emitted records"),
            ));
        }
        return Ok(FetchObservation {
            request: Available::Present(projected),
            acquisitions: vec![],
            start: Available::Inapplicable,
            terminal: Terminal::DispatchRejected { error: e.into() },
            selected_final_url: Available::Inapplicable,
            body: Body::Bytes { bytes: vec![] },
        });
    }
    let issued = match issued {
        Ok(issued) => issued,
        Err(_) => {
            return Err(failed(
                ch,
                error("accepted dispatch has no issuance provenance"),
            ));
        }
    };
    collect(
        ch,
        Collector::new(
            projected,
            request_id,
            issued,
            bounds::ROW_STORAGE.saturating_sub(expected_storage),
        ),
        until,
    )
}

use super::{
    error::{Error, Phase, ProtocolError},
    process::{Cancellation, OwnedChromium, poll_fd},
};
use crate::environment::{CONTENT_TYPE, URL};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    os::fd::AsRawFd,
    time::Instant,
};

const MAX_FRAME: usize = 4 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventSession {
    Absent,
    Capture,
    Other,
}

// Only the three pinned termination notifications, not a general event schema.
// IDs are compared in full but never copied into diagnostics.
#[derive(Debug, PartialEq, Eq)]
enum ExitSource {
    NativeRootTermination,
    InspectorDetached {
        session: EventSession,
        reason: String,
    },
    InspectorTargetCrashed {
        session: EventSession,
    },
    TargetTargetCrashed {
        session: EventSession,
        target_matches: bool,
        status: String,
        error_code: i32,
    },
}
impl ExitSource {
    fn terminates_capture(&self, phase: Phase) -> bool {
        match self {
            Self::NativeRootTermination => true,
            Self::InspectorDetached { session, .. } => {
                *session == EventSession::Capture && phase != Phase::Shutdown
            }
            Self::InspectorTargetCrashed { session } => *session == EventSession::Capture,
            Self::TargetTargetCrashed {
                session,
                target_matches,
                ..
            } => *target_matches && *session != EventSession::Other,
        }
    }
}
impl std::fmt::Display for ExitSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NativeRootTermination => f.write_str("native root termination"),
            Self::InspectorDetached { session, reason } => {
                write!(
                    f,
                    "Inspector.detached session={session:?} reason={reason:?}"
                )
            }
            Self::InspectorTargetCrashed { session } => {
                write!(f, "Inspector.targetCrashed session={session:?}")
            }
            Self::TargetTargetCrashed {
                session,
                target_matches,
                status,
                error_code,
            } => {
                write!(
                    f,
                    "Target.targetCrashed session={session:?} target_matches={target_matches} status={status:?} error_code={error_code}"
                )
            }
        }
    }
}
fn exit_event(
    message: &Value,
    session: Option<&str>,
    target: Option<&str>,
) -> Result<Option<ExitSource>, Error> {
    let method = string(message, "method")?;
    if !matches!(
        method,
        "Inspector.detached" | "Inspector.targetCrashed" | "Target.targetCrashed"
    ) {
        return Ok(None);
    }
    let params = message
        .get("params")
        .filter(|p| p.is_object())
        .ok_or_else(|| malformed("event without params"))?;
    let event_session = match message.get("sessionId") {
        None => EventSession::Absent,
        Some(Value::String(id)) if Some(id.as_str()) == session => EventSession::Capture,
        Some(Value::String(_)) => EventSession::Other,
        Some(_) => return Err(malformed("invalid event sessionId")),
    };
    let scalar = |text: &str| {
        let mut chars = text.chars();
        let mut bounded: String = chars.by_ref().take(128).collect();
        if chars.next().is_some() {
            bounded.push('…');
        }
        bounded
    };
    // Validate before routing: malformed evidence is never dismissed as foreign.
    Ok(Some(match method {
        "Inspector.detached" => ExitSource::InspectorDetached {
            session: event_session,
            reason: scalar(string(params, "reason")?),
        },
        "Inspector.targetCrashed" => ExitSource::InspectorTargetCrashed {
            session: event_session,
        },
        "Target.targetCrashed" => ExitSource::TargetTargetCrashed {
            session: event_session,
            target_matches: Some(string(params, "targetId")?) == target,
            status: scalar(string(params, "status")?),
            error_code: params
                .get("errorCode")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| malformed("invalid target crash errorCode"))?,
        },
        _ => unreachable!(),
    }))
}

// A renderer-exit callback emits Inspector.targetCrashed before the browser
// Target.targetCrashed carrying status/code. Inspect only bytes already read
// when the first event failed capture: no read, dispatch, wait or state change.
// This is supplementary diagnostic evidence, not validated stream completion.
fn buffered_target_crash(input: &[u8], session: Option<&str>, target: Option<&str>) -> String {
    let bounded = &input[..input.len().min(4096)];
    let mut frames = bounded.split(|b| *b == 0);
    for _ in 0..4 {
        let Some(frame) = frames.next() else { break };
        // The final split item has no delimiter and may be truncated.
        if frames.clone().next().is_none() {
            break;
        }
        let Ok(message) = serde_json::from_slice::<Value>(frame) else {
            return "malformed buffered JSON (not dispatched)".into();
        };
        if message.get("id").is_some()
            || message.get("method").and_then(Value::as_str) != Some("Target.targetCrashed")
        {
            continue;
        }
        match exit_event(&message, session, target) {
            Ok(Some(event)) if event.terminates_capture(Phase::Startup) => {
                return format!("{event} (not dispatched)");
            }
            Err(_) => return "malformed buffered Target.targetCrashed (not dispatched)".into(),
            _ => {}
        }
    }
    "unavailable within first 4 complete buffered frames / 4096 bytes".into()
}
// Bounded failure-only progress, never protocol contents or successful reports.
#[derive(Debug)]
struct CaptureProgress {
    operation: &'static str,
    phase: Option<Phase>,
    command_id: Option<u64>,
    activity: &'static str,
    completed: std::collections::VecDeque<&'static str>,
    shutdown: Option<ShutdownProgress>,
    exit_source: Option<ExitSource>,
}
impl CaptureProgress {
    fn complete(&mut self) {
        if self.completed.len() == 16 {
            self.completed.pop_front();
        }
        self.completed.push_back(self.operation);
    }
}
#[derive(Debug)]
struct ShutdownProgress {
    started: Instant,
    capture_deadline: Instant,
    sent: bool,
    acknowledged: bool,
    acknowledgement_elapsed: Option<std::time::Duration>,
    validated_frames: usize,
    complete_frames_read: usize,
    command_bytes_written: usize,
    eof: bool,
}
pub(super) struct Cdp<'a> {
    pub child: &'a mut OwnedChromium,
    cancel: &'a Cancellation,
    input: Vec<u8>,
    next_id: u64,
    pending: BTreeMap<u64, (String, Option<String>)>,
    pub session: Option<String>,
    pub target: Option<String>,
    pub navigation: Option<Navigation>,
    capture_progress: CaptureProgress,
}

impl<'a> Cdp<'a> {
    pub fn new(child: &'a mut OwnedChromium, cancel: &'a Cancellation) -> Self {
        Self {
            child,
            cancel,
            input: Vec::new(),
            next_id: 1,
            pending: BTreeMap::new(),
            session: None,
            target: None,
            navigation: None,
            capture_progress: CaptureProgress {
                operation: "capture not started",
                phase: None,
                command_id: None,
                activity: "idle",
                completed: Default::default(),
                shutdown: None,
                exit_source: None,
            },
        }
    }
    #[cfg(test)]
    pub fn resource_regression(&mut self, scenario: &str) -> Result<(), Error> {
        self.session = Some("fixture-session".into());
        self.navigation = Some(tests::verified_navigation());
        // These IDs represent earlier protocol calls in this isolated scenario.
        self.next_id = 10;
        let deadline = Instant::now()
            + std::time::Duration::from_millis(if scenario == "resource-shutdown-capture-budget" {
                100
            } else if scenario.starts_with("resource-shutdown-delayed-") {
                2000
            } else {
                300
            });
        if matches!(
            scenario,
            "resource-shutdown-root-invalidated"
                | "resource-shutdown-root-clean"
                | "resource-shutdown-buffered-deadline"
        ) {
            self.call("Page.getLayoutMetrics", json!({}), deadline, Phase::Capture)?;
            if scenario != "resource-shutdown-root-clean" {
                // Read the helper's actual event without dispatching it, then
                // retain its complete frame as pending input. This avoids relying
                // on pipe read sizes to arrange the shutdown interleaving.
                let event = self.read(deadline, Phase::Capture)?;
                assert_eq!(event["sessionId"], "fixture-session");
                assert_eq!(event["method"], "DOM.documentUpdated");
                let mut frame = serde_json::to_vec(&event).unwrap();
                frame.push(0);
                self.input.splice(..0, frame);
            }
            if scenario == "resource-shutdown-buffered-deadline" {
                self.child.test_expire_next_discovery();
            } else {
                self.child.kill_root().unwrap();
                while self.child.failure_observation().0 != super::process::RootObservation::Exited
                {
                    self.cancel.check(deadline, Phase::Capture)?;
                    std::thread::yield_now();
                }
            }
        }
        if scenario == "resource-shutdown-cancel-after-ack" {
            super::tests::arm_shutdown_ack_cancellation(1);
        }
        if scenario.starts_with("resource-shutdown-") {
            let result = self.close_browser(deadline);
            if scenario == "resource-shutdown-cancel-after-ack" {
                super::tests::assert_shutdown_cancellation_consumed();
            }
            assert_eq!(
                self.capture_progress.operation, "Browser.close response-stream drain",
                "shutdown diagnostics must not retain an earlier capture operation"
            );
            if scenario == "resource-shutdown-buffered-deadline" {
                super::process::fault::assert_consumed();
                assert!(matches!(
                    self.child.check_activity,
                    super::process::CheckActivity::OwnershipDiscovery
                ));
                assert!(self.input.contains(&0), "buffered event was not retained");
                assert!(self.navigation.as_ref().unwrap().resources_complete());
            }
            if matches!(
                scenario,
                "resource-shutdown-no-response"
                    | "resource-shutdown-ack-only"
                    | "resource-shutdown-capture-budget"
                    | "resource-shutdown-partial-timeout"
            ) {
                let diagnostic = self.failure_diagnostic();
                eprintln!("AG7 controlled {scenario}: {diagnostic}");
                assert!(diagnostic.contains("sent=true"), "{diagnostic}");
                assert!(diagnostic.contains("capture_expired=true"), "{diagnostic}");
                assert!(diagnostic.contains("eof=false"), "{diagnostic}");
                assert!(
                    diagnostic.contains("root_at_failure=NoExitObserved"),
                    "{diagnostic}"
                );
                assert_eq!(
                    self.capture_progress
                        .shutdown
                        .as_ref()
                        .unwrap()
                        .acknowledged,
                    scenario != "resource-shutdown-no-response"
                );
                assert!(!diagnostic.contains("PRIVATE_PAYLOAD"));
                assert!(diagnostic.len() < 4096);
            }
            if scenario == "resource-shutdown-partial-timeout" {
                assert!(
                    !self.input.is_empty(),
                    "helper never supplied partial input"
                );
                assert!(!self.input.contains(&0));
                assert!(self.navigation.as_ref().unwrap().resources_complete());
            }
            if scenario.starts_with("resource-shutdown-delayed-") {
                eprintln!(
                    "AG7 delayed completion {scenario}: result={result:?}; {}",
                    self.failure_diagnostic()
                );
            }
            result
        } else {
            self.call("Page.getLayoutMetrics", json!({}), deadline, Phase::Capture)?;
            self.wait_resource_completion(deadline)
        }
    }
    pub fn capture_operation(&mut self, operation: &'static str) {
        self.capture_progress.operation = operation;
        self.capture_progress.command_id = None;
        self.capture_progress.activity = "local capture operation";
    }
    #[cfg(test)]
    pub fn startup_exit_event_regression(&mut self, event: Value) -> Result<Value, Error> {
        self.session = Some("fixture-session".into());
        self.target = Some("fixture-target".into());
        // Control the read boundary explicitly: one event followed by the
        // pending Page.enable response. No timing or pipe-coalescing assumption.
        for frame in [
            event,
            json!({"id":self.next_id,"sessionId":"fixture-session","result":{}}),
        ] {
            self.input.extend(serde_json::to_vec(&frame).unwrap());
            self.input.push(0);
        }
        self.call(
            "Page.enable",
            json!({}),
            Instant::now() + std::time::Duration::from_secs(2),
            Phase::Startup,
        )
    }
    #[cfg(test)]
    pub fn startup_crash_diagnostic_regression(&mut self, scenario: &str) {
        let enqueue = |cdp: &mut Self, frame: Value| {
            cdp.input.extend(serde_json::to_vec(&frame).unwrap());
            cdp.input.push(0);
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        for method in [
            "Browser.getVersion",
            "Target.createTarget",
            "Target.setDiscoverTargets",
            "Target.attachToTarget",
        ] {
            enqueue(self, json!({"id":self.next_id,"result":{}}));
            self.call(method, json!({}), deadline, Phase::Startup)
                .unwrap();
        }
        self.session = Some("fixture-session".into());
        self.target = Some("fixture-target".into());
        if scenario == "after-page-response" {
            enqueue(
                self,
                json!({"id":self.next_id,"sessionId":"fixture-session","result":{}}),
            );
            self.call("Page.enable", json!({}), deadline, Phase::Startup)
                .unwrap();
        }
        if scenario != "normal" {
            enqueue(
                self,
                json!({"method":"Inspector.targetCrashed","sessionId":if scenario == "foreign" { "other-session" } else { "fixture-session" },"params":{}}),
            );
        }
        if matches!(scenario, "pending-page-response" | "after-page-response") {
            enqueue(
                self,
                json!({"method":"Target.targetCrashed","params":{"targetId":"fixture-target","status":"crashed","errorCode":17}}),
            );
        }
        enqueue(
            self,
            json!({"id":self.next_id,"sessionId":"fixture-session","result":{}}),
        );
        // Inspector.enable can report an already non-live frame synchronously.
        // It is only used in this controlled model, never added to real startup.
        let method = match scenario {
            "non-live-at-inspector-enable" => "Inspector.enable",
            "after-page-response" => "Page.setLifecycleEventsEnabled",
            _ => "Page.enable",
        };
        let result = self.call(method, json!({}), deadline, Phase::Startup);
        let failed = !matches!(scenario, "normal" | "foreign");
        assert_eq!(matches!(result, Err(Error::BrowserExited)), failed);
        if !failed {
            assert_eq!(result.unwrap(), json!({}));
        }
        let before = (self.input.clone(), self.pending.clone());
        let diagnostic = self.failure_diagnostic();
        assert_eq!((self.input.clone(), self.pending.clone()), before);
        assert_eq!(self.pending.len(), usize::from(failed));
        assert!(diagnostic.len() < 4096);
        assert!(
            self.capture_progress
                .completed
                .contains(&"Target.attachToTarget")
        );
        assert_eq!(
            self.capture_progress.completed.contains(&"Page.enable"),
            matches!(scenario, "normal" | "foreign" | "after-page-response")
        );
        if matches!(scenario, "pending-page-response" | "after-page-response") {
            assert!(
                diagnostic.contains("status=\"crashed\" error_code=17 (not dispatched)"),
                "{diagnostic}"
            );
        } else if failed {
            assert!(
                diagnostic.contains("buffered_target_crash=unavailable"),
                "{diagnostic}"
            );
        } else {
            assert!(
                diagnostic.contains("exit_source=none; buffered_target_crash=not inspected"),
                "{diagnostic}"
            );
        }
        eprintln!("controlled startup lifecycle {scenario}: {diagnostic}");
    }
    pub fn capture_operation_completed(&mut self) {
        self.capture_progress.complete();
    }
    pub fn failure_diagnostic(&self) -> String {
        // One non-reaping, nonblocking root observation and a zero-time pipe
        // poll, before cleanup. No discovery, retry, signal or deadline reset.
        let (root, pipe) = self.child.failure_observation();
        let now = Instant::now();
        let shutdown = self.capture_progress.shutdown.as_ref().map(|s| {
            format!("elapsed={:?} capture_remaining_at_start={:?} capture_expired={} sent={} acknowledged={} validated_frames={} complete_frames_read={} command_bytes_written={} acknowledgement_elapsed={:?} eof={}",
                now.saturating_duration_since(s.started), s.capture_deadline.saturating_duration_since(s.started),
                now >= s.capture_deadline, s.sent, s.acknowledged, s.validated_frames, s.complete_frames_read, s.command_bytes_written, s.acknowledgement_elapsed, s.eof)
        });
        let buffered_crash = if matches!(
            self.capture_progress.exit_source,
            Some(ExitSource::InspectorTargetCrashed { .. })
        ) {
            buffered_target_crash(&self.input, self.session.as_deref(), self.target.as_deref())
        } else {
            "not inspected".into()
        };
        format!(
            "phase={:?}; operation: {:?}; command_id: {:?}; activity: {:?}; completed: {:?}; shutdown={shutdown:?}; buffered_bytes={} complete_frames={} pending_commands={}; exit_source={}; buffered_target_crash={buffered_crash}; native_check={:?}; root_at_failure={root:?}; response_pipe_at_failure={pipe:?}; ownership={}",
            self.capture_progress.phase,
            self.capture_progress.operation,
            self.capture_progress.command_id,
            self.capture_progress.activity,
            self.capture_progress.completed,
            self.input.len(),
            self.input.iter().filter(|b| **b == 0).count(),
            self.pending.len(),
            self.capture_progress
                .exit_source
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".into()),
            self.child.check_activity,
            self.child.ownership_diagnostic()
        )
    }
    fn send(
        &mut self,
        method: &str,
        params: Value,
        deadline: Instant,
        phase: Phase,
    ) -> Result<u64, Error> {
        self.capture_progress.phase = Some(phase);
        let id = self.next_id;
        self.next_id += 1;
        let mut message = json!({"id":id,"method":method,"params":params});
        if let Some(session) = &self.session {
            message["sessionId"] = json!(session);
        }
        let mut bytes = serde_json::to_vec(&message)
            .map_err(|e| Error::Protocol(ProtocolError::Malformed(e.to_string())))?;
        if bytes.len() >= MAX_FRAME {
            return Err(Error::Protocol(ProtocolError::Oversized));
        }
        bytes.push(0);
        let mut written = 0;
        while written < bytes.len() {
            if let Err(error) = self.child.check(deadline, phase, self.cancel) {
                if matches!(error, Error::BrowserExited) {
                    self.capture_progress.exit_source = Some(ExitSource::NativeRootTermination);
                }
                return Err(error);
            }
            self.capture_progress.activity = "sending";
            let pipe = self
                .child
                .input
                .as_mut()
                .ok_or(Error::Protocol(ProtocolError::Eof))?;
            match pipe.write(&bytes[written..]) {
                Ok(0) => return Err(Error::Protocol(ProtocolError::Eof)),
                Ok(n) => {
                    written += n;
                    if let Some(shutdown) = &mut self.capture_progress.shutdown {
                        shutdown.command_bytes_written += n;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.capture_progress.activity = "waiting for writable command pipe";
                    poll_fd(pipe.as_raw_fd(), libc::POLLOUT, deadline)?
                }
                Err(e) => return Err(Error::io("CDP write", e)),
            }
        }
        self.pending
            .insert(id, (method.into(), self.session.clone()));
        Ok(id)
    }
    pub fn call(
        &mut self,
        method: &'static str,
        params: Value,
        deadline: Instant,
        phase: Phase,
    ) -> Result<Value, Error> {
        self.capture_operation(method);
        self.capture_progress.command_id = Some(self.next_id);
        self.capture_progress.activity = "preparing command";
        let id = self.send(method, params, deadline, phase)?;
        self.capture_progress.activity = "waiting for response";
        loop {
            if let Some((response_id, value)) = self.step(deadline, phase)?
                && response_id == id
            {
                if matches!(phase, Phase::Startup | Phase::Capture) {
                    self.capture_operation_completed();
                }
                return Ok(value);
            }
        }
    }
    pub fn wait_ready(&mut self, deadline: Instant) -> Result<(), Error> {
        while !self.navigation.as_ref().is_some_and(Navigation::ready) {
            self.step(deadline, Phase::Navigation)?;
        }
        Ok(())
    }
    pub fn wait_resource_completion(&mut self, deadline: Instant) -> Result<(), Error> {
        while !self.navigation.as_ref().unwrap().resources_complete() {
            self.step(deadline, Phase::Capture)?;
        }
        self.cancel.check(deadline, Phase::Capture)
    }
    pub fn close_browser(&mut self, capture_deadline: Instant) -> Result<(), Error> {
        self.capture_operation("Browser.close response-stream drain");
        self.navigation.as_ref().unwrap().finish_resources()?;
        let started = Instant::now();
        // EOF validation is part of capture, not a best-effort close request.
        // Share its existing absolute deadline: acknowledgements/events cannot
        // renew it, and cleanup retains its separate finite allowance.
        let deadline = capture_deadline;
        self.capture_progress.command_id = Some(self.next_id);
        self.capture_progress.shutdown = Some(ShutdownProgress {
            started,
            capture_deadline,
            sent: false,
            acknowledged: false,
            acknowledgement_elapsed: None,
            validated_frames: 0,
            complete_frames_read: 0,
            command_bytes_written: 0,
            eof: false,
        });
        // Browser.close is browser-scoped, but fixture events must still be
        // checked while waiting for it. Only the outgoing command loses its session.
        let session = self.session.take();
        let sent = self.send("Browser.close", json!({}), deadline, Phase::Shutdown);
        self.session = session;
        self.capture_progress.shutdown.as_mut().unwrap().sent = sent.is_ok();
        match sent {
            Ok(_) | Err(Error::BrowserExited | Error::Protocol(ProtocolError::Eof)) => {}
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(error) => return Err(error),
        }
        // A tolerated native exit while sending close is not the source of a
        // later protocol failure. Continue classifying the incoming evidence.
        self.capture_progress.exit_source = None;
        // Command delivery and root liveness do not establish that incoming
        // evidence was inspected. Only response-stream EOF after complete frame
        // dispatch is a completion boundary; timeouts and truncated frames fail.
        loop {
            match self.step(deadline, Phase::Shutdown) {
                Ok(_) => {}
                Err(Error::Protocol(ProtocolError::Eof)) => {
                    self.capture_progress.shutdown.as_mut().unwrap().eof = true;
                    self.navigation.as_ref().unwrap().finish_resources()?;
                    self.cancel.check(deadline, Phase::Shutdown)?;
                    return self.cancel.check(capture_deadline, Phase::Capture);
                }
                Err(error) => return Err(error),
            }
        }
    }
    fn read(&mut self, deadline: Instant, phase: Phase) -> Result<Value, Error> {
        loop {
            match self.child.check(deadline, phase, self.cancel) {
                // The root may exit with final events still in the pipe. This
                // grants no cleanup authority: retain and validate those events.
                Err(Error::BrowserExited) if phase == Phase::Shutdown => {
                    self.cancel.check(deadline, phase)?;
                }
                Err(Error::BrowserExited) => {
                    self.capture_progress.exit_source = Some(ExitSource::NativeRootTermination);
                    return Err(Error::BrowserExited);
                }
                result => result?,
            }
            if let Some(end) = self.input.iter().position(|b| *b == 0) {
                self.capture_progress.activity = "decoding response/event";
                let rest = self.input.split_off(end + 1);
                self.input.pop();
                let result = serde_json::from_slice(&self.input)
                    .map_err(|e| Error::Protocol(ProtocolError::Malformed(e.to_string())));
                self.input = rest;
                return result;
            }
            if self.input.len() >= MAX_FRAME {
                return Err(Error::Protocol(ProtocolError::Oversized));
            }
            let mut bytes = [0; 16384];
            let size = bytes.len().min(MAX_FRAME - self.input.len());
            self.capture_progress.activity = "receiving";
            match self.child.output.read(&mut bytes[..size]) {
                Ok(0) => {
                    return Err(Error::Protocol(if self.input.is_empty() {
                        ProtocolError::Eof
                    } else {
                        ProtocolError::Truncated
                    }));
                }
                Ok(n) => self.input.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.capture_progress.activity = "waiting for response";
                    poll_fd(self.child.output.as_raw_fd(), libc::POLLIN, deadline)?
                }
                Err(e) => return Err(Error::io("CDP read", e)),
            }
        }
    }
    fn step(&mut self, deadline: Instant, phase: Phase) -> Result<Option<(u64, Value)>, Error> {
        let message = self.read(deadline, phase)?;
        if let Some(shutdown) = &mut self.capture_progress.shutdown {
            shutdown.complete_frames_read += 1;
        }
        if let Some(id) = message.get("id") {
            let id = id
                .as_u64()
                .ok_or(Error::Protocol(ProtocolError::UnexpectedResponse))?;
            let (method, session) = self
                .pending
                .remove(&id)
                .ok_or(Error::Protocol(ProtocolError::UnexpectedResponse))?;
            if message.get("sessionId").and_then(Value::as_str) != session.as_deref() {
                return Err(Error::Protocol(ProtocolError::UnexpectedResponse));
            }
            if let Some(error) = message.get("error") {
                return Err(Error::Protocol(ProtocolError::Remote {
                    method,
                    code: error["code"].as_i64().unwrap_or(0),
                    message: error["message"]
                        .as_str()
                        .unwrap_or("missing error message")
                        .into(),
                }));
            }
            let result = message
                .get("result")
                .filter(|r| r.is_object())
                .ok_or_else(|| malformed("response without result"))?
                .clone();
            if let Some(nav) = &mut self.navigation {
                nav.command_completed(id);
            }
            if let Some(shutdown) = &mut self.capture_progress.shutdown {
                shutdown.validated_frames += 1;
                if method == "Browser.close" {
                    shutdown.acknowledged = true;
                    shutdown.acknowledgement_elapsed = Some(shutdown.started.elapsed());
                    #[cfg(test)]
                    super::tests::shutdown_acknowledged(
                        self.cancel,
                        shutdown.started,
                        shutdown.capture_deadline,
                    );
                }
            }
            return Ok(Some((id, result)));
        }
        let method = string(&message, "method")?;
        let params = message
            .get("params")
            .filter(|p| p.is_object())
            .ok_or_else(|| malformed("event without params"))?;
        if let Some(event) = exit_event(&message, self.session.as_deref(), self.target.as_deref())?
            && event.terminates_capture(phase)
        {
            self.capture_progress.exit_source = Some(event);
            return Err(Error::BrowserExited);
        }
        // Valid foreign notifications follow the existing non-resource routing.
        // A validated capture-session detach during shutdown also continues;
        // only fully validated response EOF can complete that phase.
        if message.get("sessionId").and_then(Value::as_str) == self.session.as_deref() {
            if let Some(nav) = &mut self.navigation {
                if method == "Fetch.requestPaused" {
                    let request = string(params, "requestId")?.to_owned();
                    let accepted = nav.accept_fetch(params);
                    if let Err(e) = accepted {
                        // Best-effort abort within the same deadline; failure is
                        // already latched and can never turn into an observation.
                        let _ = self.send(
                            "Fetch.failRequest",
                            json!({"requestId":request,"errorReason":"BlockedByClient"}),
                            deadline,
                            phase,
                        );
                        return Err(e);
                    }
                    let action = accepted.unwrap();
                    let (method, params) = match action {
                        FetchAction::FulfillDocument => (
                            "Fetch.fulfillRequest",
                            json!({
                            "requestId":request,"responseCode":200,
                            "responseHeaders":[{"name":"Content-Type","value":CONTENT_TYPE}],
                            "body":STANDARD.encode(&nav.html)}),
                        ),
                        FetchAction::AbortFavicon => (
                            "Fetch.failRequest",
                            json!({"requestId":request,"errorReason":"BlockedByClient"}),
                        ),
                    };
                    let id = self.send(method, params, deadline, phase)?;
                    self.navigation.as_mut().unwrap().record_command(action, id);
                } else {
                    nav.event(method, params)?;
                }
            }
        } else if self.navigation.is_some()
            && matches!(
                method,
                "Fetch.requestPaused"
                    | "Network.requestWillBeSent"
                    | "Network.responseReceived"
                    | "Network.loadingFailed"
                    | "Network.loadingFinished"
            )
        {
            return Err(malformed("resource event outside fixture session"));
        } else if self.navigation.is_some()
            && method == "Target.targetCreated"
            && params["targetInfo"]["targetId"].as_str() != self.target.as_deref()
        {
            return Err(Error::Navigation(format!(
                "unexpected browser target: {}",
                params["targetInfo"]
            )));
        }
        if let Some(shutdown) = &mut self.capture_progress.shutdown {
            shutdown.validated_frames += 1;
        }
        Ok(None)
    }
}
fn malformed(s: &str) -> Error {
    Error::Protocol(ProtocolError::Malformed(s.into()))
}
pub(super) fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(&format!("missing string {key}")))
}

const FAVICON: &str = "https://borrowser.invalid/favicon.ico";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocumentPolicy {
    Pending,
    NoLinks,
    VerifiedMetadataLinks,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FetchAction {
    FulfillDocument,
    AbortFavicon,
}
#[derive(Default)]
struct FaviconCandidate {
    network_id: Option<String>,
    fetch_id: Option<String>,
    fetch_network_id: Option<String>,
    abort_command_id: Option<u64>,
    abort_acknowledged: bool,
    loading_failed: bool,
}
impl FaviconCandidate {
    fn correlate(&self) -> Result<(), Error> {
        if let (Some(network), Some(fetch)) = (&self.network_id, &self.fetch_network_id)
            && network != fetch
        {
            return Err(Navigation::fail("conflicting favicon request identities"));
        }
        Ok(())
    }
    fn complete(&self) -> bool {
        self.network_id.is_some()
            && self.network_id == self.fetch_network_id
            && self.fetch_id.is_some()
            && self.abort_acknowledged
            && self.loading_failed
    }
    fn matches(&self, id: &str) -> bool {
        self.network_id.as_deref() == Some(id) || self.fetch_network_id.as_deref() == Some(id)
    }
}
fn request_id(p: &Value, name: &str) -> Result<String, Error> {
    let id = string(p, name)?;
    if id.is_empty() || id.len() > 128 {
        return Err(malformed("invalid request identity"));
    }
    Ok(id.into())
}
// CDP NodeId is a signed integer. Zero is valid only for a query's no-match result.
fn node_id(value: &Value, allow_zero: bool) -> Result<i32, Error> {
    value
        .as_i64()
        .and_then(|id| i32::try_from(id).ok())
        .filter(|id| *id > 0 || (allow_zero && *id == 0))
        .ok_or_else(|| malformed("invalid DOM node identity"))
}

pub(super) struct Navigation {
    frame: String,
    html: Vec<u8>,
    request: Option<String>,
    fetch_network: Option<String>,
    fetch_request: Option<String>,
    loader: Option<String>,
    acknowledged: Option<String>,
    committed: Option<String>,
    loads: BTreeSet<String>,
    response: bool,
    document_policy: DocumentPolicy,
    document_node: Option<i32>,
    fulfill_command_id: Option<u64>,
    fulfill_acknowledged: bool,
    favicon: Option<FaviconCandidate>,
}
impl Navigation {
    pub fn new(frame: String, html: &[u8]) -> Self {
        Self {
            frame,
            html: html.to_vec(),
            request: None,
            fetch_network: None,
            fetch_request: None,
            loader: None,
            acknowledged: None,
            committed: None,
            loads: BTreeSet::new(),
            response: false,
            document_policy: DocumentPolicy::Pending,
            document_node: None,
            fulfill_command_id: None,
            fulfill_acknowledged: false,
            favicon: None,
        }
    }
    fn fail(s: &str) -> Error {
        Error::Navigation(s.into())
    }
    fn accept_fetch(&mut self, p: &Value) -> Result<FetchAction, Error> {
        if p["frameId"] != self.frame
            || p["request"]["method"] != "GET"
            || p.get("redirectedRequestId").is_some()
            || p.get("responseStatusCode").is_some()
            || p.get("responseErrorReason").is_some()
        {
            return Err(Self::fail("unexpected or redirected Fetch request"));
        }
        let fetch_id = request_id(p, "requestId")?;
        let network_id = request_id(p, "networkId")?;
        if p["resourceType"] == "Document"
            && p["request"]["url"] == URL
            && self.fetch_network.is_none()
        {
            if self.favicon.as_ref().is_some_and(|candidate| {
                candidate.matches(&network_id) || candidate.fetch_id.as_deref() == Some(&fetch_id)
            }) {
                return Err(Self::fail("document conflicts with favicon interception"));
            }
            self.fetch_network = Some(network_id);
            self.fetch_request = Some(fetch_id);
            return Ok(FetchAction::FulfillDocument);
        }
        if p["resourceType"] != "Other"
            || p["request"]["url"] != FAVICON
            || self.request.as_deref() == Some(&network_id)
            || self.fetch_network.as_deref() == Some(&network_id)
            || self.fetch_request.as_deref() == Some(&fetch_id)
        {
            return Err(Self::fail(
                "unexpected, duplicate, or authored resource request",
            ));
        }
        let candidate = self.favicon.get_or_insert_with(Default::default);
        if candidate.fetch_id.is_some() {
            return Err(Self::fail("duplicate favicon interception"));
        }
        candidate.fetch_id = Some(fetch_id);
        candidate.fetch_network_id = Some(network_id);
        candidate.correlate()?;
        Ok(FetchAction::AbortFavicon)
    }
    fn record_command(&mut self, action: FetchAction, id: u64) {
        match action {
            FetchAction::FulfillDocument => self.fulfill_command_id = Some(id),
            FetchAction::AbortFavicon => self.favicon.as_mut().unwrap().abort_command_id = Some(id),
        }
    }
    fn command_completed(&mut self, id: u64) {
        if self.fulfill_command_id == Some(id) {
            self.fulfill_acknowledged = true;
        }
        if let Some(candidate) = &mut self.favicon
            && candidate.abort_command_id == Some(id)
        {
            candidate.abort_acknowledged = true;
        }
    }
    pub fn inspect_document(&mut self, document: &Value) -> Result<i32, Error> {
        let root = &document["root"];
        if !self.ready()
            || root["nodeType"] != 9
            || root["documentURL"] != URL
            || root["compatibilityMode"] != "NoQuirksMode"
            || self.document_node.is_some()
        {
            return Err(Self::fail("unexpected document identity, URL or mode"));
        }
        let id = node_id(&root["nodeId"], false)?;
        self.document_node = Some(id);
        Ok(id)
    }
    pub fn verify_document_policy(&mut self, query: &Value, tree: &Value) -> Result<(), Error> {
        self.verify_tree(tree)?;
        if self.document_node.is_none() || self.document_policy != DocumentPolicy::Pending {
            return Err(Self::fail("missing or duplicate document policy evidence"));
        }
        if node_id(&query["nodeId"], true)? != 0 {
            return Err(Self::fail(
                "authored link elements are outside the inline-only capture profile",
            ));
        }
        self.document_policy = DocumentPolicy::NoLinks;
        Ok(())
    }
    fn resources_complete(&self) -> bool {
        matches!(
            self.document_policy,
            DocumentPolicy::NoLinks | DocumentPolicy::VerifiedMetadataLinks
        ) && self.fulfill_acknowledged
            && self.favicon.as_ref().is_none_or(FaviconCandidate::complete)
    }
    pub fn verify_metadata_policy(
        &mut self,
        actual: &[crate::metadata_links::LinkFact],
        expected: &[crate::metadata_links::LinkFact],
        tree: &Value,
    ) -> Result<(), Error> {
        self.verify_tree(tree)?;
        if self.document_node.is_none() || self.document_policy != DocumentPolicy::Pending {
            return Err(Self::fail("missing or duplicate document policy evidence"));
        }
        crate::metadata_links::validate(actual).map_err(|e| Self::fail(&e))?;
        if actual != expected || expected.is_empty() {
            return Err(Self::fail("metadata link correspondence mismatch"));
        }
        self.document_policy = DocumentPolicy::VerifiedMetadataLinks;
        Ok(())
    }
    #[cfg(test)]
    pub fn verified_favicon(&self) -> bool {
        self.resources_complete()
            && self
                .favicon
                .as_ref()
                .is_some_and(FaviconCandidate::complete)
    }
    pub fn finish_resources(&self) -> Result<(), Error> {
        if !self.resources_complete() {
            return Err(Self::fail(
                "incomplete document policy or resource interception evidence",
            ));
        }
        Ok(())
    }
    pub fn acknowledge(&mut self, result: &Value) -> Result<(), Error> {
        if result["frameId"] != self.frame
            || result.get("errorText").is_some()
            || result["isDownload"] == true
        {
            return Err(Self::fail("fixture navigation failed"));
        }
        self.acknowledged = Some(string(result, "loaderId")?.into());
        Ok(())
    }
    fn event(&mut self, method: &str, p: &Value) -> Result<(), Error> {
        if method == "Network.requestWillBeSent" && p["type"] != "Document" {
            if p["frameId"] != self.frame
                || p["type"] != "Other"
                || p["request"]["url"] != FAVICON
                || p["request"]["method"] != "GET"
                || p["initiator"]["type"] != "other"
                || p.get("redirectResponse").is_some()
            {
                return Err(Self::fail("unexpected or authored resource request"));
            }
            let id = request_id(p, "requestId")?;
            if self.request.as_deref() == Some(&id) || self.fetch_network.as_deref() == Some(&id) {
                return Err(Self::fail("favicon conflicts with document request"));
            }
            let candidate = self.favicon.get_or_insert_with(Default::default);
            if candidate.network_id.is_some() {
                return Err(Self::fail("duplicate favicon request"));
            }
            candidate.network_id = Some(id);
            return candidate.correlate();
        }
        if matches!(
            method,
            "Network.loadingFailed" | "Network.loadingFinished" | "Network.responseReceived"
        ) {
            let id = request_id(p, "requestId")?;
            if let Some(candidate) = &mut self.favicon
                && candidate.matches(&id)
            {
                if method != "Network.loadingFailed"
                    || candidate.abort_command_id.is_none()
                    || candidate.loading_failed
                    || p["type"] != "Other"
                    || p["errorText"] != "net::ERR_BLOCKED_BY_CLIENT.Inspector"
                {
                    #[cfg(test)]
                    eprintln!(
                        "AG2 favicon terminal rejected: method={} type={} error={} abort_sent={} duplicate={}",
                        method.chars().take(64).collect::<String>(),
                        p["type"]
                            .as_str()
                            .unwrap_or("<invalid>")
                            .chars()
                            .take(64)
                            .collect::<String>(),
                        p["errorText"]
                            .as_str()
                            .unwrap_or("<invalid>")
                            .chars()
                            .take(128)
                            .collect::<String>(),
                        candidate.abort_command_id.is_some(),
                        candidate.loading_failed
                    );
                    return Err(Self::fail(
                        "favicon was not uniquely aborted before network delivery",
                    ));
                }
                candidate.loading_failed = true;
                return Ok(());
            }
            if self.request.as_deref() != Some(&id) {
                return Err(Self::fail("unknown resource completion identity"));
            }
        }
        match method {
            "DOM.documentUpdated" if self.document_node.is_some() => {
                // Do not permit a subsequent scan to replace invalidated evidence.
                return Err(Self::fail("document policy evidence invalidated"));
            }
            "Page.frameAttached"
            | "Page.navigatedWithinDocument"
            | "Page.frameDetached"
            | "Page.frameScheduledNavigation"
            | "Page.frameRequestedNavigation" => {
                return Err(Self::fail("unexpected frame or same-document navigation"));
            }
            "Network.requestWillBeSent" => {
                if p["frameId"] != self.frame
                    || p["type"] != "Document"
                    || p["request"]["url"] != URL
                    || p["request"]["method"] != "GET"
                    || self.request.is_some()
                    || p.get("redirectResponse").is_some()
                {
                    return Err(Error::Navigation(format!(
                        "unexpected navigation or external resource: {} {}",
                        p["type"], p["request"]["url"]
                    )));
                }
                let id = request_id(p, "requestId")?;
                if self
                    .favicon
                    .as_ref()
                    .is_some_and(|candidate| candidate.matches(&id))
                {
                    return Err(Self::fail("document conflicts with favicon request"));
                }
                self.request = Some(id);
                self.loader = Some(string(p, "loaderId")?.into());
            }
            "Network.responseReceived" => {
                if Some(string(p, "requestId")?) != self.request.as_deref()
                    || p["type"] != "Document"
                    || p["response"]["url"] != URL
                    || p["response"]["status"] != 200
                    || p["response"]["mimeType"] != "text/html"
                    || self.response
                {
                    return Err(Self::fail("unexpected fixture response"));
                }
                self.response = true;
            }
            "Network.loadingFailed" => return Err(Self::fail("fixture loading failed")),
            "Page.frameNavigated" => {
                let frame = &p["frame"];
                if frame["id"] != self.frame
                    || frame["url"] != URL
                    || frame["securityOrigin"] != "https://borrowser.invalid"
                    || self.committed.is_some()
                {
                    return Err(Self::fail("unexpected committed document"));
                }
                self.committed = Some(string(frame, "loaderId")?.into());
            }
            "Page.lifecycleEvent" if p["frameId"] == self.frame && p["name"] == "load" => {
                if self.loads.len() >= 16 {
                    return Err(Self::fail("too many document lifecycles"));
                }
                self.loads.insert(string(p, "loaderId")?.into());
            }
            _ => {}
        }
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.response
            && self.request.is_some()
            && self.request == self.fetch_network
            && self.acknowledged.is_some()
            && self.acknowledged == self.loader
            && self.acknowledged == self.committed
            && self
                .loads
                .contains(self.acknowledged.as_deref().unwrap_or(""))
    }
    pub fn verify_tree(&self, tree: &Value) -> Result<(), Error> {
        let frame = &tree["frameTree"]["frame"];
        if !self.ready()
            || frame["id"] != self.frame
            || frame["url"] != URL
            || frame["loaderId"].as_str() != self.acknowledged.as_deref()
            || tree["frameTree"]
                .get("childFrames")
                .is_some_and(|children| {
                    children
                        .as_array()
                        .is_none_or(|children| !children.is_empty())
                })
        {
            return Err(Self::fail("document changed during capture"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffered_target_crash_diagnostic_is_bounded_and_scoped() {
        let event = json!({"method":"Target.targetCrashed","params":{"targetId":"capture-target","status":"failed to launch","errorCode":13},"private":"do not report"});
        let encode = |event: &Value| {
            let mut bytes = serde_json::to_vec(event).unwrap();
            bytes.push(0);
            bytes
        };
        let inspect = |bytes: &[u8]| {
            buffered_target_crash(bytes, Some("capture-session"), Some("capture-target"))
        };
        let bytes = encode(&event);
        assert_eq!(
            inspect(&bytes),
            "Target.targetCrashed session=Absent target_matches=true status=\"failed to launch\" error_code=13 (not dispatched)"
        );
        assert!(!inspect(&bytes).contains("private"));
        for foreign in [
            json!({"sessionId":"foreign","method":"Target.targetCrashed","params":event["params"]}),
            json!({"method":"Target.targetCrashed","params":{"targetId":"foreign","status":"crashed","errorCode":1}}),
            json!({"id":5,"result":{},"method":"Target.targetCrashed","params":event["params"]}),
        ] {
            assert!(inspect(&encode(&foreign)).starts_with("unavailable"));
            let mut frames = encode(&foreign);
            frames.extend(&bytes);
            assert!(inspect(&frames).contains("failed to launch"));
        }
        assert!(inspect(&bytes[..bytes.len() - 1]).starts_with("unavailable"));
        assert_eq!(
            inspect(b"{invalid\0"),
            "malformed buffered JSON (not dispatched)"
        );
        assert_eq!(
            inspect(&encode(
                &json!({"method":"Target.targetCrashed","params":{}})
            )),
            "malformed buffered Target.targetCrashed (not dispatched)"
        );
        let mut four = encode(&json!({"id":5,"result":{}})).repeat(4);
        four.extend(&bytes);
        assert!(inspect(&four).starts_with("unavailable"));
        let mut oversized = encode(&json!({"private":"x".repeat(4096)}));
        oversized.extend(&bytes);
        assert!(inspect(&oversized).starts_with("unavailable"));
        let mut long = event;
        long["params"]["status"] = json!("\n".repeat(1000));
        let diagnostic = inspect(&encode(&long));
        assert!(diagnostic.len() < 1024);
        assert!(!diagnostic.contains('\n'));
    }
    fn termination(method: &str, params: Value, session: Option<&str>) -> Value {
        let mut event = json!({"method":method,"params":params});
        if let Some(session) = session {
            event["sessionId"] = json!(session);
        }
        event
    }
    fn classify(event: &Value) -> Result<Option<ExitSource>, Error> {
        exit_event(event, Some("capture-session"), Some("capture-target"))
    }
    fn assert_malformed(event: &Value) {
        assert!(matches!(
            classify(event),
            Err(Error::Protocol(ProtocolError::Malformed(_)))
        ));
    }
    #[test]
    fn termination_event_detach_contract_and_shutdown() {
        let event = termination(
            "Inspector.detached",
            json!({"reason":"target_closed"}),
            Some("capture-session"),
        );
        let source = classify(&event).unwrap().unwrap();
        assert!(source.terminates_capture(Phase::Startup));
        assert!(!source.terminates_capture(Phase::Shutdown));
        assert_eq!(
            source.to_string(),
            "Inspector.detached session=Capture reason=\"target_closed\""
        );
        for session in [None, Some("other-session")] {
            let mut other = event.clone();
            other.as_object_mut().unwrap().remove("sessionId");
            if let Some(session) = session {
                other["sessionId"] = json!(session);
            }
            assert!(
                !classify(&other)
                    .unwrap()
                    .unwrap()
                    .terminates_capture(Phase::Startup)
            );
            other["params"] = json!({});
            assert_malformed(&other); // Validate even foreign notifications.
        }
        for params in [
            json!({}),
            json!({"reason":null}),
            json!({"reason":1}),
            json!({"reason":[]}),
        ] {
            assert_malformed(&termination(
                "Inspector.detached",
                params,
                Some("capture-session"),
            ));
        }
    }
    #[test]
    fn termination_event_inspector_crash_contract() {
        let event = termination(
            "Inspector.targetCrashed",
            json!({}),
            Some("capture-session"),
        );
        let source = classify(&event).unwrap().unwrap();
        assert!(source.terminates_capture(Phase::Startup));
        assert!(source.terminates_capture(Phase::Shutdown));
        assert_eq!(
            source.to_string(),
            "Inspector.targetCrashed session=Capture"
        );
        for session in [None, Some("other-session")] {
            let other = termination("Inspector.targetCrashed", json!({}), session);
            assert!(
                !classify(&other)
                    .unwrap()
                    .unwrap()
                    .terminates_capture(Phase::Startup)
            );
        }
        // This event has no parameter fields. The envelope still requires an object.
        for params in [Value::Null, json!(1), json!([]), json!("bad")] {
            assert_malformed(&termination(
                "Inspector.targetCrashed",
                params,
                Some("capture-session"),
            ));
        }
        let mut missing = event;
        missing.as_object_mut().unwrap().remove("params");
        assert_malformed(&missing);
    }
    #[test]
    fn termination_event_target_crash_contract() {
        let params = json!({"targetId":"capture-target","status":"crashed","errorCode":1});
        let event = termination("Target.targetCrashed", params.clone(), None);
        let source = classify(&event).unwrap().unwrap();
        assert!(source.terminates_capture(Phase::Startup));
        assert!(source.terminates_capture(Phase::Shutdown));
        assert_eq!(
            source.to_string(),
            "Target.targetCrashed session=Absent target_matches=true status=\"crashed\" error_code=1"
        );
        for field in ["targetId", "status", "errorCode"] {
            let mut missing = event.clone();
            missing["params"].as_object_mut().unwrap().remove(field);
            assert_malformed(&missing);
            for wrong in [Value::Null, json!(false), json!([]), json!({})] {
                let mut invalid = event.clone();
                invalid["params"][field] = wrong;
                assert_malformed(&invalid);
            }
        }
        for (field, wrong) in [
            ("targetId", json!(1)),
            ("status", json!(1)),
            ("errorCode", json!("1")),
            ("errorCode", json!(1.5)),
            ("errorCode", json!(2147483648_i64)),
        ] {
            let mut invalid = event.clone();
            invalid["params"][field] = wrong;
            assert_malformed(&invalid);
        }
        for code in [i32::MIN, 0, i32::MAX] {
            let mut valid = event.clone();
            valid["params"]["errorCode"] = json!(code);
            assert!(
                classify(&valid)
                    .unwrap()
                    .unwrap()
                    .terminates_capture(Phase::Startup)
            );
        }
        let mut other = event.clone();
        other["params"]["targetId"] = json!("other-target");
        assert!(
            !classify(&other)
                .unwrap()
                .unwrap()
                .terminates_capture(Phase::Startup)
        );
        other["params"]["status"] = json!(false);
        assert_malformed(&other);
        let foreign = termination("Target.targetCrashed", params, Some("other-session"));
        assert!(
            !classify(&foreign)
                .unwrap()
                .unwrap()
                .terminates_capture(Phase::Startup)
        );
        assert!(
            !exit_event(&event, None, None)
                .unwrap()
                .unwrap()
                .terminates_capture(Phase::Startup)
        );
    }
    #[test]
    fn termination_event_envelope_and_bounded_diagnostics() {
        for (method, params) in [
            ("Inspector.detached", json!({"reason":"closed"})),
            ("Inspector.targetCrashed", json!({})),
            (
                "Target.targetCrashed",
                json!({"targetId":"capture-target","status":"crashed","errorCode":1}),
            ),
        ] {
            for invalid in [Value::Null, json!(7), json!(false), json!([])] {
                let mut event = termination(method, params.clone(), None);
                event["sessionId"] = invalid;
                assert_malformed(&event);
            }
        }
        let event = termination(
            "Inspector.detached",
            json!({"reason":"\n".repeat(10000),"ignored":"PRIVATE_PAYLOAD"}),
            Some("capture-session"),
        );
        let diagnostic = classify(&event).unwrap().unwrap().to_string();
        assert!(diagnostic.len() < 1024);
        assert!(!diagnostic.contains('\n'));
        assert!(diagnostic.ends_with("…\""));
        assert!(!diagnostic.contains("PRIVATE_PAYLOAD"));
        assert!(
            classify(&termination("Page.lifecycleEvent", json!({}), None))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn capture_diagnostic_history_is_bounded() {
        let mut progress = CaptureProgress {
            operation: "DOM.getDocument",
            phase: Some(Phase::Capture),
            command_id: Some(1),
            activity: "receiving",
            completed: Default::default(),
            shutdown: None,
            exit_source: None,
        };
        progress.complete();
        progress.operation = "Page.getLayoutMetrics";
        for _ in 0..100 {
            progress.complete();
        }
        assert_eq!(progress.completed.len(), 16);
        assert!(
            progress
                .completed
                .iter()
                .all(|method| *method == "Page.getLayoutMetrics")
        );
        assert!(format!("{progress:?}").len() < 4096);
    }
    fn request() -> Value {
        json!({"frameId":"f","type":"Document","requestId":"r","loaderId":"l","request":{"url":URL,"method":"GET"}})
    }
    fn fetched() -> Value {
        json!({"frameId":"f","resourceType":"Document","requestId":"fetch","networkId":"r","request":{"url":URL,"method":"GET"}})
    }
    #[test]
    fn readiness_requires_the_same_document_even_when_acknowledgement_arrives_last() {
        let mut nav = Navigation::new("f".into(), b"fixture");
        nav.accept_fetch(&fetched()).unwrap();
        nav.event("Network.requestWillBeSent", &request()).unwrap();
        nav.event("Network.responseReceived",&json!({"requestId":"r","type":"Document","response":{"url":URL,"status":200,"mimeType":"text/html"}})).unwrap();
        nav.event("Page.frameNavigated",&json!({"frame":{"id":"f","url":URL,"securityOrigin":"https://borrowser.invalid","loaderId":"l"}})).unwrap();
        nav.event(
            "Page.lifecycleEvent",
            &json!({"frameId":"f","name":"load","loaderId":"old"}),
        )
        .unwrap();
        assert!(!nav.ready());
        nav.acknowledge(&json!({"frameId":"f","loaderId":"l"}))
            .unwrap();
        assert!(!nav.ready());
        nav.event(
            "Page.lifecycleEvent",
            &json!({"frameId":"f","name":"load","loaderId":"l"}),
        )
        .unwrap();
        assert!(nav.ready());
    }
    #[test]
    fn redirects_duplicate_requests_and_subresources_are_errors() {
        for (field, value) in [
            ("resourceType", json!("Image")),
            ("frameId", json!("other")),
            ("redirectedRequestId", json!("previous")),
        ] {
            let mut p = fetched();
            p[field] = value;
            assert!(Navigation::new("f".into(), b"").accept_fetch(&p).is_err());
        }
        let mut nav = Navigation::new("f".into(), b"");
        nav.accept_fetch(&fetched()).unwrap();
        assert!(nav.accept_fetch(&fetched()).is_err());
        let mut nav = Navigation::new("f".into(), b"");
        nav.event("Network.requestWillBeSent", &request()).unwrap();
        assert!(nav.event("Network.requestWillBeSent", &request()).is_err());
    }
    fn ready_navigation() -> Navigation {
        let mut nav = Navigation::new("f".into(), b"fixture");
        nav.accept_fetch(&fetched()).unwrap();
        nav.record_command(FetchAction::FulfillDocument, 1);
        nav.command_completed(1);
        nav.event("Network.requestWillBeSent", &request()).unwrap();
        nav.event("Network.responseReceived", &json!({"requestId":"r","type":"Document", "response":{"url":URL,"status":200,"mimeType":"text/html"}})).unwrap();
        nav.acknowledge(&json!({"frameId":"f","loaderId":"l"}))
            .unwrap();
        nav.event("Page.frameNavigated", &json!({"frame":{"id":"f","url":URL,"securityOrigin":"https://borrowser.invalid","loaderId":"l"}})).unwrap();
        nav.event(
            "Page.lifecycleEvent",
            &json!({"frameId":"f","loaderId":"l","name":"load"}),
        )
        .unwrap();
        nav
    }
    fn tree() -> Value {
        json!({"frameTree":{"frame":{"id":"f","url":URL,"loaderId":"l"}}})
    }
    fn document() -> Value {
        json!({"root":{"nodeId":1,"nodeType":9,"documentURL":URL,"compatibilityMode":"NoQuirksMode"}})
    }
    pub(super) fn verified_navigation() -> Navigation {
        let mut nav = ready_navigation();
        nav.inspect_document(&document()).unwrap();
        nav.verify_document_policy(&json!({"nodeId":0}), &tree())
            .unwrap();
        nav
    }
    fn icon_network() -> Value {
        json!({"frameId":"f","type":"Other","requestId":"icon", "initiator":{"type":"other"},"request":{"url":FAVICON,"method":"GET"}})
    }
    fn icon_fetch() -> Value {
        json!({"frameId":"f","resourceType":"Other","requestId":"icon-fetch", "networkId":"icon","request":{"url":FAVICON,"method":"GET"}})
    }
    fn icon_failed() -> Value {
        json!({"requestId":"icon","type":"Other","errorText":"net::ERR_BLOCKED_BY_CLIENT.Inspector"})
    }
    fn intercept_icon(nav: &mut Navigation, fetch_first: bool) {
        if !fetch_first {
            nav.event("Network.requestWillBeSent", &icon_network())
                .unwrap();
        }
        assert_eq!(
            nav.accept_fetch(&icon_fetch()).unwrap(),
            FetchAction::AbortFavicon
        );
        nav.record_command(FetchAction::AbortFavicon, 2);
        if fetch_first {
            nav.event("Network.requestWillBeSent", &icon_network())
                .unwrap();
        }
    }
    #[test]
    fn resource_policy_requires_document_and_complete_abort_in_both_event_orders() {
        for fetch_first in [false, true] {
            for terminal_first in [false, true] {
                let mut nav = ready_navigation();
                intercept_icon(&mut nav, fetch_first);
                if terminal_first {
                    nav.event("Network.loadingFailed", &icon_failed()).unwrap();
                }
                assert!(!nav.resources_complete());
                nav.command_completed(2);
                if !terminal_first {
                    nav.event("Network.loadingFailed", &icon_failed()).unwrap();
                }
                // Fully aborted is still not classified without parsed-document evidence.
                assert!(nav.finish_resources().is_err());
                nav.inspect_document(&document()).unwrap();
                nav.verify_document_policy(&json!({"nodeId":0}), &tree())
                    .unwrap();
                nav.finish_resources().unwrap();
            }
        }
    }
    #[test]
    fn resource_policy_rejects_incomplete_evidence_and_wrong_acknowledgment() {
        for missing in [
            "network",
            "fetch",
            "command",
            "ack",
            "terminal",
            "fulfill-ack",
        ] {
            let mut nav = verified_navigation();
            if missing != "network" {
                nav.event("Network.requestWillBeSent", &icon_network())
                    .unwrap();
            }
            if missing != "fetch" {
                nav.accept_fetch(&icon_fetch()).unwrap();
            }
            if missing != "command" && missing != "fetch" {
                nav.record_command(FetchAction::AbortFavicon, 2);
            }
            if missing != "ack" {
                nav.command_completed(2);
            } else {
                nav.command_completed(99);
            }
            if missing != "terminal" && missing != "fetch" && missing != "command" {
                nav.event("Network.loadingFailed", &icon_failed()).unwrap();
            }
            if missing == "fulfill-ack" {
                nav.fulfill_acknowledged = false;
            }
            assert!(nav.finish_resources().is_err(), "missing {missing}");
        }
        verified_navigation().finish_resources().unwrap();
    }
    #[test]
    fn resource_policy_rejects_redirects_duplicates_conflicts_and_successful_transfer() {
        let mut nav = verified_navigation();
        intercept_icon(&mut nav, false);
        assert!(
            nav.event("Network.requestWillBeSent", &icon_network())
                .is_err()
        );
        assert!(nav.accept_fetch(&icon_fetch()).is_err());
        for (key, value) in [
            ("networkId", json!("other")),
            ("redirectedRequestId", json!("old")),
            ("responseStatusCode", json!(200)),
            ("frameId", json!("other")),
            ("resourceType", json!("Image")),
        ] {
            let mut nav = verified_navigation();
            nav.event("Network.requestWillBeSent", &icon_network())
                .unwrap();
            let mut fetch = icon_fetch();
            fetch[key] = value;
            assert!(nav.accept_fetch(&fetch).is_err(), "{key}");
        }
        for (key, value) in [
            ("requestId", json!("other")),
            ("redirectResponse", json!({})),
            ("frameId", json!("other")),
            ("initiator", json!({"type":"parser"})),
        ] {
            let mut nav = verified_navigation();
            nav.accept_fetch(&icon_fetch()).unwrap();
            let mut network = icon_network();
            network[key] = value;
            assert!(
                nav.event("Network.requestWillBeSent", &network).is_err(),
                "{key}"
            );
        }
        for method in ["Network.responseReceived", "Network.loadingFinished"] {
            let mut nav = verified_navigation();
            intercept_icon(&mut nav, false);
            assert!(nav.event(method, &json!({"requestId":"icon"})).is_err());
        }
        let mut nav = verified_navigation();
        intercept_icon(&mut nav, false);
        nav.event("Network.loadingFailed", &icon_failed()).unwrap();
        assert!(nav.event("Network.loadingFailed", &icon_failed()).is_err());
        for field in ["errorText", "type", "requestId"] {
            let mut nav = verified_navigation();
            intercept_icon(&mut nav, false);
            let mut failed = icon_failed();
            failed[field] = json!("wrong");
            assert!(nav.event("Network.loadingFailed", &failed).is_err());
        }
    }
    #[test]
    fn resource_policy_rejects_missing_malformed_and_unbounded_identifiers() {
        for value in [Value::Null, json!(0), json!(""), json!("x".repeat(129))] {
            for field in ["requestId", "networkId"] {
                let mut p = icon_fetch();
                p[field] = value.clone();
                assert!(verified_navigation().accept_fetch(&p).is_err());
            }
        }
    }
    #[test]
    fn dom_policy_requires_valid_document_query_and_unchanged_frame() {
        for value in [
            Value::Null,
            json!(-1),
            json!(0),
            json!(1.5),
            json!(2147483648_u64),
        ] {
            let mut doc = document();
            doc["root"]["nodeId"] = value;
            assert!(ready_navigation().inspect_document(&doc).is_err());
        }
        for (field, value) in [
            ("nodeType", json!(1)),
            ("documentURL", json!("about:blank")),
            ("compatibilityMode", json!("QuirksMode")),
        ] {
            let mut doc = document();
            doc["root"][field] = value;
            assert!(ready_navigation().inspect_document(&doc).is_err());
        }
        for value in [
            Value::Null,
            json!(-1),
            json!(1),
            json!(1.5),
            json!(2147483648_u64),
        ] {
            let mut nav = ready_navigation();
            nav.inspect_document(&document()).unwrap();
            assert!(
                nav.verify_document_policy(&json!({"nodeId":value}), &tree())
                    .is_err()
            );
            assert!(nav.finish_resources().is_err());
        }
        let mut nav = ready_navigation();
        nav.inspect_document(&document()).unwrap();
        let mut wrong = tree();
        wrong["frameTree"]["frame"]["loaderId"] = json!("other");
        assert!(
            nav.verify_document_policy(&json!({"nodeId":0}), &wrong)
                .is_err()
        );
        assert!(
            ready_navigation()
                .verify_document_policy(&json!({"nodeId":0}), &tree())
                .is_err()
        );
    }
    #[test]
    fn late_document_and_resource_violations_remain_errors() {
        for (method, p) in [
            ("DOM.documentUpdated", json!({})),
            (
                "Page.frameNavigated",
                json!({"frame":{"id":"f","url":URL,"securityOrigin":"https://borrowser.invalid","loaderId":"l"}}),
            ),
            (
                "Network.requestWillBeSent",
                json!({"frameId":"f","type":"Image","requestId":"bad","request":{"url":FAVICON,"method":"GET"}}),
            ),
            ("Network.loadingFailed", json!({"requestId":"unknown"})),
        ] {
            let mut nav = verified_navigation();
            nav.finish_resources().unwrap();
            assert!(
                matches!(nav.event(method, &p), Err(Error::Navigation(_))),
                "{method}"
            );
        }
        let mut nav = ready_navigation();
        nav.inspect_document(&document()).unwrap();
        assert!(nav.event("DOM.documentUpdated", &json!({})).is_err());
    }
    #[test]
    fn resource_policy_forbids_document_favicon_identity_reuse_in_either_order() {
        let mut nav = verified_navigation();
        let mut icon = icon_fetch();
        icon["requestId"] = json!("fetch");
        assert!(nav.accept_fetch(&icon).is_err());
        let mut nav = verified_navigation();
        let mut icon = icon_network();
        icon["requestId"] = json!("r");
        assert!(nav.event("Network.requestWillBeSent", &icon).is_err());
        for reuse_fetch in [false, true] {
            let mut nav = Navigation::new("f".into(), b"");
            nav.accept_fetch(&icon_fetch()).unwrap();
            let mut doc = fetched();
            doc[if reuse_fetch {
                "requestId"
            } else {
                "networkId"
            }] = json!(if reuse_fetch { "icon-fetch" } else { "icon" });
            assert!(nav.accept_fetch(&doc).is_err());
        }
        let mut nav = Navigation::new("f".into(), b"");
        nav.event("Network.requestWillBeSent", &icon_network())
            .unwrap();
        let mut doc = request();
        doc["requestId"] = json!("icon");
        assert!(nav.event("Network.requestWillBeSent", &doc).is_err());
    }
}

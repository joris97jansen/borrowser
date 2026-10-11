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
// Qualification-only progress, never protocol contents or successful reports.
#[cfg(test)]
#[derive(Debug)]
struct CaptureProgress {
    operation: &'static str,
    command_id: Option<u64>,
    activity: &'static str,
    completed: std::collections::VecDeque<&'static str>,
}
#[cfg(test)]
impl CaptureProgress {
    fn complete(&mut self) {
        if self.completed.len() == 16 {
            self.completed.pop_front();
        }
        self.completed.push_back(self.operation);
    }
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
    #[cfg(test)]
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
            #[cfg(test)]
            capture_progress: CaptureProgress {
                operation: "capture not started",
                command_id: None,
                activity: "idle",
                completed: Default::default(),
            },
        }
    }
    #[cfg(test)]
    pub fn resource_regression(&mut self, scenario: &str) -> Result<(), Error> {
        self.session = Some("fixture-session".into());
        self.navigation = Some(tests::verified_navigation());
        // These IDs represent earlier protocol calls in this isolated scenario.
        self.next_id = 10;
        let deadline = Instant::now() + std::time::Duration::from_millis(300);
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
        if scenario.starts_with("resource-shutdown-") {
            let result = self.close_browser(deadline);
            if scenario == "resource-shutdown-buffered-deadline" {
                super::process::fault::assert_consumed();
                assert!(matches!(
                    self.child.check_activity,
                    super::process::CheckActivity::OwnershipDiscovery
                ));
                assert!(self.input.contains(&0), "buffered event was not retained");
                assert!(self.navigation.as_ref().unwrap().resources_complete());
            }
            if scenario == "resource-shutdown-partial-timeout" {
                assert!(
                    !self.input.is_empty(),
                    "helper never supplied partial input"
                );
                assert!(!self.input.contains(&0));
                assert!(self.navigation.as_ref().unwrap().resources_complete());
            }
            result
        } else {
            self.call("Page.getLayoutMetrics", json!({}), deadline, Phase::Capture)?;
            self.wait_resource_completion(deadline)
        }
    }
    #[cfg(test)]
    pub fn capture_operation(&mut self, operation: &'static str) {
        self.capture_progress.operation = operation;
        self.capture_progress.command_id = None;
        self.capture_progress.activity = "local capture operation";
    }
    #[cfg(test)]
    pub fn capture_operation_completed(&mut self) {
        self.capture_progress.complete();
    }
    #[cfg(test)]
    pub fn failure_diagnostic(&self) -> String {
        // One non-reaping, nonblocking root observation and a zero-time pipe
        // poll, before cleanup. No discovery, retry, signal or deadline reset.
        let (root, pipe) = self.child.failure_observation();
        format!(
            "{:?}; native_check={:?}; root_at_failure={root:?}; response_pipe_at_failure={pipe:?}",
            self.capture_progress, self.child.check_activity
        )
    }
    fn send(
        &mut self,
        method: &str,
        params: Value,
        deadline: Instant,
        phase: Phase,
    ) -> Result<u64, Error> {
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
            self.child.check(deadline, phase, self.cancel)?;
            #[cfg(test)]
            {
                self.capture_progress.activity = "sending";
            }
            let pipe = self
                .child
                .input
                .as_mut()
                .ok_or(Error::Protocol(ProtocolError::Eof))?;
            match pipe.write(&bytes[written..]) {
                Ok(0) => return Err(Error::Protocol(ProtocolError::Eof)),
                Ok(n) => written += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    #[cfg(test)]
                    {
                        self.capture_progress.activity = "waiting for writable command pipe";
                    }
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
        #[cfg(test)]
        if phase == Phase::Capture {
            self.capture_operation(method);
            self.capture_progress.command_id = Some(self.next_id);
            self.capture_progress.activity = "preparing command";
        }
        let id = self.send(method, params, deadline, phase)?;
        #[cfg(test)]
        {
            self.capture_progress.activity = "waiting for response";
        }
        loop {
            if let Some((response_id, value)) = self.step(deadline, phase)?
                && response_id == id
            {
                #[cfg(test)]
                if phase == Phase::Capture {
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
        self.navigation.as_ref().unwrap().finish_resources()?;
        let deadline = capture_deadline.min(Instant::now() + std::time::Duration::from_millis(250));
        // Browser.close is browser-scoped, but fixture events must still be
        // checked while waiting for it. Only the outgoing command loses its session.
        let session = self.session.take();
        let sent = self.send("Browser.close", json!({}), deadline, Phase::Shutdown);
        self.session = session;
        match sent {
            Ok(_) | Err(Error::BrowserExited | Error::Protocol(ProtocolError::Eof)) => {}
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(error) => return Err(error),
        }
        // Command delivery and root liveness do not establish that incoming
        // evidence was inspected. Only response-stream EOF after complete frame
        // dispatch is a completion boundary; timeouts and truncated frames fail.
        loop {
            match self.step(deadline, Phase::Shutdown) {
                Ok(_) => {}
                Err(Error::Protocol(ProtocolError::Eof)) => {
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
                result => result?,
            }
            if let Some(end) = self.input.iter().position(|b| *b == 0) {
                #[cfg(test)]
                {
                    self.capture_progress.activity = "decoding response/event";
                }
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
            #[cfg(test)]
            {
                self.capture_progress.activity = "receiving";
            }
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
                    #[cfg(test)]
                    {
                        self.capture_progress.activity = "waiting for response";
                    }
                    poll_fd(self.child.output.as_raw_fd(), libc::POLLIN, deadline)?
                }
                Err(e) => return Err(Error::io("CDP read", e)),
            }
        }
    }
    fn step(&mut self, deadline: Instant, phase: Phase) -> Result<Option<(u64, Value)>, Error> {
        let message = self.read(deadline, phase)?;
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
            return Ok(Some((id, result)));
        }
        let method = string(&message, "method")?;
        let params = message
            .get("params")
            .filter(|p| p.is_object())
            .ok_or_else(|| malformed("event without params"))?;
        if method == "Inspector.detached" && phase == Phase::Shutdown {
            return Ok(None);
        }
        if matches!(
            method,
            "Inspector.detached" | "Inspector.targetCrashed" | "Target.targetCrashed"
        ) {
            return Err(Error::BrowserExited);
        }
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
        self.document_policy == DocumentPolicy::NoLinks
            && self.fulfill_acknowledged
            && self.favicon.as_ref().is_none_or(FaviconCandidate::complete)
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
    fn capture_diagnostic_history_is_bounded() {
        let mut progress = CaptureProgress {
            operation: "DOM.getDocument",
            command_id: Some(1),
            activity: "receiving",
            completed: Default::default(),
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

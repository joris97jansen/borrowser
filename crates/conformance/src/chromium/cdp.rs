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
pub(super) struct Cdp<'a> {
    pub child: &'a mut OwnedChromium,
    cancel: &'a Cancellation,
    input: Vec<u8>,
    next_id: u64,
    pending: BTreeMap<u64, (String, Option<String>)>,
    pub session: Option<String>,
    pub target: Option<String>,
    pub navigation: Option<Navigation>,
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
        }
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
        method: &str,
        params: Value,
        deadline: Instant,
        phase: Phase,
    ) -> Result<Value, Error> {
        let id = self.send(method, params, deadline, phase)?;
        loop {
            if let Some((response_id, value)) = self.step(deadline, phase)?
                && response_id == id
            {
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
    fn read(&mut self, deadline: Instant, phase: Phase) -> Result<Value, Error> {
        loop {
            self.child.check(deadline, phase, self.cancel)?;
            if let Some(end) = self.input.iter().position(|b| *b == 0) {
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
            return Ok(Some((id, result)));
        }
        let method = string(&message, "method")?;
        let params = message
            .get("params")
            .filter(|p| p.is_object())
            .ok_or_else(|| malformed("event without params"))?;
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
                    let body = STANDARD.encode(&nav.html);
                    self.send("Fetch.fulfillRequest", json!({"requestId":request,"responseCode":200,
                        "responseHeaders":[{"name":"Content-Type","value":CONTENT_TYPE}], "body":body}), deadline, phase)?;
                } else {
                    nav.event(method, params)?;
                }
            }
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

pub(super) struct Navigation {
    frame: String,
    html: Vec<u8>,
    request: Option<String>,
    fetch_network: Option<String>,
    loader: Option<String>,
    acknowledged: Option<String>,
    committed: Option<String>,
    loads: BTreeSet<String>,
    response: bool,
}
impl Navigation {
    pub fn new(frame: String, html: &[u8]) -> Self {
        Self {
            frame,
            html: html.to_vec(),
            request: None,
            fetch_network: None,
            loader: None,
            acknowledged: None,
            committed: None,
            loads: BTreeSet::new(),
            response: false,
        }
    }
    fn fail(s: &str) -> Error {
        Error::Navigation(s.into())
    }
    pub fn accept_fetch(&mut self, p: &Value) -> Result<(), Error> {
        if p["frameId"] != self.frame
            || p["resourceType"] != "Document"
            || p["request"]["url"] != URL
            || p["request"]["method"] != "GET"
            || self.fetch_network.is_some()
            || p.get("redirectedRequestId").is_some()
        {
            return Err(Self::fail(
                "unexpected, redirected, or duplicate resource request",
            ));
        }
        self.fetch_network = Some(string(p, "networkId")?.into());
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
        match method {
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
                self.request = Some(string(p, "requestId")?.into());
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
}

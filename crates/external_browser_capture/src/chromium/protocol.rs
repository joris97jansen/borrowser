use crate::{CaptureError as E, Result, limits::*};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

pub trait Transport {
    fn send(&mut self, bytes: &[u8], deadline: Instant) -> Result<()>;
    fn receive(&mut self, deadline: Instant) -> Result<Vec<u8>>;
    // Only used after the browser population is stopped. Incomplete framing is
    // an error, never evidence that the event stream was empty.
    fn receive_quiescent(&mut self) -> Result<Option<Vec<u8>>> {
        Err(E::Protocol)
    }
}
pub struct Protocol<T> {
    transport: T,
    next: u64,
    events: VecDeque<Value>,
    event_count: usize,
    total: usize,
    deadline: Instant,
    sequence: u64,
    pending: BTreeMap<u64, (Option<String>, String, Instant)>,
    responses: BTreeMap<u64, Result<Value>>,
}
impl<T: Transport> Protocol<T> {
    #[cfg(test)]
    pub fn new(transport: T) -> Self {
        Self::with_deadline(transport, crate::deadline::AttemptDeadline::new())
    }
    pub fn with_deadline(transport: T, deadline: crate::deadline::AttemptDeadline) -> Self {
        Self {
            transport,
            next: 1,
            events: VecDeque::new(),
            event_count: 0,
            total: 0,
            deadline: deadline.instant(),
            sequence: 0,
            pending: BTreeMap::new(),
            responses: BTreeMap::new(),
        }
    }
    pub fn call(&mut self, session: Option<&str>, method: &str, params: Value) -> Result<Value> {
        let id = self.begin(session, method, params)?;
        loop {
            if let Some(response) = self.response(id) {
                return response;
            }
            self.pump()?;
        }
    }
    pub fn begin(&mut self, session: Option<&str>, method: &str, params: Value) -> Result<u64> {
        if self.pending.len() + self.responses.len() >= 4 {
            return Err(E::Limit);
        }
        let id = self.next;
        self.next = self.next.checked_add(1).ok_or(E::Limit)?;
        let mut command = json!({"id":id,"method":method,"params":params});
        if let Some(s) = session {
            command["sessionId"] = s.into();
        }
        let mut bytes = serde_json::to_vec(&command).map_err(|_| E::Protocol)?;
        // Assert JSON framing did not transform identity-bearing inspector bytes.
        if method == "Runtime.evaluate" {
            let decoded: Value = serde_json::from_slice(&bytes).map_err(|_| E::Protocol)?;
            if decoded["params"]["expression"] != command["params"]["expression"] {
                return Err(E::Inspection);
            }
        }
        if bytes.len() > MESSAGE_BYTES {
            return Err(E::Limit);
        }
        bytes.try_reserve(1).map_err(|_| E::Allocation)?;
        bytes.push(0);
        self.total = self.total.checked_add(bytes.len()).ok_or(E::Limit)?;
        if self.total > PROTOCOL_BYTES {
            return Err(E::Limit);
        }
        let deadline = self
            .deadline
            .min(Instant::now() + Duration::from_millis(COMMAND_MS));
        if Instant::now() >= deadline {
            return Err(E::Deadline);
        }
        self.sequence += 1;
        self.transport.send(&bytes, deadline)?;
        self.pending.insert(
            id,
            (session.map(str::to_owned), method.to_owned(), deadline),
        );
        Ok(id)
    }
    pub fn response(&mut self, id: u64) -> Option<Result<Value>> {
        self.responses.remove(&id)
    }
    pub fn pump(&mut self) -> Result<()> {
        let deadline = self
            .pending
            .values()
            .map(|v| v.2)
            .min()
            .unwrap_or(self.deadline)
            .min(self.deadline);
        let value = self.read(deadline)?;
        if let Some(id) = value.get("id") {
            let id = id.as_u64().ok_or(E::Acknowledgement)?;
            let (session, method, _) = self.pending.remove(&id).ok_or(E::Acknowledgement)?;
            if value.get("sessionId").and_then(Value::as_str) != session.as_deref() {
                return Err(E::Acknowledgement);
            }
            let result = if value.get("error").is_some() {
                Err(if method == "Emulation.setScriptExecutionDisabled" {
                    E::ScriptingControl
                } else {
                    E::Protocol
                })
            } else {
                value.get("result").cloned().ok_or(E::Protocol)
            };
            self.responses.insert(id, result);
            Ok(())
        } else {
            self.queue(value)
        }
    }
    fn queue(&mut self, v: Value) -> Result<()> {
        self.event_count = self.event_count.checked_add(1).ok_or(E::Limit)?;
        if self.event_count > EVENTS {
            return Err(E::Limit);
        }
        self.events.try_reserve(1).map_err(|_| E::Allocation)?;
        self.events.push_back(v);
        Ok(())
    }
    fn read(&mut self, deadline: Instant) -> Result<Value> {
        if Instant::now() >= deadline {
            return Err(E::Deadline);
        }
        let bytes = self.transport.receive(deadline)?;
        self.decode(bytes)
    }
    fn decode(&mut self, bytes: Vec<u8>) -> Result<Value> {
        self.total = self
            .total
            .checked_add(bytes.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(E::Limit)?;
        if bytes.len() > MESSAGE_BYTES || self.total > PROTOCOL_BYTES {
            return Err(E::Limit);
        }
        self.sequence = self.sequence.checked_add(1).ok_or(E::Limit)?;
        let v: Value = serde_json::from_slice(&bytes).map_err(|_| E::Protocol)?;
        if !v.is_object()
            || (v.get("id").is_none() && (!v["method"].is_string() || !v["params"].is_object()))
        {
            return Err(E::Protocol);
        }
        Ok(v)
    }
    pub fn quiescent_event(&mut self) -> Result<Option<Value>> {
        if Instant::now() >= self.deadline {
            return Err(E::Deadline);
        }
        if !self.pending.is_empty() || !self.responses.is_empty() {
            return Err(E::Acknowledgement);
        }
        if let Some(e) = self.queued() {
            return Ok(Some(e));
        }
        let Some(bytes) = self.transport.receive_quiescent()? else {
            return Ok(None);
        };
        let e = self.decode(bytes)?;
        if e.get("id").is_some() {
            return Err(E::Acknowledgement);
        }
        self.queue(e)?;
        Ok(self.queued())
    }
    pub fn event(&mut self) -> Result<Value> {
        loop {
            if let Some(v) = self.events.pop_front() {
                return Ok(v);
            }
            self.pump()?;
        }
    }
    pub fn queued(&mut self) -> Option<Value> {
        self.events.pop_front()
    }
}

/// Incremental NUL-delimited CDP pipe framing; no unbounded read_until.
#[derive(Default)]
pub struct Framer {
    pending: Vec<u8>,
    ready: VecDeque<Vec<u8>>,
    total: usize,
}
impl Framer {
    pub fn push(&mut self, chunk: &[u8]) -> Result<()> {
        self.total = self.total.checked_add(chunk.len()).ok_or(E::Limit)?;
        if self.total > PROTOCOL_BYTES {
            return Err(E::Limit);
        }
        for part in chunk.split_inclusive(|b| *b == 0) {
            let ended = part.last() == Some(&0);
            let content = if ended { &part[..part.len() - 1] } else { part };
            if self
                .pending
                .len()
                .checked_add(content.len())
                .ok_or(E::Limit)?
                > MESSAGE_BYTES
            {
                return Err(E::Limit);
            }
            self.pending
                .try_reserve(content.len())
                .map_err(|_| E::Allocation)?;
            self.pending.extend_from_slice(content);
            if ended {
                if self.pending.is_empty() {
                    return Err(E::Framing);
                }
                if self.ready.len() >= EVENTS {
                    return Err(E::Limit);
                }
                self.ready.try_reserve(1).map_err(|_| E::Allocation)?;
                self.ready.push_back(std::mem::take(&mut self.pending));
            }
        }
        Ok(())
    }
    pub fn pop(&mut self) -> Option<Vec<u8>> {
        self.ready.pop_front()
    }
}

#[cfg(unix)]
pub struct PipeTransport {
    stream: std::os::unix::net::UnixStream,
    framer: Framer,
}
#[cfg(unix)]
impl PipeTransport {
    pub fn new(stream: std::os::unix::net::UnixStream) -> Result<Self> {
        stream.set_nonblocking(true).map_err(|_| E::Protocol)?;
        Ok(Self {
            stream,
            framer: Framer::default(),
        })
    }
}
#[cfg(unix)]
fn wait(fd: std::os::fd::RawFd, event: libc::c_short, deadline: Instant) -> Result<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(E::Deadline)?;
    let timeout = i32::try_from(remaining.as_millis().max(1)).map_err(|_| E::Limit)?;
    let mut p = libc::pollfd {
        fd,
        events: event,
        revents: 0,
    };
    // SAFETY: one initialized pollfd, no borrowed buffers across the call.
    match unsafe { libc::poll(&mut p, 1, timeout) } {
        0 => Err(E::Deadline),
        -1 => Err(E::Protocol),
        _ => {
            if p.revents & event != 0 {
                Ok(())
            } else {
                Err(E::Protocol)
            }
        }
    }
}
#[cfg(unix)]
impl Transport for PipeTransport {
    fn send(&mut self, mut bytes: &[u8], deadline: Instant) -> Result<()> {
        use std::{io::Write, os::fd::AsRawFd};
        while !bytes.is_empty() {
            wait(self.stream.as_raw_fd(), libc::POLLOUT, deadline)?;
            match self.stream.write(bytes) {
                Ok(0) => return Err(E::Protocol),
                Ok(n) => bytes = &bytes[n..],
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(_) => return Err(E::Protocol),
            }
        }
        Ok(())
    }
    fn receive_quiescent(&mut self) -> Result<Option<Vec<u8>>> {
        use std::io::Read;
        loop {
            if let Some(v) = self.framer.pop() {
                return Ok(Some(v));
            }
            let mut chunk = [0u8; 8192];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(E::Framing),
                Ok(n) => self.framer.push(&chunk[..n])?,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return if self.framer.pending.is_empty() {
                        Ok(None)
                    } else {
                        Err(E::Framing)
                    };
                }
                Err(_) => return Err(E::Protocol),
            }
        }
    }
    fn receive(&mut self, deadline: Instant) -> Result<Vec<u8>> {
        use std::{io::Read, os::fd::AsRawFd};
        let mut chunk = [0u8; 8192];
        loop {
            if let Some(v) = self.framer.pop() {
                return Ok(v);
            }
            wait(self.stream.as_raw_fd(), libc::POLLIN, deadline)?;
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(E::Framing),
                Ok(n) => self.framer.push(&chunk[..n])?,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(_) => return Err(E::Protocol),
            }
        }
    }
}

#[cfg(test)]
pub(crate) struct Scripted {
    pub replies: VecDeque<Value>,
    pub sent: Vec<Value>,
}
#[cfg(test)]
impl Transport for Scripted {
    fn send(&mut self, b: &[u8], _: Instant) -> Result<()> {
        self.sent
            .push(serde_json::from_slice(&b[..b.len() - 1]).unwrap());
        Ok(())
    }
    fn receive(&mut self, _: Instant) -> Result<Vec<u8>> {
        serde_json::to_vec(&self.replies.pop_front().ok_or(E::Completion)?).map_err(|_| E::Protocol)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn quiescent_pipe_drain_rejects_partial_frames_and_eof() {
        use std::{io::Write, os::unix::net::UnixStream};
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut p = Protocol::new(PipeTransport::new(a).unwrap());
        assert!(p.quiescent_event().unwrap().is_none());
        b.write_all(
            b"{\"sessionId\":\"s\",\"method\":\"Inspector.targetCrashed\",\"params\":{}}\0",
        )
        .unwrap();
        assert_eq!(
            p.quiescent_event().unwrap().unwrap()["method"],
            "Inspector.targetCrashed"
        );
        b.write_all(b"{").unwrap();
        assert_eq!(p.quiescent_event(), Err(E::Framing));
        drop(b);
        assert_eq!(p.quiescent_event(), Err(E::Framing));
    }
    #[test]
    fn malformed_json_and_outstanding_request_limit() {
        struct Malformed;
        impl Transport for Malformed {
            fn send(&mut self, _: &[u8], _: Instant) -> Result<()> {
                Ok(())
            }
            fn receive(&mut self, _: Instant) -> Result<Vec<u8>> {
                Ok(b"{invalid".to_vec())
            }
        }
        let mut p = Protocol::new(Malformed);
        assert_eq!(p.call(None, "x", json!({})), Err(E::Protocol));
        let mut p = Protocol::new(Scripted {
            replies: VecDeque::new(),
            sent: vec![],
        });
        for _ in 0..4 {
            p.begin(None, "x", json!({})).unwrap();
        }
        assert_eq!(p.begin(None, "x", json!({})), Err(E::Limit));
    }
    #[test]
    fn framing_limits_and_fragmentation() {
        let mut f = Framer::default();
        f.push(b"{\"id\"").unwrap();
        f.push(b":1}\0").unwrap();
        assert_eq!(f.pop().unwrap(), b"{\"id\":1}");
        assert_eq!(f.push(b"\0"), Err(E::Framing));
        let mut f = Framer::default();
        assert_eq!(f.push(&vec![b'x'; MESSAGE_BYTES + 1]), Err(E::Limit));
        let mut f = Framer {
            total: PROTOCOL_BYTES,
            ..Default::default()
        };
        assert_eq!(f.push(b"x"), Err(E::Limit));
    }
    #[test]
    fn stale_ack_and_script_failure() {
        for (reply, expected) in [
            (json!({"id":2,"result":{}}), E::Acknowledgement),
            (json!({"id":1,"error":{}}), E::ScriptingControl),
        ] {
            let mut p = Protocol::new(Scripted {
                replies: [reply].into(),
                sent: vec![],
            });
            assert_eq!(
                p.call(
                    None,
                    "Emulation.setScriptExecutionDisabled",
                    json!({"value":true})
                ),
                Err(expected)
            );
        }
        let mut p = Protocol::new(Scripted {
            replies: [json!({"id":1,"result":{}})].into(),
            sent: vec![],
        });
        p.deadline = Instant::now();
        assert_eq!(p.call(None, "x", json!({})), Err(E::Deadline));
    }
    #[test]
    fn event_and_cumulative_budget() {
        let mut p = Protocol::new(Scripted {
            replies: [json!({"method":"x","params":{}})].into(),
            sent: vec![],
        });
        p.event_count = EVENTS;
        assert_eq!(p.event(), Err(E::Limit));
        let mut p = Protocol::new(Scripted {
            replies: [json!({"method":"x","params":{}})].into(),
            sent: vec![],
        });
        p.total = PROTOCOL_BYTES;
        assert_eq!(p.event(), Err(E::Limit));
    }
}

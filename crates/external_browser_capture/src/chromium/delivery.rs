use super::protocol::{Protocol, Transport};
use crate::{CaptureError as E, Result, configuration::Configuration, limits::FIXTURE_BYTES};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

pub(crate) fn validate_input(input: &[u8]) -> Result<()> {
    if input.len() > FIXTURE_BYTES {
        return Err(E::Limit);
    }
    if input.starts_with(&[0xef, 0xbb, 0xbf]) || std::str::from_utf8(input).is_err() {
        return Err(E::Field);
    }
    Ok(())
}

// Repeated start notifications are permitted only for this exact navigation.
pub(crate) const MAX_NAVIGATION_STARTS: usize = 64;
pub(crate) fn navigation_start(v: &Value, frame: &str, url: &str) -> Result<String> {
    if v["frameId"] != frame || v["url"] != url || v["navigationType"] != "differentDocument" {
        return Err(E::UnexpectedNavigation);
    }
    text(v, "loaderId")
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum StaticDomPolicyRejection {
    MetaRefresh {
        frame: String,
        loader: String,
        destination: String,
    },
    ChildFrame {
        parent: String,
        loader: String,
        child: String,
    },
}

pub(crate) struct DocumentState {
    pub frame: String,
    pub loader: Option<String>,
    pub network: Option<String>,
    pub fulfilled: bool,
    request_seen: bool,
    navigation_starts: usize,
    frame_committed: bool,
    pub response: bool,
    pub extra_response: bool,
    pub finished: bool,
    pub parsed: bool,
    pub loaded: bool,
    pub denied: usize,
}
impl DocumentState {
    pub fn new(frame: String) -> Self {
        Self {
            frame,
            loader: None,
            network: None,
            fulfilled: false,
            request_seen: false,
            navigation_starts: 0,
            frame_committed: false,
            response: false,
            extra_response: false,
            finished: false,
            parsed: false,
            loaded: false,
            denied: 0,
        }
    }
    pub fn complete(&self) -> bool {
        self.loader.is_some()
            && self.navigation_starts > 0
            && self.request_seen
            && self.frame_committed
            && self.fulfilled
            && self.response
            && self.extra_response
            && self.finished
            && self.parsed
            && self.loaded
    }
    fn bind_loader(&mut self, loader: String) -> Result<()> {
        if self.loader.as_ref().is_some_and(|old| old != &loader) {
            return Err(E::DocumentIdentity);
        }
        self.loader = Some(loader);
        Ok(())
    }
    pub fn navigation(&mut self, value: &Value) -> Result<()> {
        if value.get("errorText").is_some() || value["frameId"].as_str() != Some(&self.frame) {
            return Err(E::UnexpectedNavigation);
        }
        let loader = text(value, "loaderId")?;
        self.bind_loader(loader)
    }
    // Validate identity collisions before resource-type guards can classify traffic
    // as unrelated. This also runs at observation barriers.
    pub fn collisions(&self, event: &Value) -> Result<()> {
        let method = event["method"].as_str().ok_or(E::Protocol)?;
        let v = &event["params"];
        let controlled = |key: &str| {
            self.network
                .as_deref()
                .is_some_and(|n| v[key].as_str() == Some(n))
        };
        match method {
            "Network.requestWillBeSent" | "Network.responseReceived" if controlled("requestId") => {
                if v["type"] != "Document"
                    || v["frameId"] != self.frame
                    || self.loader.as_deref().is_some_and(|l| v["loaderId"] != l)
                {
                    return Err(E::DocumentIdentity);
                }
            }
            "Fetch.requestPaused" if controlled("networkId") => {
                if v["resourceType"] != "Document" || v["frameId"] != self.frame {
                    return Err(E::DocumentIdentity);
                }
            }
            "Page.lifecycleEvent" => {
                if v["frameId"] != self.frame
                    || self.loader.as_deref().is_some_and(|l| v["loaderId"] != l)
                {
                    return Err(E::DocumentIdentity);
                }
            }
            "Network.loadingFinished" | "Network.loadingFailed" => {
                text(v, "requestId")?;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn event<T: Transport>(
        &mut self,
        p: &mut Protocol<T>,
        session: &str,
        c: &Configuration,
        input: &[u8],
        event: &Value,
    ) -> Result<Option<StaticDomPolicyRejection>> {
        self.collisions(event)?;
        let method = text(event, "method")?;
        let v = &event["params"];
        match method.as_str() {
            "Page.frameStartedNavigating" => {
                let loader = navigation_start(v, &self.frame, &c.target_url)?;
                self.bind_loader(loader)?;
                self.navigation_starts = self.navigation_starts.checked_add(1).ok_or(E::Limit)?;
                if self.navigation_starts > MAX_NAVIGATION_STARTS {
                    return Err(E::Limit);
                }
            }
            "Fetch.requestPaused" => {
                let id = text(v, "requestId")?;
                if v["resourceType"] == "Document" {
                    if self.fulfilled
                        || v["frameId"].as_str() != Some(&self.frame)
                        || v["request"]["url"] != c.target_url
                        || v["request"]["method"] != "GET"
                        || v.get("redirectedRequestId").is_some()
                        || v.get("responseStatusCode").is_some()
                        || v.get("responseErrorReason").is_some()
                        || v.get("responseHeaders").is_some()
                    {
                        p.call(
                            Some(session),
                            "Fetch.failRequest",
                            json!({"requestId":id,"errorReason":"BlockedByClient"}),
                        )?;
                        return Err(E::UnexpectedNavigation);
                    }
                    validate_input(input)?;
                    let network = text(v, "networkId")?;
                    if self.network.as_ref().is_some_and(|old| old != &network) {
                        return Err(E::DocumentIdentity);
                    }
                    self.network = Some(network);
                    let body = STANDARD.encode(input);
                    if STANDARD.decode(&body).map_err(|_| E::Protocol)? != input {
                        return Err(E::Digest);
                    }
                    p.call(
                        Some(session),
                        "Fetch.fulfillRequest",
                        json!({"requestId":id,"responseCode":200,
                        "responseHeaders":[{"name":"Content-Type","value":c.content_type_header},
                        {"name":"Content-Length","value":input.len().to_string()},
                        {"name":"Cache-Control","value":"no-store"}],"body":body}),
                    )?;
                    self.fulfilled = true;
                } else {
                    p.call(
                        Some(session),
                        "Fetch.failRequest",
                        json!({"requestId":id,"errorReason":"BlockedByClient"}),
                    )?;
                    self.denied = self.denied.checked_add(1).ok_or(E::Limit)?;
                }
            }
            "Network.requestWillBeSent" if v["type"] == "Document" => {
                if self.request_seen
                    || v["frameId"].as_str() != Some(&self.frame)
                    || v["request"]["method"] != "GET"
                    || v["request"]["url"] != c.target_url
                    || v.get("redirectResponse").is_some()
                {
                    return Err(E::UnexpectedNavigation);
                }
                let network = text(v, "requestId")?;
                if self.network.as_ref().is_some_and(|old| old != &network) {
                    return Err(E::DocumentIdentity);
                }
                self.network = Some(network);
                let loader = text(v, "loaderId")?;
                self.bind_loader(loader)?;
                self.request_seen = true;
            }
            "Network.responseReceived" if v["type"] == "Document" => {
                if self.response
                    || self.network.is_none()
                    || self.loader.is_none()
                    || v["requestId"].as_str() != self.network.as_deref()
                    || v["loaderId"].as_str() != self.loader.as_deref()
                    || v["frameId"].as_str() != Some(&self.frame)
                    || v["response"]["url"] != c.target_url
                    || v["response"]["status"] != 200
                    || v["response"]["mimeType"] != "text/html"
                    || v["response"]["fromDiskCache"] != false
                    || v["response"]["fromServiceWorker"] != false
                    || v["hasExtraInfo"] != true
                {
                    return Err(E::DocumentIdentity);
                }
                for flag in ["fromPrefetchCache", "fromEarlyHints"] {
                    if v["response"].get(flag).is_some_and(|v| v != false) {
                        return Err(E::DocumentIdentity);
                    }
                }
                for field in [
                    "serviceWorkerRouterInfo",
                    "serviceWorkerResponseSource",
                    "cacheStorageCacheName",
                ] {
                    if v["response"].get(field).is_some() {
                        return Err(E::DocumentIdentity);
                    }
                }
                required_headers(&v["response"]["headers"], input.len())?;
                self.response = true;
            }
            "Network.responseReceivedExtraInfo" => {
                if self.network.is_none()
                    || v["requestId"].as_str() != self.network.as_deref()
                    || self.extra_response
                    || v["statusCode"] != 200
                {
                    return Err(E::DocumentIdentity);
                }
                // ExtraInfo explicitly preserves duplicate values with newline separators.
                required_headers(&v["headers"], input.len())?;
                if let Some(raw) = v.get("headersText") {
                    raw_headers(raw.as_str().ok_or(E::Protocol)?, input.len())?;
                }
                self.extra_response = true;
            }
            "Network.requestServedFromCache" | "Network.responseReceivedEarlyHints" => {
                return Err(E::DocumentIdentity);
            }
            "Network.loadingFinished"
                if self.network.is_some() && v["requestId"].as_str() == self.network.as_deref() =>
            {
                if self.finished {
                    return Err(E::Completion);
                }
                self.finished = true
            }
            "Network.loadingFailed"
                if self.network.is_some() && v["requestId"].as_str() == self.network.as_deref() =>
            {
                return Err(E::Completion);
            }
            "Page.frameNavigated" => {
                if self.frame_committed
                    || v["frame"]["id"].as_str() != Some(&self.frame)
                    || v["frame"]["url"] != c.target_url
                    || v["frame"].get("parentId").is_some()
                {
                    return Err(E::UnexpectedNavigation);
                }
                let loader = text(&v["frame"], "loaderId")?;
                self.bind_loader(loader)?;
                self.frame_committed = true;
            }
            "Page.lifecycleEvent" => {
                if v["frameId"].as_str() != Some(&self.frame) {
                    return Err(E::DocumentIdentity);
                }
                if matches!(v["name"].as_str(), Some("DOMContentLoaded" | "load")) {
                    if v["loaderId"].as_str() != self.loader.as_deref() {
                        return Err(E::DocumentIdentity);
                    }
                    if v["name"] == "DOMContentLoaded" {
                        self.parsed = true;
                    } else {
                        self.loaded = true;
                    }
                }
            }
            "Page.frameAttached" => {
                let parent = text(v, "parentFrameId")?;
                let child = text(v, "frameId")?;
                if parent != self.frame || child == self.frame || !self.fulfilled {
                    return Err(E::DocumentIdentity);
                }
                return Ok(Some(StaticDomPolicyRejection::ChildFrame {
                    parent,
                    child,
                    loader: self.loader.clone().ok_or(E::DocumentIdentity)?,
                }));
            }
            "Page.frameScheduledNavigation" | "Page.frameRequestedNavigation"
                if v["reason"] == "metaTagRefresh" =>
            {
                if v["frameId"] != self.frame || !self.fulfilled {
                    return Err(E::DocumentIdentity);
                }
                return Ok(Some(StaticDomPolicyRejection::MetaRefresh {
                    frame: self.frame.clone(),
                    loader: self.loader.clone().ok_or(E::DocumentIdentity)?,
                    destination: text(v, "url")?,
                }));
            }
            "Page.frameDetached"
            | "Page.frameRequestedNavigation"
            | "Page.frameScheduledNavigation"
            | "Page.javascriptDialogOpening" => return Err(E::UnexpectedNavigation),
            "Network.requestWillBeSent" | "Network.responseReceived" => {
                text(v, "requestId")?;
                text(v, "type")?;
            }
            "Network.loadingFinished"
            | "Network.loadingFailed"
            | "Network.dataReceived"
            | "Network.requestWillBeSentExtraInfo"
            | "Page.loadEventFired"
            | "Page.domContentEventFired" => {}
            "Page.frameStartedLoading" | "Page.frameStoppedLoading" => {
                if v["frameId"] != self.frame {
                    return Err(E::DocumentIdentity);
                }
            }
            "Runtime.executionContextCreated" => {
                if v["context"]["auxData"]["frameId"] != self.frame {
                    return Err(E::RealmIdentity);
                }
            }
            "Runtime.executionContextDestroyed" | "Runtime.executionContextsCleared" => {}
            _ => return Err(E::UnexpectedEvent),
        }
        Ok(None)
    }
}
pub(crate) fn text(v: &Value, key: &str) -> Result<String> {
    let s = v[key].as_str().ok_or(E::Protocol)?;
    if s.is_empty() || s.len() > 1024 {
        return Err(E::Limit);
    }
    Ok(s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_no_bom_and_size_policy() {
        assert_eq!(validate_input(&[0xef, 0xbb, 0xbf, b'x']), Err(E::Field));
        assert_eq!(validate_input(&[0xff]), Err(E::Field));
        assert_eq!(
            validate_input(&vec![b'x'; FIXTURE_BYTES + 1]),
            Err(E::Limit)
        );
        assert!(validate_input("é水🙂".as_bytes()).is_ok());
    }
    #[test]
    fn stale_loader_and_navigation_rejected() {
        let mut d = DocumentState::new("f".into());
        d.loader = Some("l".into());
        assert_eq!(
            d.navigation(&json!({"frameId":"f","loaderId":"old"})),
            Err(E::DocumentIdentity)
        );
        assert_eq!(
            d.navigation(&json!({"frameId":"other","loaderId":"l"})),
            Err(E::UnexpectedNavigation)
        );
    }
    #[test]
    fn frame_and_redirect_are_not_completion() {
        let mut d = DocumentState::new("f".into());
        let mut p = Protocol::new(super::super::protocol::Scripted {
            replies: [].into(),
            sent: vec![],
        });
        let c = crate::configuration::specimen();
        assert_eq!(
            d.event(
                &mut p,
                "s",
                &c,
                b"",
                &json!({"method":"Page.frameAttached","params":{"frameId":"child","parentFrameId":"f"}})
            ),
            Err(E::DocumentIdentity)
        );
        assert_eq!(d.event(&mut p,"s",&c,b"",&json!({"method":"Network.requestWillBeSent","params":{"type":"Document","frameId":"f","request":{"url":c.target_url},"redirectResponse":{}}})),Err(E::UnexpectedNavigation));
    }
}

fn required_headers(value: &Value, len: usize) -> Result<()> {
    let map = value.as_object().ok_or(E::Protocol)?;
    let length = len.to_string();
    for (name, wanted) in [
        ("content-type", "text/html; charset=utf-8"),
        ("content-length", length.as_str()),
        ("cache-control", "no-store"),
    ] {
        let mut values = map.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name));
        let (_, v) = values.next().ok_or(E::DocumentIdentity)?;
        if values.next().is_some() || v.as_str() != Some(wanted) {
            return Err(E::DocumentIdentity);
        }
    }
    Ok(())
}
fn raw_headers(raw: &str, len: usize) -> Result<()> {
    if raw.len() > 65536 || !raw.ends_with("\r\n\r\n") {
        return Err(E::Protocol);
    }
    let mut lines = raw[..raw.len() - 4].split("\r\n");
    let status = lines.next().ok_or(E::Protocol)?;
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(E::DocumentIdentity);
    }
    let mut map = serde_json::Map::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(E::Protocol)?;
        if name.is_empty() || name.trim() != name || line.starts_with([' ', '\t']) {
            return Err(E::Protocol);
        }
        let key = name.to_ascii_lowercase();
        if map
            .insert(key, Value::String(value.trim_matches([' ', '\t']).into()))
            .is_some()
        {
            return Err(E::DocumentIdentity);
        }
    }
    required_headers(&Value::Object(map), len)
}
#[cfg(test)]
mod header_tests {
    use super::*;
    #[test]
    fn response_requires_complete_headers_and_explicit_fresh_provenance() {
        let c = crate::configuration::specimen();
        let good = json!({"method":"Network.responseReceived","params":{"type":"Document","requestId":"n","frameId":"f","loaderId":"l","hasExtraInfo":true,"response":{"url":c.target_url,"status":200,"mimeType":"text/html","fromDiskCache":false,"fromServiceWorker":false,"headers":{"Content-Type":"text/html; charset=utf-8","Content-Length":"3","Cache-Control":"no-store"}}}});
        let evaluate = |event: Value| {
            let mut d = DocumentState::new("f".into());
            d.network = Some("n".into());
            d.loader = Some("l".into());
            let mut p = Protocol::new(super::super::protocol::Scripted {
                replies: [].into(),
                sent: vec![],
            });
            d.event(&mut p, "s", &c, b"abc", &event)
        };
        assert_eq!(evaluate(good.clone()), Ok(None));
        for field in ["fromDiskCache", "fromServiceWorker"] {
            let mut bad = good.clone();
            bad["params"]["response"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(evaluate(bad).is_err());
        }
        for field in [
            "fromDiskCache",
            "fromServiceWorker",
            "fromPrefetchCache",
            "fromEarlyHints",
        ] {
            let mut bad = good.clone();
            bad["params"]["response"][field] = true.into();
            assert!(evaluate(bad).is_err());
        }
        for (field, value) in [
            ("status", json!(304)),
            ("mimeType", json!("text/plain")),
            ("serviceWorkerResponseSource", json!("network")),
        ] {
            let mut bad = good.clone();
            bad["params"]["response"][field] = value;
            assert!(evaluate(bad).is_err());
        }
        let mut bad = good;
        bad["params"]["hasExtraInfo"] = false.into();
        assert!(evaluate(bad).is_err());
    }
    #[test]
    fn required_headers_and_raw_duplicates() {
        let good = json!({"Content-Type":"text/html; charset=utf-8","Content-Length":"3","Cache-Control":"no-store"});
        assert!(required_headers(&good, 3).is_ok());
        for key in ["Content-Type", "Content-Length", "Cache-Control"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(required_headers(&bad, 3).is_err());
            for wrong in ["wrong", "no-store\nno-store", "3, 3"] {
                let mut bad = good.clone();
                bad[key] = wrong.into();
                assert!(required_headers(&bad, 3).is_err());
            }
            let mut bad = good.clone();
            bad[key.to_ascii_lowercase()] = good[key].clone();
            assert!(required_headers(&bad, 3).is_err());
        }
        let good = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 3\r\nCache-Control: no-store\r\n\r\n";
        assert!(raw_headers(good, 3).is_ok());
        for extra in [
            "Content-Length: 3",
            "content-length: 4",
            "Cache-Control: no-store",
            "Content-Type: text/html; charset=utf-8",
        ] {
            assert!(
                raw_headers(
                    &good.replace("\r\n\r\n", &format!("\r\n{extra}\r\n\r\n")),
                    3
                )
                .is_err()
            );
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    #[test]
    fn policy_events_require_committed_main_frame_and_exact_reason() {
        let c = crate::configuration::specimen();
        let mut p = Protocol::new(super::super::protocol::Scripted {
            replies: [].into(),
            sent: vec![],
        });
        let mut d = DocumentState::new("f".into());
        d.fulfilled = true;
        d.loader = Some("l".into());
        let redirect = json!({"method":"Page.frameScheduledNavigation","params":{"frameId":"f","reason":"metaTagRefresh","url":"http://ag9g.invalid/redirected.html"}});
        assert!(matches!(
            d.event(&mut p, "s", &c, b"", &redirect),
            Ok(Some(StaticDomPolicyRejection::MetaRefresh { .. }))
        ));
        let mut stale = redirect.clone();
        stale["params"]["frameId"] = "old".into();
        assert_eq!(
            d.event(&mut p, "s", &c, b"", &stale),
            Err(E::DocumentIdentity)
        );
        let mut wrong = redirect;
        wrong["params"]["reason"] = "scriptInitiated".into();
        assert_eq!(
            d.event(&mut p, "s", &c, b"", &wrong),
            Err(E::UnexpectedNavigation)
        );
        let child =
            json!({"method":"Page.frameAttached","params":{"frameId":"child","parentFrameId":"f"}});
        assert!(matches!(
            d.event(&mut p, "s", &c, b"", &child),
            Ok(Some(StaticDomPolicyRejection::ChildFrame { .. }))
        ));
        d.fulfilled = false;
        assert_eq!(
            d.event(&mut p, "s", &c, b"", &child),
            Err(E::DocumentIdentity)
        );
    }
}

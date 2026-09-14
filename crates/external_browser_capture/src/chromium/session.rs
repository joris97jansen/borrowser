use super::{
    delivery::{DocumentState, text},
    events::{EventBoundary, ScopedEvent},
    inspection,
    protocol::{Protocol, Transport},
};
use crate::{
    CaptureError as E, Result, configuration::Configuration, packaging::InspectorExpressionV1,
};
use serde_json::{Value, json};

pub(crate) struct PreparedSession<T> {
    protocol: Protocol<T>,
    session: String,
    events: EventBoundary,
    frame: String,
    document: Option<DocumentState>,
}
#[derive(Debug)]
pub(crate) enum CaptureOutcome {
    Observation(Observation),
    Rejected(super::delivery::StaticDomPolicyRejection),
}
#[derive(Debug)]
pub(crate) struct Observation {
    pub bytes: Vec<u8>,
    pub denied_requests: usize,
    pub document: String,
    pub realm: String,
}
impl<T: Transport> PreparedSession<T> {
    pub fn prepare(
        transport: T,
        c: &Configuration,
        deadline: crate::deadline::AttemptDeadline,
    ) -> Result<Self> {
        deadline.check()?;
        let mut p = Protocol::with_deadline(transport, deadline);
        let version = p.call(None, "Browser.getVersion", json!({}))?;
        if version["product"] != format!("{}/{}", c.browser_product, c.browser_version)
            || version["protocolVersion"] != c.cdp_protocol_version
            || c.browser_build_revision
                .as_ref()
                .is_some_and(|r| version["revision"] != *r)
        {
            return Err(E::ProcessIdentity);
        }
        if c.browser_build_revision.is_none()
            && version
                .get("revision")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
        {
            return Err(E::ProcessIdentity);
        }
        // Register lifecycle observation before any authoritative inventory.
        // Protocol retains every event interleaved with these command responses.
        p.call(None, "Target.setDiscoverTargets", json!({"discover":true}))?;
        let initial = EventBoundary::initial(&p.call(None, "Target.getTargets", json!({}))?)?;
        let context = text(
            &p.call(
                None,
                "Target.createBrowserContext",
                json!({"disposeOnDetach":true}),
            )?,
            "browserContextId",
        );
        let context = context?;
        let target = text(
            &p.call(
                None,
                "Target.createTarget",
                json!({"url":"about:blank","browserContextId":context} ),
            )?,
            "targetId",
        )?;
        let session = text(
            &p.call(
                None,
                "Target.attachToTarget",
                json!({"targetId":target,"flatten":true}),
            )?,
            "sessionId",
        )?;
        p.call(
            Some(&session),
            "Target.setAutoAttach",
            json!({"autoAttach":true,"waitForDebuggerOnStart":true,"flatten":true}),
        )?;
        for method in [
            "Inspector.enable",
            "Page.enable",
            "Runtime.enable",
            "Network.enable",
        ] {
            p.call(Some(&session), method, json!({}))?;
        }
        p.call(
            Some(&session),
            "Network.setCacheDisabled",
            json!({"cacheDisabled":true}),
        )?;
        p.call(
            Some(&session),
            "Network.setBypassServiceWorker",
            json!({"bypass":true}),
        )?;
        p.call(
            Some(&session),
            "Emulation.setScriptExecutionDisabled",
            json!({"value":true}),
        )?;
        p.call(
            Some(&session),
            "Fetch.enable",
            json!({"patterns":[{"urlPattern":"*","requestStage":"Request"}]}),
        )?;
        let tree = p.call(Some(&session), "Page.getFrameTree", json!({}))?;
        if tree["frameTree"].get("childFrames").is_some()
            || tree["frameTree"]["frame"].get("parentId").is_some()
        {
            return Err(E::DocumentIdentity);
        }
        let frame = text(&tree["frameTree"]["frame"], "id")?;
        if tree["frameTree"]["frame"]["url"] != "about:blank" {
            return Err(E::UnexpectedNavigation);
        }
        let events = EventBoundary::new(
            session.clone(),
            target,
            context,
            initial,
            c.target_url.clone(),
        )?;
        let loader = text(&tree["frameTree"]["frame"], "loaderId")?;
        // Blink immediately replays the current DocumentLoader's observed phases.
        // Establish the committed baseline before requesting that replay; Page
        // was already enabled so pending-navigation starts remain observable.
        p.call(
            Some(&session),
            "Page.setLifecycleEventsEnabled",
            json!({"enabled":true}),
        )?;
        while let Some(e) = p.queued() {
            events.startup(&e, &frame, &loader)?;
        }
        // Notifications may follow the inventory or creation/attachment replies.
        // Wait for the exact ledger facts, never for an idle interval.
        while !events.startup_complete() {
            events.startup(&p.event()?, &frame, &loader)?;
        }
        let inventory = p.call(None, "Target.getTargets", json!({}))?;
        // Explicit second barrier consumes events arriving after the inventory
        // response and checks the same private blank document before navigation.
        let final_tree = p.call(Some(&session), "Page.getFrameTree", json!({}))?;
        while let Some(e) = p.queued() {
            events.startup(&e, &frame, &loader)?;
        }
        events.reconcile(&inventory)?;
        events.startup_barrier(&final_tree, &frame, &loader)?;
        Ok(Self {
            protocol: p,
            session,
            events,
            frame,
            document: None,
        })
    }
    pub fn capture(
        &mut self,
        c: &Configuration,
        input: &[u8],
        expression: &InspectorExpressionV1,
    ) -> Result<CaptureOutcome> {
        super::delivery::validate_input(input)?;
        let mut d = DocumentState::new(self.frame.clone());
        let nav = self.protocol.begin(
            Some(&self.session),
            "Page.navigate",
            json!({"url":c.target_url}),
        )?;
        let mut acknowledged = false;
        while !acknowledged || !d.complete() {
            if let Some(v) = self.protocol.response(nav) {
                d.navigation(&v?)?;
                acknowledged = true;
            }
            while let Some(e) = self.protocol.queued() {
                let ScopedEvent::Attached(e) = self.events.validate(&e, false)? else {
                    continue;
                };
                if let Some(rejection) = d.event(&mut self.protocol, &self.session, c, input, e)? {
                    if let Some(v) = self.protocol.response(nav) {
                        d.navigation(&v?)?;
                        acknowledged = true;
                    }
                    if !acknowledged {
                        return Err(E::Completion);
                    }
                    self.document = Some(d);
                    return Ok(CaptureOutcome::Rejected(rejection));
                }
            }
            // Fulfillment can receive the outstanding navigation response while handling its own ack.
            if let Some(v) = self.protocol.response(nav) {
                d.navigation(&v?)?;
                acknowledged = true;
            }
            if !acknowledged || !d.complete() {
                self.protocol.pump()?;
            }
        }
        // Barrier; all queued navigation events are checked before selecting an inspection realm.
        self.verify_document(&d, c)?;
        while let Some(e) = self.protocol.queued() {
            let ScopedEvent::Attached(e) = self.events.validate(&e, false)? else {
                continue;
            };
            if let Some(rejection) = d.event(&mut self.protocol, &self.session, c, input, e)? {
                self.document = Some(d);
                return Ok(CaptureOutcome::Rejected(rejection));
            }
        }
        let (bytes, realm) = inspection::inspect(&mut self.protocol, &self.events, &d, expression)?;
        // Never sends false. Reassert the same disabled control, then bind the post-observation document.
        self.protocol.call(
            Some(&self.session),
            "Emulation.setScriptExecutionDisabled",
            json!({"value":true}),
        )?;
        self.verify_document(&d, c)?;
        while let Some(e) = self.protocol.queued() {
            let ScopedEvent::Attached(e) = self.events.validate(&e, false)? else {
                continue;
            };
            if e["method"] == "Runtime.executionContextDestroyed"
                || e["method"] == "Runtime.executionContextsCleared"
            {
                return Err(E::RealmIdentity);
            }
            if !inspection::benign(e) {
                return Err(E::UnexpectedEvent);
            }
            inspection::verify_event_binding(e, &d)?;
        }
        // Keep the renderer alive for the supervisor's post-observation sandbox/process checks.
        // This one-shot session is disposed by terminating/reaping its isolated browser process tree.
        let output = CaptureOutcome::Observation(Observation {
            bytes,
            denied_requests: d.denied,
            document: format!(
                "{}:{}:{}",
                self.events.target,
                d.frame,
                d.loader.as_ref().ok_or(E::Completion)?
            ),
            realm,
        });
        self.document = Some(d);
        Ok(output)
    }
    // Called after namespace-wide quiescence and before termination. The same
    // session boundary remains alive, so already emitted fatal events cannot be
    // lost by dropping the transport at the observation boundary.
    pub fn verify_quiescent_events(&mut self) -> Result<()> {
        let d = self.document.as_ref().ok_or(E::Completion)?;
        while let Some(e) = self.protocol.quiescent_event()? {
            let ScopedEvent::Attached(e) = self.events.validate(&e, false)? else {
                continue;
            };
            if !inspection::benign(e) {
                return Err(E::UnexpectedEvent);
            }
            inspection::verify_event_binding(e, d)?;
        }
        Ok(())
    }
    fn verify_document(&mut self, d: &DocumentState, c: &Configuration) -> Result<()> {
        let v = self
            .protocol
            .call(Some(&self.session), "Page.getFrameTree", json!({}))?;
        let f = &v["frameTree"]["frame"];
        if f["id"] != d.frame
            || f["loaderId"].as_str() != d.loader.as_deref()
            || f["url"] != c.target_url
            || v["frameTree"].get("childFrames").is_some()
        {
            return Err(E::DocumentIdentity);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Instant};
    #[derive(Clone, Copy)]
    enum Fault {
        Inject(&'static str, &'static str),
        Late(&'static str),
        Envelope(&'static str, u8),
        Collision(&'static str, &'static str, &'static str),
        Ancillary,
        StartAfterAck,
        RepeatStart,
        StartupStart,
        StartupNavigation(u8),
        DelayedDiscovery,
        InventoryMismatch,
        NegativeRedirect,
        NegativeChild,
        ProtocolDeadline,
        PostDeadline,
        None,
        WrongRealm,
        WrongFrame,
        NewTarget,
        ScriptControl,
        WrongSession,
        DestroyedTarget,
        Startup,
        DuplicateRequest,
        DuplicateFetch,
        ResponseReplacement,
        PostFrameReplacement,
        NetworkReplacement,
        LoaderReplacement,
        DuplicateResponse,
        DuplicateExtra,
        ExtraReplacement,
        MissingExtra,
        LoadingFailure,
        InspectionFailure,
        PostFailure,
        TargetCrash,
    }
    struct Peer {
        replies: VecDeque<Value>,
        trace: Rc<RefCell<Vec<Value>>>,
        nav: Option<u64>,
        committed: bool,
        fault: Fault,
    }
    impl Peer {
        fn event(&mut self, method: &str, mut params: Value) {
            if matches!(self.fault, Fault::MissingExtra)
                && method == "Network.responseReceivedExtraInfo"
            {
                return;
            }
            if matches!(self.fault, Fault::NetworkReplacement) && method == "Fetch.requestPaused" {
                params["networkId"] = "replacement".into();
            }
            if matches!(self.fault, Fault::LoaderReplacement)
                && method == "Network.responseReceived"
            {
                params["loaderId"] = "replacement".into();
            }
            if matches!(self.fault, Fault::ExtraReplacement)
                && method == "Network.responseReceivedExtraInfo"
            {
                params["requestId"] = "replacement".into();
            }
            if matches!(self.fault, Fault::ResponseReplacement)
                && method == "Network.responseReceived"
            {
                params["requestId"] = "replacement".into();
            }
            let method = if matches!(self.fault, Fault::LoadingFailure)
                && method == "Network.loadingFinished"
            {
                "Network.loadingFailed"
            } else {
                method
            };
            let mut event = json!({"sessionId":"s","method":method,"params":params});
            if let Fault::Envelope(selected, scope) = self.fault
                && method == selected
            {
                match scope {
                    0 => {
                        event.as_object_mut().unwrap().remove("sessionId");
                    }
                    1 => event["sessionId"] = Value::Null,
                    2 => event["sessionId"] = "wrong".into(),
                    _ => {}
                }
            }
            if let Fault::Collision(selected, key, value) = self.fault
                && method == selected
            {
                // Preserve the original event so identities are already retained.
                self.replies.push_back(event.clone());
                event["params"][key] = value.into();
            }
            if matches!(
                (self.fault, method),
                (Fault::DuplicateRequest, "Network.requestWillBeSent")
                    | (Fault::DuplicateFetch, "Fetch.requestPaused")
                    | (Fault::DuplicateResponse, "Network.responseReceived")
                    | (Fault::DuplicateExtra, "Network.responseReceivedExtraInfo")
            ) {
                self.replies.push_back(event.clone());
            }
            self.replies.push_back(event);
        }
    }
    impl Transport for Peer {
        fn send(&mut self, b: &[u8], _: Instant) -> Result<()> {
            let v: Value = serde_json::from_slice(&b[..b.len() - 1]).unwrap();
            self.trace.borrow_mut().push(v.clone());
            let id = v["id"].as_u64().unwrap();
            let method = v["method"].as_str().unwrap();
            if matches!(self.fault, Fault::ProtocolDeadline) && method == "Page.navigate" {
                return Err(E::Deadline);
            }
            if matches!(self.fault, Fault::PostDeadline)
                && method == "Emulation.setScriptExecutionDisabled"
                && self
                    .trace
                    .borrow()
                    .iter()
                    .filter(|v| v["method"] == method)
                    .count()
                    == 2
            {
                return Err(E::Deadline);
            }
            if let Fault::Inject(command, event) = self.fault
                && command == method
            {
                self.replies.push_back(serde_json::from_str(event).unwrap());
            }
            let mut result = json!({});
            match method {
                "Browser.getVersion" => result = json!({"product":"x/x","protocolVersion":"x"}),
                "Target.setDiscoverTargets" => {
                    if !matches!(self.fault, Fault::DelayedDiscovery) {
                        self.replies.push_back(json!({"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"initial","type":"page","url":"about:blank","attached":false}}}));
                    }
                }
                "Target.getTargets" => {
                    let final_inventory = self
                        .trace
                        .borrow()
                        .iter()
                        .filter(|v| v["method"] == method)
                        .count()
                        == 2;
                    result = json!({"targetInfos":[{"targetId":"initial","type":"page","url":"about:blank","attached":false}]});
                    if final_inventory && !matches!(self.fault, Fault::InventoryMismatch) {
                        result["targetInfos"].as_array_mut().unwrap().push(json!({"targetId":"t","type":"page","url":"about:blank","attached":true,"browserContextId":"ctx"}));
                    }
                    if !final_inventory && matches!(self.fault, Fault::DelayedDiscovery) {
                        self.replies.push_back(json!({"id":id,"result":result}));
                        self.replies.push_back(json!({"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"initial","type":"page","url":"about:blank","attached":false}}}));
                        return Ok(());
                    }
                }
                "Target.createBrowserContext" if matches!(self.fault, Fault::Startup) => {
                    self.event(
                        "Target.targetCreated",
                        json!({"targetInfo":{"type":"worker","url":"unexpected"}}),
                    );
                    result = json!({"browserContextId":"ctx"});
                }
                "Target.createBrowserContext" => result = json!({"browserContextId":"ctx"}),
                "Target.createTarget" => {
                    self.replies.push_back(json!({"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"t","type":"page","url":"about:blank","attached":false,"browserContextId":"ctx"}}}));
                    result = json!({"targetId":"t"});
                }
                "Target.attachToTarget" => {
                    self.replies.push_back(json!({"method":"Target.attachedToTarget","params":{"sessionId":"s","waitingForDebugger":false,"targetInfo":{"targetId":"t","type":"page","url":"about:blank","attached":true,"browserContextId":"ctx"}}}));
                    result = json!({"sessionId":"s"});
                }
                "Page.enable" if matches!(self.fault, Fault::StartupNavigation(_)) => {
                    let Fault::StartupNavigation(mode) = self.fault else {
                        unreachable!()
                    };
                    self.event("Page.frameStartedNavigating",json!({"frameId":"f","loaderId":"B","url":"about:blank","navigationType":"differentDocument"}));
                    if mode == 2 || mode == 10 {
                        self.event("Page.frameStartedNavigating",json!({"frameId":"f","loaderId":if mode == 2 { "B" } else { "C" },"url":"about:blank","navigationType":"differentDocument"}));
                    }
                    if mode == 1 {
                        self.event(
                            "Page.frameNavigated",
                            json!({"frame":{"id":"f","loaderId":"B","url":"about:blank"}}),
                        );
                    }
                    if mode == 8 {
                        self.event(
                            "Page.lifecycleEvent",
                            json!({"frameId":"f","loaderId":"C","name":"init"}),
                        );
                    }
                }
                "Page.enable" if matches!(self.fault, Fault::StartupStart) => {
                    self.event("Page.frameStartedNavigating",json!({"frameId":"f","loaderId":"blank","url":"about:blank","navigationType":"differentDocument"}));
                    self.event(
                        "Page.frameNavigated",
                        json!({"frame":{"id":"f","loaderId":"blank","url":"about:blank"}}),
                    );
                }
                "Page.setLifecycleEventsEnabled" => {
                    let mode = if let Fault::StartupNavigation(mode) = self.fault {
                        mode
                    } else {
                        255
                    };
                    if mode == 14 {
                        self.replies
                            .push_back(json!({"id":id,"sessionId":"s","error":{"code":-1}}));
                        return Ok(());
                    }
                    if mode == 11 || mode == 13 {
                        self.event(
                            "Page.frameNavigated",
                            json!({"frame":{"id":"f","loaderId":"B","url":"about:blank"}}),
                        );
                    }
                    let loader = match mode {
                        1 | 11 => "B",
                        12 => "C",
                        _ => "blank",
                    };
                    // Replay is delivered before the enable acknowledgement.
                    for name in [
                        "commit",
                        "DOMContentLoaded",
                        "load",
                        "networkAlmostIdle",
                        "networkIdle",
                    ] {
                        self.event(
                            "Page.lifecycleEvent",
                            json!({"frameId":"f","loaderId":loader,"name":name}),
                        );
                    }
                }
                "Emulation.setScriptExecutionDisabled"
                    if matches!(self.fault, Fault::ScriptControl) =>
                {
                    self.replies
                        .push_back(json!({"id":id,"sessionId":"s","error":{"code":-1}}));
                    return Ok(());
                }
                "Page.getFrameTree" => {
                    let frame = if (matches!(self.fault, Fault::WrongFrame) && self.committed)
                        || (matches!(self.fault, Fault::PostFrameReplacement)
                            && self
                                .trace
                                .borrow()
                                .iter()
                                .any(|v| v["method"] == "Runtime.evaluate"))
                    {
                        "old"
                    } else {
                        "f"
                    };
                    let mut loader = if self.committed { "l" } else { "blank" };
                    if let Fault::StartupNavigation(mode) = self.fault {
                        let second = self
                            .trace
                            .borrow()
                            .iter()
                            .filter(|v| v["method"] == "Page.getFrameTree")
                            .count()
                            == 2;
                        if !self.committed && (mode == 1 || second) {
                            loader = "B";
                        }
                        if second {
                            if mode == 3 {
                                loader = "blank";
                            }
                            if mode == 7 {
                                loader = "C";
                            }
                            if !matches!(mode, 1 | 3 | 11 | 13) {
                                let mut frame = json!({"id":"f","loaderId":if mode == 4 {"C"} else {"B"},"url":if mode == 5 {"http://ag9g.invalid/fixture.html"} else {"about:blank"}});
                                if mode == 6 {
                                    frame["parentId"] = "parent".into();
                                }
                                self.event("Page.frameNavigated", json!({"frame":frame}));
                                if mode == 9 {
                                    self.event("Page.frameStartedNavigating",json!({"frameId":"f","loaderId":"C","url":"about:blank","navigationType":"differentDocument"}));
                                }
                            }
                        }
                    }
                    let url = if self.committed {
                        "http://ag9g.invalid/fixture.html"
                    } else {
                        "about:blank"
                    };
                    result =
                        json!({"frameTree":{"frame":{"id":frame,"loaderId":loader,"url":url}}});
                }
                "Page.navigate" => {
                    self.nav = Some(id);
                    if matches!(self.fault, Fault::StartAfterAck) {
                        self.replies.push_back(json!({"id":self.nav.take().unwrap(),"sessionId":"s","result":{"frameId":"f","loaderId":"l"}}));
                    }
                    for _ in 0..if matches!(self.fault, Fault::RepeatStart) {
                        2
                    } else {
                        1
                    } {
                        self.event("Page.frameStartedNavigating",json!({"frameId":"f","loaderId":"l","url":"http://ag9g.invalid/fixture.html","navigationType":"differentDocument"}));
                    }
                    if matches!(self.fault, Fault::TargetCrash) {
                        self.event("Inspector.targetCrashed", json!({"targetId":"t"}));
                    }
                    if matches!(self.fault, Fault::WrongSession) {
                        self.replies.push_back(json!({"sessionId":"wrong","method":"Page.frameAttached","params":{"frameId":"child","parentFrameId":"f"}}));
                    }
                    if matches!(self.fault, Fault::DestroyedTarget) {
                        self.event("Target.targetDestroyed", json!({"targetId":"t"}));
                    }
                    if matches!(self.fault, Fault::NewTarget) {
                        self.event(
                            "Target.attachedToTarget",
                            json!({"targetInfo":{"targetId":"child"}}),
                        );
                    }
                    self.event("Network.requestWillBeSent",json!({"type":"Document","frameId":"f","loaderId":"l","requestId":"n","request":{"url":"http://ag9g.invalid/fixture.html","method":"GET"}}));
                    if matches!(self.fault, Fault::Ancillary) {
                        self.event("Fetch.requestPaused", json!({"resourceType":"Image","frameId":"f","networkId":"image","requestId":"image-fetch","request":{"url":"http://ag9g.invalid/image","method":"GET"}}));
                    }
                    self.event("Fetch.requestPaused",json!({"resourceType":"Document","frameId":"f","networkId":"n","requestId":"fetch","request":{"url":"http://ag9g.invalid/fixture.html","method":"GET"}}));
                    return Ok(());
                }
                "Fetch.fulfillRequest" => {
                    self.committed = true;
                    // Navigation may acknowledge before fulfillment; both are outstanding and separately bound.
                    if let Some(nav) = self.nav.take() {
                        self.replies.push_back(json!({"id":nav,"sessionId":"s","result":{"frameId":"f","loaderId":"l"}}));
                    }
                    let headers = json!({"Content-Type":"text/html; charset=utf-8","Content-Length":v["params"]["responseHeaders"][1]["value"],"Cache-Control":"no-store"});
                    self.event(
                        "Network.responseReceivedExtraInfo",
                        json!({"requestId":"n","statusCode":200,"headers":headers}),
                    );
                    self.event("Network.responseReceived",json!({"type":"Document","requestId":"n","frameId":"f","loaderId":"l","hasExtraInfo":true,"response":{"url":"http://ag9g.invalid/fixture.html","status":200,"mimeType":"text/html","fromDiskCache":false,"fromServiceWorker":false,"headers":headers}}));
                    self.event("Page.frameNavigated", json!({"frame":{"id":"f","loaderId":"l","url":"http://ag9g.invalid/fixture.html"}}));
                    self.event("Network.loadingFinished", json!({"requestId":"n"}));
                    for name in ["DOMContentLoaded", "load"] {
                        self.event(
                            "Page.lifecycleEvent",
                            json!({"frameId":"f","loaderId":"l","name":name}),
                        );
                    }
                    if matches!(self.fault, Fault::NegativeRedirect) {
                        self.event("Page.frameScheduledNavigation", json!({"frameId":"f","reason":"metaTagRefresh","url":"http://ag9g.invalid/redirected.html"}));
                    }
                    if matches!(self.fault, Fault::NegativeChild) {
                        self.event(
                            "Page.frameAttached",
                            json!({"frameId":"child","parentFrameId":"f"}),
                        );
                    }
                }
                "Page.createIsolatedWorld" => {
                    if matches!(self.fault, Fault::InspectionFailure) {
                        self.event("Network.loadingFailed", json!({"requestId":"n"}));
                    }
                    let default = matches!(self.fault, Fault::WrongRealm);
                    self.event("Runtime.executionContextCreated",json!({"context":{"id":42,"uniqueId":"unique","name":"ag9g-read-only-v1","auxData":{"frameId":"f","isDefault":default}}}));
                    result = json!({"executionContextId":42});
                }
                "Runtime.evaluate" => {
                    if matches!(self.fault, Fault::PostFailure) {
                        self.event("Network.loadingFailed", json!({"requestId":"n"}));
                    }
                    result = json!({"result":{"type":"string","value":"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n"}})
                }
                _ => (),
            }
            let mut reply = json!({"id":id,"result":result});
            if let Some(s) = v.get("sessionId") {
                reply["sessionId"] = s.clone();
            }
            self.replies.push_back(reply);
            Ok(())
        }
        fn receive_quiescent(&mut self) -> Result<Option<Vec<u8>>> {
            if let Fault::Late(event) = self.fault {
                self.fault = Fault::None;
                return Ok(Some(event.as_bytes().to_vec()));
            }
            self.replies
                .pop_front()
                .map(|v| serde_json::to_vec(&v).map_err(|_| E::Protocol))
                .transpose()
        }
        fn receive(&mut self, _: Instant) -> Result<Vec<u8>> {
            serde_json::to_vec(&self.replies.pop_front().ok_or(E::Completion)?)
                .map_err(|_| E::Protocol)
        }
    }
    fn execute(fault: Fault) -> (Result<CaptureOutcome>, Rc<RefCell<Vec<Value>>>) {
        let trace = Rc::new(RefCell::new(Vec::new()));
        let peer = Peer {
            replies: VecDeque::new(),
            trace: trace.clone(),
            nav: None,
            committed: false,
            fault,
        };
        let c = crate::configuration::specimen();
        let e = crate::packaging::specimen();
        (
            PreparedSession::prepare(peer, &c, crate::deadline::AttemptDeadline::new()).and_then(
                |mut s| {
                    let result = s.capture(&c, b"<!doctype html><p>exact\n", &e)?;
                    s.verify_quiescent_events()?;
                    Ok(result)
                },
            ),
            trace,
        )
    }
    #[test]
    fn independent_sessions_prepare_fresh_context_target_and_controls_after_failure() {
        let (failed, first_trace) = execute(Fault::InspectionFailure);
        assert!(failed.is_err());
        let (completed, second_trace) = execute(Fault::None);
        assert!(completed.is_ok());
        assert!(!Rc::ptr_eq(&first_trace, &second_trace));
        for trace in [first_trace, second_trace] {
            let trace = trace.borrow();
            for method in [
                "Target.createBrowserContext",
                "Target.createTarget",
                "Target.attachToTarget",
            ] {
                assert_eq!(trace.iter().filter(|v| v["method"] == method).count(), 1);
            }
            let navigation = trace
                .iter()
                .position(|v| v["method"] == "Page.navigate")
                .unwrap();
            assert!(
                trace[..navigation]
                    .iter()
                    .any(|v| v["method"] == "Emulation.setScriptExecutionDisabled"
                        && v["params"]["value"] == true)
            );
        }
    }

    #[test]
    fn lifecycle_replay_follows_baseline_and_precedes_fixture_navigation() {
        for fault in [
            Fault::None,
            Fault::StartupNavigation(0),
            Fault::StartupNavigation(1),
            Fault::StartupNavigation(11),
        ] {
            let (result, trace) = execute(fault);
            assert!(result.is_ok(), "{result:?}");
            let trace = trace.borrow();
            let position = |method| trace.iter().position(|v| v["method"] == method).unwrap();
            assert!(position("Page.enable") < position("Page.getFrameTree"));
            assert!(position("Page.getFrameTree") < position("Page.setLifecycleEventsEnabled"));
            assert!(position("Page.setLifecycleEventsEnabled") < position("Page.navigate"));
            assert_eq!(
                trace[..position("Page.navigate")]
                    .iter()
                    .filter(|v| v["method"] == "Page.getFrameTree")
                    .count(),
                2
            );
        }
    }
    #[test]
    fn lifecycle_contradictions_and_failed_enable_never_navigate() {
        for mode in [3, 12, 13, 14] {
            let (result, trace) = execute(Fault::StartupNavigation(mode));
            assert!(result.is_err(), "mode {mode}");
            assert!(
                !trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Page.navigate" || v["method"] == "Runtime.evaluate")
            );
        }
    }
    #[test]
    fn startup_pending_and_committed_loaders_converge_in_both_orders() {
        for fault in [
            Fault::None,
            Fault::StartupNavigation(0),
            Fault::StartupNavigation(1),
            Fault::StartupNavigation(2),
        ] {
            let (result, trace) = execute(fault);
            assert!(result.is_ok(), "{result:?}");
            assert!(
                trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Page.navigate")
            );
        }
    }
    #[test]
    fn unreconciled_startup_never_reaches_fixture_navigation() {
        for mode in 3..=10 {
            let (result, trace) = execute(Fault::StartupNavigation(mode));
            assert!(result.is_err(), "startup mode {mode}");
            assert!(
                !trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Page.navigate" || v["method"] == "Runtime.evaluate")
            );
        }
        let event = r#"{"sessionId":"s","method":"Page.frameStartedNavigating","params":{"frameId":"f","loaderId":"B","url":"http://ag9g.invalid/fixture.html","navigationType":"differentDocument"}}"#;
        let (result, trace) = execute(Fault::Inject("Page.enable", event));
        assert!(result.is_err());
        assert!(
            !trace
                .borrow()
                .iter()
                .any(|v| v["method"] == "Page.navigate")
        );
    }
    #[test]
    fn discovery_ledger_and_navigation_orderings_converge() {
        for fault in [
            Fault::None,
            Fault::DelayedDiscovery,
            Fault::StartAfterAck,
            Fault::RepeatStart,
            Fault::StartupStart,
        ] {
            let (result, trace) = execute(fault);
            assert!(result.is_ok(), "{result:?}");
            let trace = trace.borrow();
            let position = |method| trace.iter().position(|v| v["method"] == method).unwrap();
            assert!(position("Target.setDiscoverTargets") < position("Target.getTargets"));
            assert_eq!(
                trace
                    .iter()
                    .filter(|v| v["method"] == "Target.getTargets")
                    .count(),
                2
            );
        }
    }
    #[test]
    fn startup_history_cannot_be_erased_by_inventory() {
        for method in [
            "Target.targetDestroyed",
            "Target.targetCrashed",
            "Target.detachedFromTarget",
        ] {
            let event = match method {
                "Target.targetDestroyed" => {
                    r#"{"method":"Target.targetDestroyed","params":{"targetId":"initial"}}"#
                }
                "Target.targetCrashed" => {
                    r#"{"method":"Target.targetCrashed","params":{"targetId":"t"}}"#
                }
                _ => {
                    r#"{"method":"Target.detachedFromTarget","params":{"targetId":"t","sessionId":"s"}}"#
                }
            };
            let (result, trace) = execute(Fault::Inject("Target.getTargets", event));
            assert!(result.is_err());
            assert!(
                !trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Runtime.evaluate")
            );
        }
        for event in [
            r#"{"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"extra","type":"page","url":"about:blank","attached":false}}}"#,
            r#"{"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"worker","type":"worker","url":"","attached":false}}}"#,
            r#"{"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"t","type":"page","url":"about:blank","attached":true,"browserContextId":"wrong"}}}"#,
            r#"{"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"initial","type":"worker","url":"about:blank","attached":false}}}"#,
        ] {
            let (result, trace) = execute(Fault::Inject("Page.enable", event));
            assert!(result.is_err());
            assert!(
                !trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Page.navigate")
            );
        }
        assert!(execute(Fault::InventoryMismatch).0.is_err());
    }
    #[test]
    fn navigation_start_scope_and_identity_are_closed() {
        for scope in 0..3 {
            assert!(
                execute(Fault::Envelope("Page.frameStartedNavigating", scope))
                    .0
                    .is_err()
            );
        }
        for (key, value) in [
            ("frameId", "child"),
            ("url", "about:blank"),
            ("loaderId", "replacement"),
            ("navigationType", "sameDocument"),
        ] {
            assert!(
                execute(Fault::Collision("Page.frameStartedNavigating", key, value))
                    .0
                    .is_err()
            );
        }
        let late = r#"{"sessionId":"s","method":"Page.frameStartedNavigating","params":{"frameId":"f","loaderId":"l","url":"http://ag9g.invalid/fixture.html","navigationType":"differentDocument"}}"#;
        assert!(execute(Fault::Inject("Runtime.evaluate", late)).0.is_err());
        assert!(execute(Fault::Late(late)).0.is_err());
        let unrelated = r#"{"sessionId":"s","method":"Page.frameStartedNavigating","params":{"frameId":"f","loaderId":"other","url":"http://ag9g.invalid/redirected.html","navigationType":"differentDocument"}}"#;
        assert!(
            execute(Fault::Inject("Page.navigate", unrelated))
                .0
                .is_err()
        );
    }
    #[test]
    fn same_capture_core_fulfills_during_pending_navigation_and_only_inspects_isolated_world() {
        use base64::Engine;
        let (result, trace) = execute(Fault::None);
        let CaptureOutcome::Observation(output) = result.unwrap() else {
            panic!("unexpected rejection")
        };
        assert_eq!(output.realm, "unique");
        assert_eq!(output.document, "t:f:l");
        assert_eq!(output.denied_requests, 0);
        assert!(!output.bytes.is_empty());
        let trace = trace.borrow();
        let nav = trace
            .iter()
            .position(|v| v["method"] == "Page.navigate")
            .unwrap();
        let disable = trace
            .iter()
            .position(|v| v["method"] == "Emulation.setScriptExecutionDisabled")
            .unwrap();
        assert!(disable < nav);
        assert!(
            trace
                .iter()
                .position(|v| v["method"] == "Inspector.enable")
                .unwrap()
                < nav
        );
        for command in trace
            .iter()
            .filter(|v| v["method"] == "Emulation.setScriptExecutionDisabled")
        {
            assert_eq!(command["params"]["value"], true);
        }
        let inspect = trace
            .iter()
            .find(|v| v["method"] == "Runtime.evaluate")
            .unwrap();
        assert_eq!(inspect["params"]["uniqueContextId"], "unique");
        assert!(inspect["params"].get("contextId").is_none());
        assert_eq!(
            inspect["params"]["expression"].as_str().unwrap().as_bytes(),
            crate::packaging::specimen().bytes()
        );
        let fulfill = trace
            .iter()
            .find(|v| v["method"] == "Fetch.fulfillRequest")
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(fulfill["params"]["body"].as_str().unwrap())
                .unwrap(),
            b"<!doctype html><p>exact\n"
        );
    }
    #[test]
    fn startup_faults_never_navigate_inspect_or_satisfy_negative_vectors() {
        for event in [
            r#"{"sessionId":"s","method":"Inspector.targetCrashed","params":{}}"#,
            r#"{"sessionId":"s","method":"Inspector.detached","params":{"reason":"gone"}}"#,
            r#"{"sessionId":"s","method":"Inspector.targetReloadedAfterCrash","params":{}}"#,
            r#"{"method":"Target.targetDestroyed","params":{"targetId":"t"}}"#,
            r#"{"method":"Target.detachedFromTarget","params":{"sessionId":"s","targetId":"t"}}"#,
            r#"{"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"worker","type":"worker","url":"about:blank"}}}"#,
            r#"{"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"second","type":"page","url":"about:blank"}}}"#,
            r#"{"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"wrong","type":"page","url":"about:blank","browserContextId":"ctx"}}}"#,
            r#"{"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"t","type":"page","url":"about:blank","browserContextId":"wrong"}}}"#,
            r#"{"method":"Page.lifecycleEvent","params":{"frameId":"f","loaderId":"blank"}}"#,
            r#"{"sessionId":"wrong","method":"Page.lifecycleEvent","params":{"frameId":"f","loaderId":"blank"}}"#,
            r#"{"sessionId":"s","method":"Page.lifecycleEvent","params":{"frameId":"other","loaderId":"blank"}}"#,
        ] {
            let (result, trace) = execute(Fault::Inject("Inspector.enable", event));
            assert!(result.is_err(), "{event}");
            assert!(!trace.borrow().iter().any(|v| matches!(
                v["method"].as_str(),
                Some("Page.navigate" | "Runtime.evaluate")
            )));
        }
    }
    #[test]
    fn exact_session_required_in_all_attached_phases() {
        for method in [
            "Network.requestWillBeSent",
            "Fetch.requestPaused",
            "Page.lifecycleEvent",
            "Runtime.executionContextCreated",
        ] {
            for scope in 0..3 {
                let (result, trace) = execute(Fault::Envelope(method, scope));
                assert!(result.is_err(), "{method} {scope}");
                assert!(
                    !trace
                        .borrow()
                        .iter()
                        .any(|v| v["method"] == "Runtime.evaluate")
                );
            }
            assert!(execute(Fault::Envelope(method, 3)).0.is_ok());
        }
        for command in [
            "Page.navigate",
            "Page.createIsolatedWorld",
            "Runtime.evaluate",
        ] {
            for event in [
                r#"{"sessionId":"s","method":"Inspector.targetCrashed","params":{}}"#,
                r#"{"sessionId":"s","method":"Inspector.detached","params":{}}"#,
                r#"{"method":"Inspector.targetCrashed","params":{}}"#,
                r#"{"method":"Inspector.detached","params":{}}"#,
                r#"{"sessionId":null,"method":"Page.lifecycleEvent","params":{"frameId":"f","loaderId":"l","name":"load"}}"#,
            ] {
                assert!(execute(Fault::Inject(command, event)).0.is_err());
            }
        }
    }
    #[test]
    fn controlled_identity_collisions_cannot_be_unrelated_traffic() {
        for (method, field, value) in [
            ("Network.requestWillBeSent", "type", "XHR"),
            ("Network.requestWillBeSent", "frameId", "wrong"),
            ("Network.requestWillBeSent", "loaderId", "wrong"),
            ("Network.responseReceived", "type", "Image"),
            ("Fetch.requestPaused", "resourceType", "Image"),
            ("Page.lifecycleEvent", "loaderId", "wrong"),
        ] {
            assert!(
                execute(Fault::Collision(method, field, value)).0.is_err(),
                "{method} {field}"
            );
        }
        assert!(execute(Fault::Inject("Runtime.evaluate", r#"{"sessionId":"s","method":"Network.responseReceived","params":{"requestId":"n","type":"XHR"}}"#)).0.is_err());
        let CaptureOutcome::Observation(o) = execute(Fault::Ancillary).0.unwrap() else {
            panic!()
        };
        assert_eq!(o.denied_requests, 1);
    }
    #[test]
    fn only_correlated_policy_events_satisfy_negative_outcomes() {
        assert!(matches!(
            execute(Fault::NegativeRedirect).0,
            Ok(CaptureOutcome::Rejected(
                super::super::delivery::StaticDomPolicyRejection::MetaRefresh { .. }
            ))
        ));
        assert!(matches!(
            execute(Fault::NegativeChild).0,
            Ok(CaptureOutcome::Rejected(
                super::super::delivery::StaticDomPolicyRejection::ChildFrame { .. }
            ))
        ));
    }
    #[test]
    fn late_events_are_checked_before_stopped_population_teardown() {
        for event in [
            r#"{"sessionId":"s","method":"Inspector.targetCrashed","params":{}}"#,
            r#"{"sessionId":"s","method":"Inspector.detached","params":{}}"#,
            r#"{"method":"Network.loadingFinished","params":{"requestId":"n"}}"#,
            r#"{"sessionId":"s","method":"Page.lifecycleEvent","params":{"frameId":"f","loaderId":"wrong","name":"load"}}"#,
        ] {
            let (result, trace) = execute(Fault::Late(event));
            assert!(result.is_err());
            assert!(
                trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Runtime.evaluate")
            );
        }
    }
    #[test]
    fn protocol_and_post_observation_deadlines_discard_output() {
        for fault in [Fault::ProtocolDeadline, Fault::PostDeadline] {
            let (result, trace) = execute(fault);
            assert!(matches!(result, Err(E::Deadline)));
            let inspected = trace
                .borrow()
                .iter()
                .any(|v| v["method"] == "Runtime.evaluate");
            assert_eq!(inspected, matches!(fault, Fault::PostDeadline));
        }
    }
    #[test]
    fn wrong_realm_frame_new_target_and_control_failure_never_observe() {
        for (fault, error) in [
            (Fault::WrongRealm, E::RealmIdentity),
            (Fault::WrongFrame, E::DocumentIdentity),
            (Fault::NewTarget, E::UnexpectedEvent),
            (Fault::ScriptControl, E::ScriptingControl),
            (Fault::WrongSession, E::UnexpectedEvent),
            (Fault::DestroyedTarget, E::UnexpectedEvent),
            (Fault::Startup, E::UnexpectedEvent),
        ] {
            let (result, trace) = execute(fault);
            assert!(matches!(result,Err(e)if e==error));
            assert!(
                !trace
                    .borrow()
                    .iter()
                    .any(|v| v["method"] == "Runtime.evaluate")
            );
        }
    }

    #[test]
    fn transaction_disruptions_never_become_successful_recovery() {
        for (fault, expected) in [
            (Fault::DuplicateRequest, E::UnexpectedNavigation),
            (Fault::DuplicateFetch, E::UnexpectedNavigation),
            (Fault::ResponseReplacement, E::DocumentIdentity),
            (Fault::PostFrameReplacement, E::DocumentIdentity),
            (Fault::NetworkReplacement, E::DocumentIdentity),
            (Fault::LoaderReplacement, E::DocumentIdentity),
            (Fault::DuplicateResponse, E::DocumentIdentity),
            (Fault::DuplicateExtra, E::DocumentIdentity),
            (Fault::ExtraReplacement, E::DocumentIdentity),
            (Fault::MissingExtra, E::Completion),
            (Fault::LoadingFailure, E::Completion),
            (Fault::InspectionFailure, E::Completion),
            (Fault::PostFailure, E::Completion),
            (Fault::TargetCrash, E::Completion),
        ] {
            assert!(
                matches!(execute(fault).0, Err(e) if e == expected),
                "expected {expected:?}"
            );
        }
    }
    #[test]
    fn unchanged_transaction_identity_contains_no_network_service_lifetime() {
        // Two identical scripted transactions, not a real Chromium restart or
        // A/B collection. Process PIDs are deliberately not inputs to this API.
        let CaptureOutcome::Observation(a) = execute(Fault::None).0.unwrap() else {
            panic!()
        };
        let CaptureOutcome::Observation(b) = execute(Fault::None).0.unwrap() else {
            panic!()
        };
        assert_eq!(a.bytes, b.bytes);
        assert_eq!(a.document, b.document);
        assert_eq!(a.document, "t:f:l");
        assert_eq!(a.realm, b.realm);
    }
}

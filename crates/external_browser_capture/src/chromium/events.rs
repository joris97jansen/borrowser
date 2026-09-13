//! Flattened CDP event authority. Command responses are owned by Protocol.
use super::delivery::text;
use crate::{CaptureError as E, Result};
use serde_json::Value;
use std::cell::{Cell, RefCell};

pub(crate) enum ScopedEvent<'a> {
    Browser,
    Attached(&'a Value),
}

// Exactly two possible identities; no transient target is removed from history.
// Inventory observations never substitute for creation/attachment notifications.
#[derive(Default)]
struct StartupLedger {
    seen: Cell<u8>,
    private_attached: Cell<bool>,
    document: RefCell<Option<StartupDocument>>,
}

// Startup observations never contribute to controlled fixture completion.
enum StartupNavigation {
    None,
    Pending(String),
    Committed,
}
struct StartupDocument {
    committed: String,
    navigation: StartupNavigation,
    starts: usize,
}
impl StartupDocument {
    fn new(loader: &str) -> Self {
        Self {
            committed: loader.into(),
            navigation: StartupNavigation::None,
            starts: 0,
        }
    }
    fn start(&mut self, loader: String) -> Result<()> {
        match &self.navigation {
            StartupNavigation::None => self.navigation = StartupNavigation::Pending(loader),
            StartupNavigation::Pending(expected) if *expected == loader => {}
            _ => return Err(E::UnexpectedNavigation),
        }
        self.starts = self.starts.checked_add(1).ok_or(E::Limit)?;
        if self.starts > super::delivery::MAX_NAVIGATION_STARTS {
            return Err(E::Limit);
        }
        Ok(())
    }
    fn commit(&mut self, loader: String) -> Result<()> {
        if !matches!(&self.navigation, StartupNavigation::Pending(expected) if *expected == loader)
        {
            return Err(E::DocumentIdentity);
        }
        self.committed = loader;
        self.navigation = StartupNavigation::Committed;
        Ok(())
    }
    fn lifecycle(&self, loader: &str) -> Result<()> {
        if loader == self.committed
            || matches!(&self.navigation, StartupNavigation::Pending(expected) if expected == loader)
        {
            Ok(())
        } else {
            Err(E::DocumentIdentity)
        }
    }
}

pub(crate) struct EventBoundary {
    pub session: String,
    pub target: String,
    context: String,
    initial: String,
    initial_context: Option<String>,
    startup: StartupLedger,
    document_url: String,
}
impl EventBoundary {
    pub fn initial(v: &Value) -> Result<(String, Option<String>)> {
        let targets = v["targetInfos"].as_array().ok_or(E::Protocol)?;
        if targets.len() != 1 {
            return Err(E::UnexpectedEvent);
        }
        let info = &targets[0];
        if info["type"] != "page" || info["url"] != "about:blank" || info["attached"] != false {
            return Err(E::UnexpectedEvent);
        }
        Ok((
            text(info, "targetId")?,
            info.get("browserContextId")
                .map(|_| text(info, "browserContextId"))
                .transpose()?,
        ))
    }
    pub fn new(
        session: String,
        target: String,
        context: String,
        initial: (String, Option<String>),
        document_url: String,
    ) -> Result<Self> {
        if target == initial.0 {
            return Err(E::UnexpectedEvent);
        }
        Ok(Self {
            session,
            target,
            context,
            initial: initial.0,
            initial_context: initial.1,
            startup: StartupLedger::default(),
            document_url,
        })
    }
    fn info(&self, v: &Value, startup: bool) -> Result<()> {
        let id = text(v, "targetId")?;
        let expected = if id == self.target {
            Some(self.context.as_str())
        } else if id == self.initial {
            self.initial_context.as_deref()
        } else {
            return Err(E::UnexpectedEvent);
        };
        if v["type"] != "page"
            || v.get("browserContextId").map(Value::as_str) != expected.map(Some)
            || ((startup || id == self.initial) && v["url"] != "about:blank")
            || (!startup && id == self.target && v["url"] != self.document_url)
        {
            return Err(E::UnexpectedEvent);
        }
        let attached = v["attached"].as_bool().ok_or(E::Protocol)?;
        if id == self.initial && attached {
            return Err(E::UnexpectedEvent);
        }
        if id == self.target {
            if !attached && (!startup || self.startup.private_attached.get()) {
                return Err(E::UnexpectedEvent);
            }
            if attached {
                self.startup.private_attached.set(true);
            }
        }
        Ok(())
    }
    pub fn startup_complete(&self) -> bool {
        self.startup.seen.get() == 7 && self.startup.private_attached.get()
    }
    // Called after draining observed events following the final inventory/barrier.
    // Current inventory proves presence, while the ledger proves the observed
    // history: disappearance/transient unknown targets were already fatal.
    pub fn reconcile(&self, inventory: &Value) -> Result<()> {
        let infos = inventory["targetInfos"].as_array().ok_or(E::Protocol)?;
        if infos.len() != 2 || !self.startup_complete() {
            return Err(E::UnexpectedEvent);
        }
        let mut initial = false;
        let mut private = false;
        for info in infos {
            self.info(info, true)?;
            if info["targetId"] == self.initial {
                if initial {
                    return Err(E::UnexpectedEvent);
                }
                initial = true;
            } else if info["targetId"] == self.target {
                if private || info["attached"] != true {
                    return Err(E::UnexpectedEvent);
                }
                private = true;
            }
        }
        if !initial || !private {
            return Err(E::UnexpectedEvent);
        }
        Ok(())
    }
    fn once(&self, bit: u8) -> Result<()> {
        let seen = self.startup.seen.get();
        if seen & bit != 0 {
            return Err(E::UnexpectedEvent);
        }
        self.startup.seen.set(seen | bit);
        Ok(())
    }
    pub fn validate<'a>(&self, e: &'a Value, startup: bool) -> Result<ScopedEvent<'a>> {
        if !e.is_object() || e.get("id").is_some() || !e["params"].is_object() {
            return Err(E::Protocol);
        }
        let method = e["method"].as_str().ok_or(E::Protocol)?;
        if method.starts_with("Target.") {
            // Root discovery / explicit flattened attachment only. Session auto-attached
            // descendants are outside the selected one-page population.
            if e.get("sessionId").is_some() {
                return Err(E::UnexpectedEvent);
            }
            let p = &e["params"];
            match method {
                "Target.targetCreated" if startup => {
                    self.info(&p["targetInfo"], true)?;
                    let bit = if p["targetInfo"]["targetId"] == self.target {
                        1
                    } else {
                        2
                    };
                    self.once(bit)?;
                }
                "Target.targetInfoChanged" => self.info(&p["targetInfo"], startup)?,
                "Target.attachedToTarget" if startup => {
                    self.once(4)?;
                    self.info(&p["targetInfo"], true)?;
                    if p["targetInfo"]["targetId"] != self.target
                        || p["sessionId"] != self.session
                        || p["waitingForDebugger"] != false
                    {
                        return Err(E::UnexpectedEvent);
                    }
                }
                // No permitted target/session may disappear, even at startup.
                "Target.targetCrashed" => return Err(E::Completion),
                "Target.targetDestroyed" | "Target.detachedFromTarget" => {
                    return Err(E::UnexpectedEvent);
                }
                _ => return Err(E::UnexpectedEvent),
            }
            return Ok(ScopedEvent::Browser);
        }
        let domain = method.split_once('.').ok_or(E::Protocol)?.0;
        if !matches!(
            domain,
            "Page" | "Runtime" | "Network" | "Fetch" | "Inspector"
        ) || e.get("sessionId").and_then(Value::as_str) != Some(self.session.as_str())
        {
            return Err(E::UnexpectedEvent);
        }
        if matches!(
            method,
            "Inspector.detached" | "Inspector.targetCrashed" | "Inspector.targetReloadedAfterCrash"
        ) {
            return Err(E::Completion);
        }
        Ok(ScopedEvent::Attached(e))
    }
    pub fn startup_barrier(&self, tree: &Value, frame: &str, original_loader: &str) -> Result<()> {
        let state = self.startup.document.borrow();
        let committed = state
            .as_ref()
            .map_or(original_loader, |d| d.committed.as_str());
        if state
            .as_ref()
            .is_some_and(|d| matches!(d.navigation, StartupNavigation::Pending(_)))
        {
            return Err(E::Completion);
        }
        let f = &tree["frameTree"]["frame"];
        if f["id"] != frame
            || f["url"] != "about:blank"
            || f["loaderId"] != committed
            || f.get("parentId").is_some()
            || tree["frameTree"].get("childFrames").is_some()
        {
            return Err(E::DocumentIdentity);
        }
        Ok(())
    }
    pub fn startup(&self, e: &Value, frame: &str, loader: &str) -> Result<()> {
        let ScopedEvent::Attached(e) = self.validate(e, true)? else {
            return Ok(());
        };
        let p = &e["params"];
        let mut state = self.startup.document.borrow_mut();
        let document = state.get_or_insert_with(|| StartupDocument::new(loader));
        match e["method"].as_str() {
            Some("Runtime.executionContextCreated") => {
                text(&p["context"], "uniqueId")?;
                if p["context"]["auxData"]["frameId"] != frame
                    || p["context"]["auxData"]["isDefault"] != true
                {
                    return Err(E::RealmIdentity);
                }
            }
            Some("Page.frameStartedNavigating") => {
                let pending = super::delivery::navigation_start(p, frame, "about:blank")?;
                document.start(pending)?;
            }
            Some("Page.frameNavigated") => {
                let committed = &p["frame"];
                if committed["id"] != frame
                    || committed["url"] != "about:blank"
                    || committed.get("parentId").is_some()
                {
                    return Err(E::DocumentIdentity);
                }
                document.commit(text(committed, "loaderId")?)?;
            }
            Some("Page.lifecycleEvent") => {
                if p["frameId"] != frame {
                    return Err(E::DocumentIdentity);
                }
                document.lifecycle(&text(p, "loaderId")?)?;
            }
            Some("Page.frameStoppedLoading" | "Page.frameStartedLoading") => {
                if p["frameId"] != frame {
                    return Err(E::DocumentIdentity);
                }
            }
            Some("Page.loadEventFired" | "Page.domContentEventFired") => {}
            _ => return Err(E::UnexpectedEvent),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn boundary() -> EventBoundary {
        EventBoundary::new(
            "s".into(),
            "t".into(),
            "ctx".into(),
            ("initial".into(), None),
            "http://ag9g.invalid/fixture.html".into(),
        )
        .unwrap()
    }
    #[test]
    fn startup_lifecycle_tracks_only_committed_or_pending_identity() {
        let mut d = StartupDocument::new("A");
        d.start("B".into()).unwrap();
        d.lifecycle("A").unwrap();
        d.lifecycle("B").unwrap();
        assert!(d.lifecycle("C").is_err());
        d.commit("B".into()).unwrap();
        assert!(d.lifecycle("A").is_err());
        d.lifecycle("B").unwrap();
        assert!(d.commit("B".into()).is_err());
        assert!(d.start("B".into()).is_err());
    }
    #[test]
    fn startup_commit_requires_pending_main_blank_document() {
        for frame in [
            json!({"id":"child","loaderId":"B","url":"about:blank"}),
            json!({"id":"f","loaderId":"B","url":"about:blank","parentId":"parent"}),
            json!({"id":"f","loaderId":"C","url":"about:blank"}),
            json!({"id":"f","loaderId":"B","url":"http://ag9g.invalid/fixture.html"}),
        ] {
            let b = boundary();
            b.startup(&json!({"sessionId":"s","method":"Page.frameStartedNavigating","params":{"frameId":"f","loaderId":"B","url":"about:blank","navigationType":"differentDocument"}}),"f","A").unwrap();
            assert!(b.startup(&json!({"sessionId":"s","method":"Page.frameNavigated","params":{"frame":frame}}),"f","A").is_err());
        }
        assert!(boundary().startup(&json!({"sessionId":"s","method":"Page.frameNavigated","params":{"frame":{"id":"f","loaderId":"A","url":"about:blank"}}}),"f","A").is_err());
    }
    #[test]
    fn startup_navigation_is_bounded_and_exact() {
        let b = boundary();
        let e = json!({"sessionId":"s","method":"Page.frameStartedNavigating","params":{"frameId":"f","loaderId":"blank","url":"about:blank","navigationType":"differentDocument"}});
        for _ in 0..super::super::delivery::MAX_NAVIGATION_STARTS {
            assert!(b.startup(&e, "f", "blank").is_ok());
        }
        assert!(matches!(b.startup(&e, "f", "blank"), Err(E::Limit)));
        for (key, value) in [
            ("frameId", "child"),
            ("url", "http://ag9g.invalid/fixture.html"),
            ("navigationType", "sameDocument"),
        ] {
            let mut bad = e.clone();
            bad["params"][key] = value.into();
            assert!(boundary().startup(&bad, "f", "blank").is_err());
        }
    }
    #[test]
    fn startup_population_is_exact_not_url_based() {
        let good = json!({"targetInfos":[{"targetId":"initial","type":"page","url":"about:blank","attached":false}]});
        assert!(EventBoundary::initial(&good).is_ok());
        for count in [0, 2] {
            let mut bad = good.clone();
            bad["targetInfos"] = json!(vec![good["targetInfos"][0].clone(); count]);
            assert!(EventBoundary::initial(&bad).is_err());
        }
        let b = boundary();
        let initial =
            json!({"method":"Target.targetCreated","params":{"targetInfo":good["targetInfos"][0]}});
        assert!(b.startup(&initial, "f", "blank").is_ok());
        assert!(b.startup(&initial, "f", "blank").is_err());
        let private = json!({"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"t","type":"page","url":"about:blank","browserContextId":"ctx","attached":false}}});
        assert!(b.startup(&private, "f", "blank").is_ok());
        let mut attached = private["params"]["targetInfo"].clone();
        attached["attached"] = true.into();
        let attach = json!({"method":"Target.attachedToTarget","params":{"sessionId":"s","waitingForDebugger":false,"targetInfo":attached}});
        assert!(b.startup(&attach, "f", "blank").is_ok());
        assert!(b.startup(&attach, "f", "blank").is_err());
        for key in ["sessionId", "waitingForDebugger"] {
            let mut bad = attach.clone();
            bad["params"].as_object_mut().unwrap().remove(key);
            assert!(boundary().startup(&bad, "f", "blank").is_err());
        }
        for key in ["targetId", "browserContextId"] {
            let mut bad = private.clone();
            bad["params"]["targetInfo"][key] = "wrong".into();
            assert!(boundary().startup(&bad, "f", "blank").is_err());
        }
    }
    #[test]
    fn malformed_envelopes_and_unknown_methods_are_not_startup_traffic() {
        let b = boundary();
        for e in [
            json!(null),
            json!({"method":"Page.lifecycleEvent","params":null}),
            json!({"sessionId":"s","method":"Inspector.unknown","params":{}}),
            json!({"method":"Target.unknown","params":{}}),
            json!({"sessionId":"s","method":"Page.frameAttached","params":{"frameId":"child","parentFrameId":"f"}}),
        ] {
            assert!(b.startup(&e, "f", "blank").is_err());
        }
    }
}

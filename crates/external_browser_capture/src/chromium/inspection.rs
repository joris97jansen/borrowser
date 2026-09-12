use super::{
    delivery::{DocumentState, text},
    events::{EventBoundary, ScopedEvent},
    protocol::{Protocol, Transport},
};
use crate::{CaptureError as E, Result, limits::ARTIFACT_BYTES, packaging::InspectorExpressionV1};
use external_test_provenance::validate_web_observable_dom_tree_v1;
use serde_json::{Value, json};

pub(crate) fn inspect<T: Transport>(
    p: &mut Protocol<T>,
    events: &EventBoundary,
    d: &DocumentState,
    expression: &InspectorExpressionV1,
) -> Result<(Vec<u8>, String)> {
    let session = events.session.as_str();
    if !d.complete() {
        return Err(E::Completion);
    }
    let world = p.call(
        Some(session),
        "Page.createIsolatedWorld",
        json!({"frameId":d.frame,"worldName":"ag9g-read-only-v1","grantUniveralAccess":false}),
    )?;
    let context = world["executionContextId"]
        .as_u64()
        .ok_or(E::RealmIdentity)?;
    let mut unique = None;
    // Runtime.enable is established before navigation. Match the created world event, never the default world.
    loop {
        let e = match p.queued() {
            Some(e) => e,
            None if unique.is_some() => break,
            None => p.event()?,
        };
        let ScopedEvent::Attached(e) = events.validate(&e, false)? else {
            continue;
        };
        if e["method"] == "Runtime.executionContextCreated" {
            let c = &e["params"]["context"];
            if c["id"].as_u64() == Some(context) {
                if c["name"] != "ag9g-read-only-v1"
                    || c["auxData"]["frameId"] != d.frame
                    || c["auxData"]["isDefault"] != false
                {
                    return Err(E::RealmIdentity);
                }
                if unique.replace(text(c, "uniqueId")?).is_some() {
                    return Err(E::RealmIdentity);
                }
            } else {
                return Err(E::RealmIdentity);
            }
        } else if !benign(e) {
            return Err(E::UnexpectedEvent);
        } else {
            verify_event_binding(e, d)?;
        }
    }
    let unique = unique.ok_or(E::RealmIdentity)?;
    let result=p.call(Some(session),"Runtime.evaluate",json!({"expression":expression.text(),"uniqueContextId":unique,
        "returnByValue":true,"awaitPromise":false,"includeCommandLineAPI":false,"userGesture":false,"timeout":30000}))?;
    if result.get("exceptionDetails").is_some() || result["result"]["type"] != "string" {
        return Err(E::Inspection);
    }
    let output = result["result"]["value"].as_str().ok_or(E::Inspection)?;
    if output.len() > ARTIFACT_BYTES {
        return Err(E::Limit);
    }
    validate_web_observable_dom_tree_v1(output.as_bytes()).map_err(|_| E::Artifact)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(output.len())
        .map_err(|_| E::Allocation)?;
    bytes.extend_from_slice(output.as_bytes());
    Ok((bytes, unique))
}
pub(crate) fn verify_event_binding(e: &Value, d: &DocumentState) -> Result<()> {
    d.collisions(e)?;
    let params = &e["params"];
    if matches!(
        e["method"].as_str(),
        Some("Network.loadingFailed" | "Network.loadingFinished")
    ) && params["requestId"].as_str() == d.network.as_deref()
    {
        // The controlled request already finished before the observation barrier.
        // A failure or repeated finish is a transaction contradiction.
        return Err(E::Completion);
    }
    if params.get("frameId").is_some_and(|f| f != &d.frame)
        || params
            .get("loaderId")
            .is_some_and(|l| Some(l.as_str().unwrap_or("")) != d.loader.as_deref())
    {
        return Err(E::DocumentIdentity);
    }
    Ok(())
}
pub(crate) fn benign(e: &Value) -> bool {
    matches!(
        e["method"].as_str(),
        Some(
            "Page.lifecycleEvent"
                | "Page.loadEventFired"
                | "Page.domContentEventFired"
                | "Page.frameStoppedLoading"
                | "Network.dataReceived"
                | "Network.loadingFinished"
                | "Network.loadingFailed"
                | "Network.requestWillBeSentExtraInfo"
        )
    )
}

use std::sync::mpsc::Sender;

use bus::CoreEvent;
use core_types::{DomHandle, DomVersion, RequestId, TabId};
use html::DomPatch;
use log::error;
#[cfg(feature = "patch-stats")]
use log::info;

pub(crate) fn emit_patch_update(
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
    dom_handle: DomHandle,
    version: &mut DomVersion,
    patches: Vec<DomPatch>,
) -> Result<(), std::sync::mpsc::SendError<CoreEvent>> {
    let from = *version;
    let to = from.next();
    let send_result = evt_tx.send(CoreEvent::DomPatchUpdate {
        tab_id,
        request_id,
        handle: dom_handle,
        from,
        to,
        patches,
    });
    if let Err(err) = send_result {
        error!(
            target: "runtime_parse",
            "patch sink dropped; stopping updates for tab={tab_id:?} request={request_id:?}"
        );
        return Err(err);
    }
    *version = to;
    Ok(())
}

pub(crate) fn estimate_patch_bytes(patch: &DomPatch) -> usize {
    const PATCH_OVERHEAD: usize = 8;
    match patch {
        DomPatch::Clear => PATCH_OVERHEAD,
        DomPatch::CreateDocument { doctype, .. } => {
            PATCH_OVERHEAD + doctype.as_ref().map(|s| s.len()).unwrap_or(0)
        }
        DomPatch::CreateElement {
            name, attributes, ..
        } => {
            let mut total = PATCH_OVERHEAD + name.len();
            for (k, v) in attributes {
                total += k.len();
                if let Some(value) = v {
                    total += value.len();
                }
            }
            total
        }
        DomPatch::CreateText { text, .. } | DomPatch::CreateComment { text, .. } => {
            PATCH_OVERHEAD + text.len()
        }
        DomPatch::AppendChild { .. }
        | DomPatch::InsertBefore { .. }
        | DomPatch::RemoveNode { .. }
        | DomPatch::SetAttributes { .. }
        | DomPatch::SetText { .. } => PATCH_OVERHEAD,
        DomPatch::AppendText { text, .. } => PATCH_OVERHEAD + text.len(),
        _ => PATCH_OVERHEAD,
    }
}

pub(crate) fn estimate_patch_bytes_slice(patches: &[DomPatch]) -> usize {
    patches.iter().fold(0usize, |total, patch| {
        total.saturating_add(estimate_patch_bytes(patch))
    })
}

#[cfg(feature = "patch-stats")]
pub(crate) fn log_patch_stats(tab_id: TabId, request_id: RequestId, patches: &[DomPatch]) {
    let mut created = 0usize;
    let mut removed = 0usize;
    for patch in patches {
        match patch {
            DomPatch::CreateDocument { .. }
            | DomPatch::CreateElement { .. }
            | DomPatch::CreateText { .. }
            | DomPatch::CreateComment { .. } => {
                created += 1;
            }
            DomPatch::RemoveNode { .. } => {
                removed += 1;
            }
            _ => {}
        }
    }
    info!(
        target: "runtime_parse",
        "patch_stats tab={tab_id:?} request={request_id:?} patches={} created={} removed={}",
        patches.len(),
        created,
        removed
    );
}

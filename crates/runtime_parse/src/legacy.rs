use std::sync::mpsc::Sender;
use std::time::Instant;

use bus::CoreEvent;
use core_types::{RequestId, TabId};
use html::DomPatch;
use log::error;

use crate::patching::{emit_patch_update, estimate_patch_bytes_slice};
use crate::policy::{PreviewPolicy, maybe_log_large_buffer};
use crate::state::HtmlState;

impl HtmlState {
    pub(crate) fn drain_tokens_into_builder(&mut self) {
        if self.token_buffer.is_empty() {
            return;
        }
        let atoms = self.tokenizer.atoms();
        let drained = std::mem::take(&mut self.token_buffer);
        for token in drained {
            if let Err(err) = self.builder.push_token(&token, atoms, &self.tokenizer) {
                error!(target: "runtime_parse", "tree builder error: {err}");
                self.failed = true;
                self.patch_buffer.clear();
                self.reset_pending();
                break;
            }
        }
        if self.failed {
            let _ = self.builder.take_patches();
            return;
        }
        let new_patches = self.builder.take_patches();
        if !new_patches.is_empty() {
            self.pending_patch_bytes = self
                .pending_patch_bytes
                .saturating_add(estimate_patch_bytes_slice(&new_patches));
            self.patch_buffer.extend(new_patches);
            self.update_patch_buffer_max();
        }
    }

    pub(crate) fn flush_patch_buffer(
        &mut self,
        evt_tx: &Sender<CoreEvent>,
        tab_id: TabId,
        request_id: RequestId,
    ) {
        if self.patch_buffer.is_empty() {
            return;
        }
        let patches = std::mem::replace(
            &mut self.patch_buffer,
            Vec::with_capacity(self.patch_buffer_retain),
        );

        #[cfg(feature = "patch-stats")]
        crate::patching::log_patch_stats(tab_id, request_id, &patches);

        let ok = emit_patch_update(
            evt_tx,
            tab_id,
            request_id,
            self.dom_handle,
            &mut self.version,
            patches,
        )
        .is_ok();
        self.reset_pending();
        if !ok {
            self.failed = true;
        }
    }

    pub(crate) fn update_patch_buffer_max(&mut self) {
        let len = self.patch_buffer.len();
        if len > self.max_patch_buffer_len {
            self.max_patch_buffer_len = len;
        }
        if self.pending_patch_bytes > self.max_patch_buffer_bytes {
            self.max_patch_buffer_bytes = self.pending_patch_bytes;
        }
    }

    pub(crate) fn reset_pending(&mut self) {
        self.pending_bytes = 0;
        self.pending_tokens = 0;
        self.pending_patch_bytes = 0;
    }
}

pub(crate) fn handle_legacy_chunk(
    st: &mut HtmlState,
    bytes: &[u8],
    policy: &PreviewPolicy,
    now: Instant,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
) -> bool {
    if st.failed {
        return true;
    }

    st.total_bytes = st.total_bytes.saturating_add(bytes.len());
    st.pending_bytes = st.pending_bytes.saturating_add(bytes.len());
    st.pending_tokens = st.pending_tokens.saturating_add(st.tokenizer.feed(bytes));
    // Always drain tokenizer tokens immediately so flush decisions reflect patch backlog.
    st.tokenizer.drain_into(&mut st.token_buffer);
    st.drain_tokens_into_builder();

    if policy.should_flush(
        now.saturating_duration_since(st.last_emit),
        st.pending_tokens,
        st.pending_bytes,
        st.patch_buffer.len(),
        st.pending_patch_bytes,
    ) {
        st.last_emit = now;
        maybe_log_large_buffer(st.total_bytes, &mut st.logged_large_buffer);
        st.flush_patch_buffer(evt_tx, tab_id, request_id);
        if st.failed {
            return true;
        }
    }

    false
}

pub(crate) fn handle_legacy_done(
    mut st: Box<HtmlState>,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
) {
    if st.failed {
        return;
    }
    st.pending_tokens = st.pending_tokens.saturating_add(st.tokenizer.finish());
    st.tokenizer.drain_into(&mut st.token_buffer);
    st.drain_tokens_into_builder();
    if let Err(err) = st.builder.finish() {
        error!(target: "runtime_parse", "tree builder finish error: {err}");
    }
    let final_patches: Vec<DomPatch> = st.builder.take_patches();
    if !final_patches.is_empty() {
        st.pending_patch_bytes = st
            .pending_patch_bytes
            .saturating_add(estimate_patch_bytes_slice(&final_patches));
        st.patch_buffer.extend(final_patches);
        st.update_patch_buffer_max();
    }
    st.flush_patch_buffer(evt_tx, tab_id, request_id);
}

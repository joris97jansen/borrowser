use std::sync::mpsc::Sender;
use std::time::Instant;

use bus::CoreEvent;
use core_types::{RequestId, TabId};
use html::html5::Html5SessionError;
use log::error;

use crate::patching::{emit_patch_update, estimate_patch_bytes_slice};
use crate::policy::{PreviewPolicy, maybe_log_large_buffer};
use crate::state::Html5State;

pub(crate) fn log_html5_session_error(
    tab_id: TabId,
    request_id: RequestId,
    err: Html5SessionError,
) {
    match err {
        Html5SessionError::Decode => {
            error!(
                target: "runtime_parse",
                "html5 decode error tab={tab_id:?} request={request_id:?}"
            );
        }
        Html5SessionError::Invariant => {
            error!(
                target: "runtime_parse",
                "html5 invariant error tab={tab_id:?} request={request_id:?}"
            );
        }
    }
}

impl Html5State {
    pub(crate) fn drain_patches(&mut self) {
        let new_patches = self.session.take_patches();
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

    pub(crate) fn update_pending_tokens(&mut self) {
        let total = self.session.tokens_processed();
        let delta = total.saturating_sub(self.last_tokens_processed);
        self.last_tokens_processed = total;
        self.pending_tokens = self.pending_tokens.saturating_add(delta as usize);
    }
}

pub(crate) fn handle_html5_chunk(
    st: &mut Html5State,
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
    if let Err(err) = st.session.push_bytes(bytes) {
        log_html5_session_error(tab_id, request_id, err);
        st.failed = true;
        st.reset_pending();
    } else if let Err(err) = st.session.pump() {
        log_html5_session_error(tab_id, request_id, err);
        st.failed = true;
        st.reset_pending();
    } else {
        st.update_pending_tokens();
        st.drain_patches();
    }

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

pub(crate) fn handle_html5_done(
    mut st: Box<Html5State>,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
) {
    if st.failed {
        st.flush_patch_buffer(evt_tx, tab_id, request_id);
        return;
    }
    if let Err(err) = st.session.pump() {
        log_html5_session_error(tab_id, request_id, err);
        if matches!(err, Html5SessionError::Decode) {
            st.update_pending_tokens();
            st.drain_patches();
            st.flush_patch_buffer(evt_tx, tab_id, request_id);
        }
        st.failed = true;
        st.reset_pending();
        return;
    }
    st.update_pending_tokens();
    st.drain_patches();
    st.flush_patch_buffer(evt_tx, tab_id, request_id);
}

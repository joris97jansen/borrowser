use std::sync::mpsc::Sender;
use std::time::Instant;

use bus::CoreEvent;
use core_types::{RequestId, TabId};
use html::HtmlParseError;
use log::error;

use crate::patching::{emit_patch_update, emit_publication_failure, estimate_patch_bytes_slice};
use crate::policy::{PreviewPolicy, maybe_log_large_buffer};
use crate::state::{PRESELECTION_BYTE_LIMIT, PRESELECTION_PATCH_LIMIT, RuntimeState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeParseFailure {
    Parser(HtmlParseError),
    PreSelectionBudgetExceeded,
    DocumentModeUnavailable,
    DocumentModeChanged {
        expected: html::DocumentMode,
        actual: html::DocumentMode,
    },
}

impl From<HtmlParseError> for RuntimeParseFailure {
    fn from(value: HtmlParseError) -> Self {
        Self::Parser(value)
    }
}

pub(crate) fn parser_error_discards_unpublished(error: &HtmlParseError) -> bool {
    matches!(error, HtmlParseError::Fatal(_))
}

pub(crate) fn parser_error_drains_on_completion(error: &HtmlParseError) -> bool {
    matches!(error, HtmlParseError::Decode)
}

fn log_runtime_parse_error(tab_id: TabId, request_id: RequestId, err: &HtmlParseError) {
    match err {
        HtmlParseError::Decode => {
            error!(
                target: "runtime_parse",
                "runtime parse decode error tab={tab_id:?} request={request_id:?}"
            );
        }
        HtmlParseError::DocumentModeUnavailable => {
            error!(target: "runtime_parse", "document mode unavailable tab={tab_id:?} request={request_id:?}");
        }
        HtmlParseError::Fatal(error) => {
            error!(
                target: "runtime_parse",
                "runtime parse fatal error tab={tab_id:?} request={request_id:?}: {error}"
            );
        }
        HtmlParseError::PatchValidation(detail) => {
            error!(
                target: "runtime_parse",
                "runtime parse patch validation error tab={tab_id:?} request={request_id:?}: {detail}"
            );
        }
    }
}

impl RuntimeState {
    pub(crate) fn drain_patches(&mut self) -> Result<(), RuntimeParseFailure> {
        let new_patches = self
            .parser
            .take_patches()
            .map_err(RuntimeParseFailure::from)?;

        // Refresh parser readiness before classifying this drain as
        // pre-selection state. A single parser pump may select the mode and
        // emit a large patch batch, even when RuntimeState had not observed
        // that selection yet.
        let observed = self
            .parser
            .selected_document_mode()
            .map_err(RuntimeParseFailure::from)?;
        if let Some(actual) = observed {
            match self.document_mode {
                None => self.document_mode = Some(actual),
                Some(expected) if expected == actual => {}
                Some(expected) => {
                    return Err(RuntimeParseFailure::DocumentModeChanged { expected, actual });
                }
            }
        }

        if !new_patches.is_empty() {
            let added_bytes = estimate_patch_bytes_slice(&new_patches);
            if self.document_mode.is_none()
                && (self.patch_buffer.len().saturating_add(new_patches.len())
                    > PRESELECTION_PATCH_LIMIT
                    || self.pending_patch_bytes.saturating_add(added_bytes)
                        > PRESELECTION_BYTE_LIMIT)
            {
                return Err(RuntimeParseFailure::PreSelectionBudgetExceeded);
            }
            self.pending_patch_bytes = self.pending_patch_bytes.saturating_add(added_bytes);
            self.patch_buffer.extend(new_patches);
            self.update_patch_buffer_max();
        }
        Ok(())
    }

    pub(crate) fn flush_patch_buffer(
        &mut self,
        evt_tx: &Sender<CoreEvent>,
        tab_id: TabId,
        request_id: RequestId,
    ) -> Result<(), RuntimeParseFailure> {
        if self.patch_buffer.is_empty() {
            return Ok(());
        }
        let Some(document_mode) = self.document_mode else {
            return Err(RuntimeParseFailure::DocumentModeUnavailable);
        };
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
            document_mode,
            &mut self.version,
            patches,
        )
        .is_ok();
        self.reset_pending();
        if !ok {
            self.failed = true;
        }
        Ok(())
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
        let total = self.parser.tokens_processed();
        let delta = total.saturating_sub(self.last_tokens_processed);
        self.last_tokens_processed = total;
        self.pending_tokens = self.pending_tokens.saturating_add(delta as usize);
    }

    pub(crate) fn discard_unpublished_after_parser_fatal(&mut self) {
        self.patch_buffer.clear();
        self.failed = true;
        self.reset_pending();
    }
}

fn handle_chunk_error(
    st: &mut RuntimeState,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
    err: RuntimeParseFailure,
) -> bool {
    match &err {
        RuntimeParseFailure::Parser(parser_error) => {
            log_runtime_parse_error(tab_id, request_id, parser_error);
            if parser_error_discards_unpublished(parser_error) {
                st.discard_unpublished_after_parser_fatal();
                return true;
            }
        }
        RuntimeParseFailure::PreSelectionBudgetExceeded => {
            let _ = emit_publication_failure(
                evt_tx,
                tab_id,
                request_id,
                Some(st.dom_handle),
                bus::DocumentPublicationFailure::PreSelectionBudgetExceeded,
            );
        }
        RuntimeParseFailure::DocumentModeUnavailable => {
            let _ = emit_publication_failure(
                evt_tx,
                tab_id,
                request_id,
                Some(st.dom_handle),
                bus::DocumentPublicationFailure::DocumentModeUnavailable,
            );
        }
        RuntimeParseFailure::DocumentModeChanged { .. } => {
            let _ = emit_publication_failure(
                evt_tx,
                tab_id,
                request_id,
                Some(st.dom_handle),
                bus::DocumentPublicationFailure::DocumentModeChanged,
            );
        }
    }
    st.failed = true;
    st.reset_pending();
    false
}

pub(crate) fn handle_runtime_chunk(
    st: &mut RuntimeState,
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
    if let Err(err) = st.parser.push_bytes(bytes) {
        handle_chunk_error(st, evt_tx, tab_id, request_id, err.into());
        return st.failed;
    } else if let Err(err) = st.parser.pump() {
        handle_chunk_error(st, evt_tx, tab_id, request_id, err.into());
        return st.failed;
    } else if let Err(err) = st.drain_patches() {
        handle_chunk_error(st, evt_tx, tab_id, request_id, err);
        return st.failed;
    } else {
        st.update_pending_tokens();
    }

    if st.document_mode.is_some()
        && policy.should_flush(
            now.saturating_duration_since(st.last_emit),
            st.pending_tokens,
            st.pending_bytes,
            st.patch_buffer.len(),
            st.pending_patch_bytes,
        )
    {
        st.last_emit = now;
        maybe_log_large_buffer(st.total_bytes, &mut st.logged_large_buffer);
        if st.flush_patch_buffer(evt_tx, tab_id, request_id).is_err() {
            st.failed = true;
        }
        if st.failed {
            return true;
        }
    }

    false
}

pub(crate) fn handle_runtime_done(
    mut st: Box<RuntimeState>,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
) {
    if st.failed {
        return;
    }
    if let Err(err) = st.parser.finish() {
        log_runtime_parse_error(tab_id, request_id, &err);
        if parser_error_discards_unpublished(&err) {
            st.discard_unpublished_after_parser_fatal();
            return;
        }
        if parser_error_drains_on_completion(&err) {
            st.update_pending_tokens();
            if let Err(failure) = st.drain_patches() {
                let _ = handle_chunk_error(&mut st, evt_tx, tab_id, request_id, failure);
                return;
            }
            if st.flush_patch_buffer(evt_tx, tab_id, request_id).is_err() {
                st.failed = true;
            }
        }
        st.failed = true;
        st.reset_pending();
        return;
    }
    st.update_pending_tokens();
    if let Err(err) = st.drain_patches() {
        let _ = handle_chunk_error(&mut st, evt_tx, tab_id, request_id, err);
        return;
    }
    if st.document_mode.is_none() {
        let _ = handle_chunk_error(
            &mut st,
            evt_tx,
            tab_id,
            request_id,
            RuntimeParseFailure::DocumentModeUnavailable,
        );
        return;
    }
    if st.flush_patch_buffer(evt_tx, tab_id, request_id).is_err() {
        st.failed = true;
    }
}

use super::Tab;
use super::state::{
    CssRuntimeProgress, DocumentParseStatus, StylesheetLoadPhase, StylesheetLoadState,
};
use super::status::format_network_error;
use bus::CoreCommand;
use core_types::{NetworkErrorKind, NetworkResponseInfo, RequestId, StylesheetSlotId};

impl Tab {
    fn accepts_css_network_event(&self, slot: StylesheetSlotId, request: RequestId) -> bool {
        self.document_load.parse_status != DocumentParseStatus::Failed
            && self.page.has_external_stylesheet_slot(slot)
            && self
                .stylesheet_loads
                .get(&slot)
                .is_some_and(|load| load.request_id == request)
    }

    fn accepts_css_runtime_event(&self, slot: StylesheetSlotId, request: RequestId) -> bool {
        self.page.has_external_stylesheet_slot(slot)
            && self.stylesheet_loads.get(&slot).is_some_and(|load| {
                load.request_id == request
                    && (self.document_load.parse_status != DocumentParseStatus::Failed
                        || load.phase.network_complete())
            })
    }

    /// CSS-local delivery: failures are handled by the owning stylesheet, never
    /// by cancelling its document or sibling network requests.
    fn deliver_css_command(&self, command: CoreCommand) -> bool {
        self.cmd_tx
            .as_ref()
            .is_some_and(|tx| tx.send(command).is_ok())
    }

    /// The caller has already removed admission and settled/retired the slot.
    /// Failed delivery cannot reopen it or queue a retry for another generation.
    pub(super) fn abort_css_buffer(&mut self, slot: StylesheetSlotId, load: StylesheetLoadState) {
        if !self.deliver_css_command(CoreCommand::CssAbort {
            tab_id: self.tab_id,
            request_id: load.request_id,
            stylesheet_slot_id: slot,
            url: load.issued_key.as_str().to_owned(),
        }) && self.document_load.parse_status != DocumentParseStatus::Failed
        {
            self.last_status = Some("Stylesheet runtime cleanup delivery failed".into());
        }
    }

    fn fail_css_request(&mut self, slot: StylesheetSlotId, reason: &str) {
        let Some(load) = self.stylesheet_loads.remove(&slot) else {
            return;
        };
        let work = self.page.mark_css_aborted(slot);
        self.abort_css_buffer(slot, load);
        self.update_loading();
        if self.document_load.parse_status != DocumentParseStatus::Failed {
            self.last_status = Some(reason.into());
        }
        if !self.request_optional_render_work(work) {
            self.poke_redraw();
        }
    }

    pub(super) fn on_css_network_start(
        &mut self,
        slot: StylesheetSlotId,
        response: NetworkResponseInfo,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let load = self.stylesheet_loads.get(&slot).expect("issued request");
        // The producer emits one final response, after redirects. Duplicate
        // starts never replace admitted metadata, even when they conflict.
        if !matches!(load.phase, StylesheetLoadPhase::AwaitingStart) {
            return;
        }
        if response.requested_url != load.issued_key.as_str() {
            self.fail_css_request(
                slot,
                "Stylesheet response does not match the issued request",
            );
            return;
        }
        let accept_body = should_accept_css_response(response.content_type.as_deref());
        self.stylesheet_loads
            .get_mut(&slot)
            .expect("issued request")
            .phase = StylesheetLoadPhase::Receiving {
            response,
            accept_body,
            body_forwarded: false,
        };
    }

    pub(super) fn on_css_network_chunk(
        &mut self,
        slot: StylesheetSlotId,
        url: String,
        bytes: Vec<u8>,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let load = self.stylesheet_loads.get(&slot).expect("issued request");
        let accept_body = match &load.phase {
            StylesheetLoadPhase::AwaitingStart => {
                self.fail_css_request(slot, "Stylesheet chunk received before response admission");
                return;
            }
            StylesheetLoadPhase::Receiving { accept_body, .. } => *accept_body,
            _ => return,
        };
        if url != load.issued_key.as_str() {
            self.fail_css_request(slot, "Stylesheet chunk does not match the issued request");
            return;
        }
        if !accept_body {
            return;
        }
        if !self.deliver_css_command(CoreCommand::CssChunk {
            tab_id: self.tab_id,
            request_id: request,
            stylesheet_slot_id: slot,
            url,
            bytes,
        }) {
            self.fail_css_request(slot, "Stylesheet chunk delivery failed");
            return;
        }
        if let StylesheetLoadPhase::Receiving { body_forwarded, .. } = &mut self
            .stylesheet_loads
            .get_mut(&slot)
            .expect("issued request")
            .phase
        {
            *body_forwarded = true;
        }
    }

    pub(super) fn on_css_network_done(
        &mut self,
        slot: StylesheetSlotId,
        response: NetworkResponseInfo,
        _bytes_received: usize,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let load = self.stylesheet_loads.get(&slot).expect("issued request");
        let (accepted, accept_body, body_forwarded) = match &load.phase {
            StylesheetLoadPhase::AwaitingStart => {
                self.fail_css_request(
                    slot,
                    "Stylesheet completion received before response admission",
                );
                return;
            }
            StylesheetLoadPhase::Receiving {
                response,
                accept_body,
                body_forwarded,
            } => (response, *accept_body, *body_forwarded),
            _ => return,
        };
        // Do not normalize event strings or replace the immutable start snapshot.
        if accepted != &response {
            self.fail_css_request(
                slot,
                "Stylesheet completion does not match the admitted response",
            );
            return;
        }
        let accepted = accepted.clone();
        if !self.deliver_css_command(CoreCommand::CssDone {
            tab_id: self.tab_id,
            request_id: request,
            stylesheet_slot_id: slot,
            url: load.issued_key.as_str().to_owned(),
        }) {
            self.fail_css_request(slot, "Stylesheet completion delivery failed");
            return;
        }
        self.stylesheet_loads
            .get_mut(&slot)
            .expect("issued request")
            .phase = StylesheetLoadPhase::AwaitingRuntime {
            response: accepted,
            accept_body,
            progress: if body_forwarded {
                CssRuntimeProgress::AwaitingBlock
            } else {
                CssRuntimeProgress::NoBlockExpected
            },
        };
    }

    pub(super) fn on_css_network_error(
        &mut self,
        slot: StylesheetSlotId,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let load = self.stylesheet_loads.get(&slot).expect("issued request");
        if load.phase.network_complete() {
            return;
        }
        if url != load.issued_key.as_str() {
            self.fail_css_request(slot, "Stylesheet error does not match the issued request");
            return;
        }
        let message = format_network_error("stylesheet", &url, error_kind, status_code, &error);
        self.fail_css_request(slot, &message);
    }

    pub(super) fn on_css_decoded_block(
        &mut self,
        slot: StylesheetSlotId,
        url: &str,
        css_block: String,
        request: RequestId,
    ) {
        if !self.accepts_css_runtime_event(slot, request) {
            return;
        }
        let load = self.stylesheet_loads.get(&slot).expect("issued request");
        if matches!(load.phase, StylesheetLoadPhase::Complete { .. }) {
            return;
        }
        if url != load.issued_key.as_str() {
            self.fail_css_request(slot, "Decoded stylesheet does not match the issued request");
            return;
        }
        match &load.phase {
            StylesheetLoadPhase::AwaitingRuntime {
                accept_body: true,
                progress: CssRuntimeProgress::AwaitingBlock,
                ..
            } => {}
            StylesheetLoadPhase::AwaitingRuntime {
                progress: CssRuntimeProgress::BlockReceived,
                ..
            } => return,
            _ => {
                self.fail_css_request(slot, "Unexpected stylesheet runtime block");
                return;
            }
        }
        let work = self.page.apply_css_block(slot, &css_block);
        if let StylesheetLoadPhase::AwaitingRuntime { progress, .. } = &mut self
            .stylesheet_loads
            .get_mut(&slot)
            .expect("issued request")
            .phase
        {
            *progress = CssRuntimeProgress::BlockReceived;
        }
        self.request_optional_render_work(work);
    }

    pub(super) fn on_css_sheet_done(
        &mut self,
        slot: StylesheetSlotId,
        url: &str,
        request: RequestId,
    ) {
        if !self.accepts_css_runtime_event(slot, request) {
            return;
        }
        let load = self.stylesheet_loads.get(&slot).expect("issued request");
        if matches!(load.phase, StylesheetLoadPhase::Complete { .. }) {
            return;
        }
        if url != load.issued_key.as_str() {
            self.fail_css_request(
                slot,
                "Stylesheet runtime completion does not match the issued request",
            );
            return;
        }
        let (response, accept_body, progress) = match &load.phase {
            StylesheetLoadPhase::AwaitingRuntime {
                response,
                accept_body,
                progress,
            } if *progress != CssRuntimeProgress::AwaitingBlock => {
                (response.clone(), *accept_body, *progress)
            }
            _ => {
                self.fail_css_request(slot, "Premature stylesheet runtime completion");
                return;
            }
        };
        let work = if accept_body {
            self.page.mark_css_done(slot)
        } else {
            self.page.mark_css_failed(slot)
        };
        if progress == CssRuntimeProgress::BlockReceived && accept_body {
            self.stylesheet_loads
                .get_mut(&slot)
                .expect("issued request")
                .phase = StylesheetLoadPhase::Complete {
                response: response.clone(),
            };
        } else {
            // No decoded body means no loaded contribution (existing semantics).
            self.stylesheet_loads.remove(&slot);
        }
        self.update_loading();
        if self.document_load.parse_status != DocumentParseStatus::Failed {
            self.last_status = Some(if !accept_body {
                format!(
                    "Stylesheet ignored • unexpected content type {} ({} remaining)",
                    response.content_type.unwrap_or_else(|| "unknown".into()),
                    self.page.pending_count()
                )
            } else if self.page.pending_count() > 0 {
                format!(
                    "Stylesheet loaded ({} remaining)",
                    self.page.pending_count()
                )
            } else {
                "All stylesheets loaded".into()
            });
        }
        if !self.request_optional_render_work(work) {
            self.poke_redraw();
        }
    }
}

pub(super) fn should_accept_css_response(content_type: Option<&str>) -> bool {
    match normalized_content_type(content_type) {
        None => true,
        Some(content_type) => content_type == "text/css",
    }
}

fn normalized_content_type(content_type: Option<&str>) -> Option<String> {
    let content_type = content_type?.split(';').next()?.trim();
    if content_type.is_empty() {
        None
    } else {
        Some(content_type.to_ascii_lowercase())
    }
}

use super::Tab;
use super::state::{DocumentParseStatus, StylesheetLoadState};
use super::status::format_network_error;
use bus::CoreCommand;
use core_types::{NetworkErrorKind, NetworkResponseInfo, RequestId, StylesheetSlotId};

impl Tab {
    fn accepts_css_network_event(&self, slot_id: StylesheetSlotId) -> bool {
        self.document_load.parse_status != DocumentParseStatus::Failed
            && self.page.has_external_stylesheet_slot(slot_id)
    }

    fn accepts_css_runtime_event(&self, slot_id: StylesheetSlotId) -> bool {
        self.page.has_external_stylesheet_slot(slot_id)
            && (self.document_load.parse_status != DocumentParseStatus::Failed
                || self
                    .stylesheet_loads
                    .get(&slot_id)
                    .is_some_and(|load| load.network_done))
    }

    pub(super) fn on_css_network_start(
        &mut self,
        stylesheet_slot_id: StylesheetSlotId,
        response: NetworkResponseInfo,
    ) {
        if !self.accepts_css_network_event(stylesheet_slot_id) {
            return;
        }
        let accept_body = should_accept_css_response(response.content_type.as_deref());
        self.stylesheet_loads.insert(
            stylesheet_slot_id,
            StylesheetLoadState {
                response,
                accept_body,
                network_done: false,
            },
        );
    }

    pub(super) fn on_css_network_chunk(
        &mut self,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
        bytes: Vec<u8>,
        request_id: RequestId,
    ) {
        if !self.accepts_css_network_event(stylesheet_slot_id) {
            return;
        }
        if !self
            .stylesheet_loads
            .get(&stylesheet_slot_id)
            .map(|state| state.accept_body)
            .unwrap_or(true)
        {
            return;
        }
        self.send_cmd(CoreCommand::CssChunk {
            tab_id: self.tab_id,
            request_id,
            stylesheet_slot_id,
            url,
            bytes,
        });
    }

    pub(super) fn on_css_network_done(
        &mut self,
        stylesheet_slot_id: StylesheetSlotId,
        response: NetworkResponseInfo,
        _bytes_received: usize,
        request_id: RequestId,
    ) {
        if !self.accepts_css_network_event(stylesheet_slot_id) {
            return;
        }
        let url = response.requested_url.clone();
        self.stylesheet_loads.insert(
            stylesheet_slot_id,
            StylesheetLoadState {
                accept_body: should_accept_css_response(response.content_type.as_deref()),
                response,
                network_done: true,
            },
        );
        self.send_cmd(CoreCommand::CssDone {
            tab_id: self.tab_id,
            request_id,
            stylesheet_slot_id,
            url,
        });
    }

    pub(super) fn on_css_network_error(
        &mut self,
        stylesheet_slot_id: StylesheetSlotId,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
        request_id: RequestId,
    ) {
        if !self.accepts_css_network_event(stylesheet_slot_id) {
            return;
        }
        self.stylesheet_loads.remove(&stylesheet_slot_id);
        let render_work = self.page.mark_css_aborted(stylesheet_slot_id);
        self.send_cmd(CoreCommand::CssAbort {
            tab_id: self.tab_id,
            request_id,
            stylesheet_slot_id,
            url: url.clone(),
        });
        let remaining = self.page.pending_count();
        self.update_loading();
        if self.document_load.parse_status != super::state::DocumentParseStatus::Failed {
            self.last_status = Some(format!(
                "{} ({} remaining)",
                format_network_error("stylesheet", &url, error_kind, status_code, &error),
                remaining
            ));
        }
        if !self.request_optional_render_work(render_work) {
            self.poke_redraw();
        }
    }

    pub(super) fn on_css_decoded_block(
        &mut self,
        stylesheet_slot_id: StylesheetSlotId,
        css_block: String,
    ) {
        if !self.accepts_css_runtime_event(stylesheet_slot_id) {
            return;
        }
        let render_work = self.page.apply_css_block(stylesheet_slot_id, &css_block);
        let _ = self.request_optional_render_work(render_work);
    }

    pub(super) fn on_css_sheet_done(&mut self, stylesheet_slot_id: StylesheetSlotId, _url: String) {
        if !self.accepts_css_runtime_event(stylesheet_slot_id) {
            return;
        }
        let mut render_work = self.page.mark_css_done(stylesheet_slot_id);
        let remaining = self.page.pending_count();
        self.update_loading();
        let stylesheet = self.stylesheet_loads.remove(&stylesheet_slot_id);
        if stylesheet.as_ref().is_some_and(|state| !state.accept_body) {
            render_work = render_work.or(self.page.mark_css_failed(stylesheet_slot_id));
        }
        if self.document_load.parse_status != super::state::DocumentParseStatus::Failed {
            self.last_status = Some(match stylesheet {
                Some(state) if !state.accept_body => {
                    let content_type = state
                        .response
                        .content_type
                        .unwrap_or_else(|| "unknown".into());
                    if remaining > 0 {
                        format!(
                            "Stylesheet ignored • unexpected content type {content_type} ({} remaining)",
                            remaining
                        )
                    } else {
                        format!("Stylesheet ignored • unexpected content type {content_type}")
                    }
                }
                _ if remaining > 0 => format!("Stylesheet loaded ({} remaining)", remaining),
                _ => "All stylesheets loaded".to_string(),
            });
        }
        if !self.request_optional_render_work(render_work) {
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

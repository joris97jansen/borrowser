use super::Tab;
use super::state::StylesheetLoadState;
use super::status::format_network_error;
use bus::CoreCommand;
use core_types::{NetworkErrorKind, NetworkResponseInfo, RequestId};

impl Tab {
    pub(super) fn on_css_network_start(&mut self, response: NetworkResponseInfo) {
        let accept_body = should_accept_css_response(response.content_type.as_deref());
        self.stylesheet_loads.insert(
            response.requested_url.clone(),
            StylesheetLoadState {
                response,
                accept_body,
            },
        );
    }

    pub(super) fn on_css_network_chunk(
        &mut self,
        url: String,
        bytes: Vec<u8>,
        request_id: RequestId,
    ) {
        if !self
            .stylesheet_loads
            .get(&url)
            .map(|state| state.accept_body)
            .unwrap_or(true)
        {
            return;
        }
        self.send_cmd(CoreCommand::CssChunk {
            tab_id: self.tab_id,
            request_id,
            url,
            bytes,
        });
    }

    pub(super) fn on_css_network_done(
        &mut self,
        response: NetworkResponseInfo,
        _bytes_received: usize,
        request_id: RequestId,
    ) {
        let url = response.requested_url.clone();
        self.stylesheet_loads.insert(
            url.clone(),
            StylesheetLoadState {
                accept_body: should_accept_css_response(response.content_type.as_deref()),
                response,
            },
        );
        self.send_cmd(CoreCommand::CssDone {
            tab_id: self.tab_id,
            request_id,
            url,
        });
    }

    pub(super) fn on_css_network_error(
        &mut self,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
        request_id: RequestId,
    ) {
        self.stylesheet_loads.remove(&url);
        self.page.mark_css_done(&url);
        self.send_cmd(CoreCommand::CssAbort {
            tab_id: self.tab_id,
            request_id,
            url: url.clone(),
        });
        let remaining = self.page.pending_count();
        self.loading = remaining > 0;
        self.last_status = Some(format!(
            "{} ({} remaining)",
            format_network_error("stylesheet", &url, error_kind, status_code, &error),
            remaining
        ));
        self.poke_redraw();
    }

    pub(super) fn on_css_decoded_block(&mut self, css_block: String) {
        self.page.apply_css_block(&css_block);
        self.poke_redraw();
    }

    pub(super) fn on_css_sheet_done(&mut self, url: String) {
        self.page.mark_css_done(&url);
        let remaining = self.page.pending_count();
        self.loading = remaining > 0;
        let stylesheet = self.stylesheet_loads.remove(&url);
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
        self.poke_redraw();
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

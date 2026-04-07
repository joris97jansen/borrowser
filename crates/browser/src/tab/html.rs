use super::Tab;
use super::state::DocumentLoadState;
use super::status::{format_network_error, response_summary};
use bus::CoreCommand;
use core_types::{NetworkErrorKind, NetworkResponseInfo, RequestId};
use html::Node;

impl Tab {
    pub(super) fn on_html_network_start(
        &mut self,
        response: NetworkResponseInfo,
        request_id: RequestId,
    ) {
        self.dom_store.clear();
        self.dom_handle = None;
        self.document_load = DocumentLoadState {
            response: Some(response.clone()),
            bytes_received: 0,
        };
        self.stylesheet_loads.clear();
        self.url = response.final_url.clone();
        self.page.start_nav(response.display_url());
        self.loading = true;
        self.last_status = Some(format!(
            "Loading document • {}",
            response_summary(&response, 0)
        ));
        self.send_cmd(CoreCommand::ParseHtmlStart {
            tab_id: self.tab_id,
            request_id,
        });
        self.poke_redraw();
    }

    pub(super) fn on_html_network_chunk(&mut self, bytes: Vec<u8>, request_id: RequestId) {
        self.document_load.bytes_received = self
            .document_load
            .bytes_received
            .saturating_add(bytes.len());
        self.send_cmd(CoreCommand::ParseHtmlChunk {
            tab_id: self.tab_id,
            request_id,
            bytes,
        });
    }

    pub(super) fn on_html_network_done(
        &mut self,
        response: NetworkResponseInfo,
        bytes_received: usize,
        request_id: RequestId,
    ) {
        self.document_load = DocumentLoadState {
            response: Some(response.clone()),
            bytes_received,
        };
        self.send_cmd(CoreCommand::ParseHtmlDone {
            tab_id: self.tab_id,
            request_id,
        });
        self.last_status = Some(format!(
            "Document response complete • {}",
            response_summary(&response, bytes_received)
        ));
        self.poke_redraw();
    }

    pub(super) fn on_html_network_error(
        &mut self,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
    ) {
        self.loading = false;
        self.last_status = Some(format_network_error(
            "document",
            &url,
            error_kind,
            status_code,
            &error,
        ));
        self.poke_redraw();
    }

    pub(super) fn on_dom_update(&mut self, dom: Box<Node>, request_id: RequestId) {
        self.page.dom = Some(dom);
        self.page.update_head_metadata();
        self.page.apply_inline_style_blocks();
        self.page
            .seed_input_values_from_dom(&mut self.document_input.input_values);
        self.page.update_visible_text_cache();

        self.discover_resources(request_id);

        let pending = self.page.pending_count();
        self.loading = pending > 0;
        let response = self.document_load.response.as_ref();
        let base = if pending > 0 {
            format!("Document parsed • fetching {pending} stylesheet(s)")
        } else {
            "Document parsed".to_string()
        };
        self.last_status = Some(match response {
            Some(response) => format!(
                "{base} • {}",
                response_summary(response, self.document_load.bytes_received)
            ),
            None => base,
        });
        self.poke_redraw();
    }
}

use super::Tab;
use super::state::{DocumentLoadFailure, DocumentParseStatus};
use super::status::response_summary;
use bus::CoreCommand;
use core_types::RequestId;
use net::{BodyProgress, NetworkErrorKind, NetworkResponseInfo};

impl Tab {
    pub(super) fn on_html_network_start(
        &mut self,
        response: NetworkResponseInfo,
        request_id: RequestId,
    ) {
        // Networking emits one start for the final response, after redirects.
        // Only navigation resets the lifecycle; duplicate starts must not clear
        // a streaming publication or reopen a completed/failed parser session.
        if self.document_load.parse_status != DocumentParseStatus::Pending
            || self.document_load.response.is_some()
        {
            return;
        }
        if !self.response_matches_history_request(response.requested_url()) {
            self.fail_document(DocumentLoadFailure::ResponseRequestMismatch(response));
            return;
        }
        let document_url = match response.response_url() {
            Some(Ok(parsed)) => parsed.url.clone(),
            _ => {
                self.fail_document(DocumentLoadFailure::ResponseUrlUnavailable(response));
                return;
            }
        };
        // All fallible admission work precedes history/response/page mutations.
        // URL-origin computation alone cannot stand in for missing creation
        // context. A rejected response follows the existing terminal failure
        // path; navigation may already have retired the previous page.
        let addresses = match crate::page::DocumentAddressState::new(document_url) {
            Ok(addresses) => addresses,
            Err(cause) => {
                self.fail_document(DocumentLoadFailure::UnsupportedDocumentCreation {
                    cause,
                    response_context: response,
                });
                return;
            }
        };
        self.accept_history_document(addresses.document_url());
        self.retire_document_stylesheets();
        self.dom_store.clear();
        self.dom_handle = None;
        self.dom_version = core_types::DomVersion::INITIAL;
        self.document_load.response = Some(response.clone());
        self.document_load.bytes_received = 0;
        self.stylesheet_loads.clear();
        self.url = response
            .reported_response_url()
            .expect("admitted response URL")
            .to_owned();
        self.page.reset_for_navigation();
        self.page.begin_document(addresses);
        self.clear_render_orchestration_state();
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

    pub(super) fn on_html_network_chunk(
        &mut self,
        url: &str,
        bytes: Vec<u8>,
        request_id: RequestId,
    ) {
        if self.document_load.parse_status != DocumentParseStatus::Pending
            || self.document_load.body_completion.is_some()
        {
            return;
        }
        // Pending also covers the interval before response admission. Only an
        // accepted start authorizes feeding this document's parser session.
        let Some(accepted) = self.document_load.response.as_ref() else {
            self.fail_document(DocumentLoadFailure::ChunkBeforeAdmission);
            return;
        };
        // Producers label chunks with the original requested URL, even after
        // redirects or same-document URL changes. Preserve that exact provenance.
        if url != accepted.requested_url() {
            self.fail_document(DocumentLoadFailure::ChunkProvenanceMismatch);
            return;
        }
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
        body: BodyProgress,
        request_id: RequestId,
    ) {
        if self.document_load.parse_status != DocumentParseStatus::Pending
            || self.document_load.body_completion.is_some()
        {
            return;
        }
        let Some(accepted) = self.document_load.response.as_ref() else {
            self.fail_document(DocumentLoadFailure::CompletionBeforeAdmission);
            return;
        };
        // HTTP and file producers reuse the exact start snapshot at completion.
        // Preserve all immutable provenance, not just equivalent URL spelling:
        // requested/final URL, status, and content type must agree. Completion
        // cannot admit a document or change its retained URL/origin/history.
        if accepted != &response {
            self.fail_document(DocumentLoadFailure::CompletionMetadataMismatch(response));
            return;
        }
        if !body.matches_route(core_types::ResourceKind::Html)
            || body.delivered_bytes() != self.document_load.bytes_received
        {
            self.fail_document(DocumentLoadFailure::CompletionAccountingMismatch(body));
            return;
        }
        let bytes_received = body.delivered_bytes();
        self.document_load.bytes_received = bytes_received;
        self.document_load.body_completion = Some(body);
        let summary = response_summary(accepted, bytes_received);
        self.send_cmd(CoreCommand::ParseHtmlDone {
            tab_id: self.tab_id,
            request_id,
        });
        self.last_status = Some(format!("Document response complete • {}", summary));
        self.poke_redraw();
    }

    pub(super) fn on_html_network_error(
        &mut self,
        error_kind: NetworkErrorKind,
        response_context: Option<NetworkResponseInfo>,
        body_progress: Option<BodyProgress>,
    ) {
        if self.document_load.parse_status != DocumentParseStatus::Pending
            || self.document_load.body_completion.is_some()
        {
            return;
        }
        self.fail_document(DocumentLoadFailure::Network {
            kind: error_kind,
            response_context,
            body_progress,
        });
    }
}

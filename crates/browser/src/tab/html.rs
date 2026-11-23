use super::Tab;
use super::state::{
    DocumentEncodingDecision, DocumentLoadFailure, DocumentParseStatus, DocumentResponseRejection,
};
use super::status::response_summary;
use bus::CoreCommand;
use core_types::RequestId;
use net::{
    BodyProgress, FetchRequest, NetworkErrorKind, NetworkResponseInfo, ResponseContentDisposition,
    ResponseContentType,
};

enum DocumentResponsePreparation {
    Html {
        addresses: Box<crate::page::DocumentAddressState>,
        encoding: DocumentEncodingDecision,
    },
    PreserveActiveDocument,
}

fn select_document_encoding(
    charset: Option<&str>,
) -> Result<DocumentEncodingDecision, DocumentResponseRejection> {
    let Some(label) = charset else {
        return Ok(DocumentEncodingDecision::MissingCharsetDefault);
    };
    // Encoding Standard ASCII whitespace, not Unicode whitespace or HTTP OWS.
    let label = label.trim_matches(['\t', '\n', '\u{000C}', '\r', ' ']);
    if label.is_empty() {
        return Ok(DocumentEncodingDecision::EmptyCharsetDefault);
    }
    if [
        "unicode-1-1-utf-8",
        "unicode11utf8",
        "unicode20utf8",
        "utf-8",
        "utf8",
        "x-unicode20utf8",
    ]
    .iter()
    .any(|accepted| label.eq_ignore_ascii_case(accepted))
    {
        Ok(DocumentEncodingDecision::ResponseCharset)
    } else {
        Err(DocumentResponseRejection::UnsupportedDocumentCharset)
    }
}

fn prepare_document_response(
    request: &FetchRequest,
    response: &NetworkResponseInfo,
) -> Result<DocumentResponsePreparation, DocumentLoadFailure> {
    if response.requested_url() != request.original_url().as_str()
        || response.original_url() != request.original_url()
        || response.resource_kind() != request.address().kind()
    {
        return Err(DocumentLoadFailure::ResponseRequestMismatch(
            response.clone(),
        ));
    }
    let url = match response.response_url() {
        Some(Ok(parsed)) => parsed.url.clone(),
        _ => {
            return Err(DocumentLoadFailure::ResponseUrlUnavailable(
                response.clone(),
            ));
        }
    };
    let reject = |reason| DocumentLoadFailure::ResponseRejected {
        reason,
        response_context: response.clone(),
    };
    // AI owns extraction. Invalid declarations remain visible and are ignored
    // for applicability; only valid attachment/extension types prevent HTML.
    if matches!(
        response.content_disposition(),
        ResponseContentDisposition::Attachment | ResponseContentDisposition::Other
    ) {
        return Err(reject(DocumentResponseRejection::DownloadUnsupported));
    }
    match response.status_code() {
        Some(204 | 205) => return Ok(DocumentResponsePreparation::PreserveActiveDocument),
        Some(304) => {
            return Err(reject(
                DocumentResponseRejection::NotModifiedWithoutRepresentation,
            ));
        }
        Some(206) => {
            return Err(reject(
                DocumentResponseRejection::PartialRepresentationUnsupported,
            ));
        }
        Some(226) => {
            return Err(reject(
                DocumentResponseRejection::InstanceManipulationUnsupported,
            ));
        }
        _ => {}
    }
    let encoding = match response.content_type() {
        ResponseContentType::Missing => {
            return Err(reject(DocumentResponseRejection::MissingContentType));
        }
        ResponseContentType::Empty => {
            return Err(reject(DocumentResponseRejection::EmptyContentType));
        }
        ResponseContentType::Unusable => {
            return Err(reject(DocumentResponseRejection::UnusableContentType));
        }
        ResponseContentType::Parsed(value) if value.mime_type() == "text/html" => {
            select_document_encoding(value.charset()).map_err(reject)?
        }
        _ => return Err(reject(DocumentResponseRejection::UnsupportedContentType)),
    };
    let addresses = crate::page::DocumentAddressState::new(url).map_err(|cause| {
        DocumentLoadFailure::UnsupportedDocumentCreation {
            cause,
            response_context: response.clone(),
        }
    })?;
    Ok(DocumentResponsePreparation::Html {
        addresses: Box::new(addresses),
        encoding,
    })
}

impl Tab {
    pub(super) fn on_html_network_start(
        &mut self,
        response: NetworkResponseInfo,
        request_id: RequestId,
    ) {
        let Some(attempt) = self.navigation_attempt.as_ref().filter(|a| {
            a.request.request_id() == request_id
                && a.terminal.is_none()
                && a.non_replacing_response.is_none()
        }) else {
            // Includes duplicate starts for an already activated document.
            return;
        };
        match prepare_document_response(&attempt.request, &response) {
            Ok(DocumentResponsePreparation::Html {
                addresses,
                encoding,
            }) => {
                if let Err(failure) =
                    self.activate_document_response(response, *addresses, encoding)
                {
                    self.fail_navigation_attempt(failure);
                }
            }
            Ok(DocumentResponsePreparation::PreserveActiveDocument) => {
                self.navigation_attempt
                    .as_mut()
                    .expect("matched attempt")
                    .non_replacing_response = Some(response);
                self.poke_redraw();
            }
            Err(failure) => self.fail_navigation_attempt(failure),
        }
    }

    pub(super) fn on_html_network_chunk(
        &mut self,
        url: &str,
        bytes: Vec<u8>,
        request_id: RequestId,
    ) {
        if self.is_navigation_request(self.tab_id, request_id) {
            if self
                .navigation_attempt
                .as_ref()
                .is_some_and(|a| a.terminal.is_none())
            {
                let failure = if self
                    .navigation_attempt
                    .as_ref()
                    .is_some_and(|a| a.non_replacing_response.is_some())
                {
                    DocumentLoadFailure::UnexpectedNoContentBody
                } else {
                    DocumentLoadFailure::ChunkBeforeAdmission
                };
                self.fail_navigation_attempt(failure);
            }
            return;
        }
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
        if let Err(failure) = self.enqueue_html_command(CoreCommand::ParseHtmlChunk {
            tab_id: self.tab_id,
            request_id,
            bytes,
        }) {
            self.fail_document(failure);
        }
    }

    pub(super) fn on_html_network_done(
        &mut self,
        response: NetworkResponseInfo,
        body: BodyProgress,
        request_id: RequestId,
    ) {
        if self.is_navigation_request(self.tab_id, request_id) {
            self.finish_non_replacing_response(response, body);
            return;
        }
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
        if let Err(failure) = self.enqueue_html_command(CoreCommand::ParseHtmlDone {
            tab_id: self.tab_id,
            request_id,
        }) {
            self.fail_document(failure);
            return;
        }
        self.last_status = Some(format!("Document response complete • {}", summary));
        self.poke_redraw();
    }

    pub(super) fn on_html_network_error(
        &mut self,
        error_kind: NetworkErrorKind,
        response_context: Option<NetworkResponseInfo>,
        body_progress: Option<BodyProgress>,
        request_id: RequestId,
    ) {
        if self.is_navigation_request(self.tab_id, request_id) {
            self.fail_navigation_attempt(DocumentLoadFailure::Network {
                kind: error_kind,
                response_context,
                body_progress,
            });
            return;
        }
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
    fn finish_non_replacing_response(&mut self, response: NetworkResponseInfo, body: BodyProgress) {
        let Some(attempt) = self
            .navigation_attempt
            .as_ref()
            .filter(|a| a.terminal.is_none())
        else {
            return;
        };
        let failure = match &attempt.non_replacing_response {
            None => Some(DocumentLoadFailure::CompletionBeforeAdmission),
            Some(accepted) if accepted != &response => {
                Some(DocumentLoadFailure::CompletionMetadataMismatch(response))
            }
            Some(_)
                if !body.matches_route(core_types::ResourceKind::Html)
                    || body.delivered_bytes() != 0 =>
            {
                Some(DocumentLoadFailure::CompletionAccountingMismatch(body))
            }
            Some(_) => None,
        };
        if let Some(failure) = failure {
            self.fail_navigation_attempt(failure);
            return;
        }
        self.navigation_attempt
            .as_mut()
            .expect("matched attempt")
            .terminal = Some(Ok(body));
        self.restore_active_toolbar_url();
        self.poke_redraw();
    }
}

#[cfg(test)]
mod decoding_tests {
    use super::*;

    #[test]
    fn aj4_charset_normalization_uses_only_encoding_standard_whitespace() {
        assert_eq!(
            select_document_encoding(Some("\t\n\u{c}\r UTF8 \r\n")),
            Ok(DocumentEncodingDecision::ResponseCharset)
        );
        for label in [
            "\u{b}utf8",
            "\u{a0}utf8",
            "\u{feff}utf8",
            "utf_8",
            "us-ascii",
        ] {
            assert_eq!(
                select_document_encoding(Some(label)),
                Err(DocumentResponseRejection::UnsupportedDocumentCharset)
            );
        }
    }
}

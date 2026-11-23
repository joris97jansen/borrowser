use super::state::DocumentLoadFailure;
use net::{NetworkErrorKind, NetworkResponseInfo};

/// Fixed, bounded presentation of owning typed failures. Parser/backend details,
/// URLs, headers and bodies never enter UI text or nested Debug through here.
pub(super) fn document_failure_status(failure: &DocumentLoadFailure) -> String {
    use DocumentLoadFailure::*;
    match failure {
        Network {
            kind,
            response_context,
            ..
        } => format_network_error(
            "document",
            "",
            *kind,
            response_context
                .as_ref()
                .and_then(NetworkResponseInfo::status_code),
            "",
        ),
        ParserCommandDeliveryUnavailable => "Document parser command delivery unavailable".into(),
        UnexpectedNoContentBody => "Unexpected body for a non-replacing document response".into(),
        ResponseRejected { reason, .. } => format!("Document response rejected: {reason:?}"),
        Parser(error) => {
            use bus::HtmlParseFailure::*;
            let (stage, detail) = match error {
                Initialization(error) => ("Initialization", Some(error)),
                Execution(error) => ("Execution", Some(error)),
                Finalization(error) => ("Finalization", Some(error)),
                DomHandleExhausted => ("DomHandleExhausted", None),
                PreSelectionBudgetExceeded => ("PreSelectionBudgetExceeded", None),
                DocumentModeUnavailable => ("DocumentModeUnavailable", None),
                DocumentModeChanged { .. } => ("DocumentModeChanged", None),
                InputClosed => ("InputClosed", None),
            };
            let detail = detail.map(|error| match error {
                html::HtmlParseError::Decode => "Decode",
                html::HtmlParseError::DocumentModeUnavailable => "DocumentModeUnavailable",
                html::HtmlParseError::Fatal(_) => "Fatal",
                html::HtmlParseError::PatchValidation(_) => "PatchValidation",
            });
            match detail {
                Some(detail) => format!("Document parsing failed: {stage}({detail})"),
                None => format!("Document parsing failed: {stage}"),
            }
        }
        Publication(error) => format!("Document publication failed: {error:?}"),
        ResponseRequestMismatch(_) => "Document response does not match the current request".into(),
        ResponseUrlUnavailable(_) => "Document response URL is missing or invalid".into(),
        UnsupportedDocumentCreation { .. } => "Unsupported document creation context".into(),
        ChunkBeforeAdmission => "HTML chunk received before document admission".into(),
        ChunkProvenanceMismatch => "HTML chunk does not match the accepted response".into(),
        CompletionBeforeAdmission => "HTML completion received before document admission".into(),
        CompletionMetadataMismatch(_) => {
            "HTML completion does not match the accepted response".into()
        }
        CompletionAccountingMismatch(_) => {
            "Document completion body accounting does not match delivery".into()
        }
        ParserCompletionBeforeBody => {
            "Document parser completed before accepted body completion".into()
        }
        // A slot's snapshot-local ordinal requires DocumentStyleSet's order.
        // Tab Debug supplies that correlation, including omitted rows; this
        // context-free projection must not invent it or expose the raw ID.
        LoadStateInvariantViolation {
            relationship,
            slot,
            slot_records,
            request_records,
        } => format!(
            "Document load state invariant violated: {relationship:?} (slot_present={} slots={slot_records} issued={request_records})",
            slot.is_some()
        ),
    }
}

pub(super) fn response_summary(response: &NetworkResponseInfo, bytes_received: usize) -> String {
    let mut parts = Vec::new();

    if let Some(status_code) = response.status_code() {
        parts.push(format!("HTTP {status_code}"));
    }

    if bytes_received > 0 {
        parts.push(format!("{bytes_received} B"));
    }

    parts.join(" • ")
}

pub(super) fn format_network_error(
    resource_label: &str,
    _url: &str,
    error_kind: NetworkErrorKind,
    status_code: Option<u16>,
    _error: &str,
) -> String {
    let message = match error_kind {
        NetworkErrorKind::Cancelled => format!("Cancelled {resource_label} load"),
        NetworkErrorKind::Backend => {
            format!("Backend error loading {resource_label}")
        }
        NetworkErrorKind::DnsFailure => format!("DNS error loading {resource_label}"),
        NetworkErrorKind::ConnectionFailure => format!("Connection error loading {resource_label}"),
        NetworkErrorKind::TlsFailure => format!("TLS error loading {resource_label}"),
        NetworkErrorKind::Timeout => format!("Timed out loading {resource_label}"),
        NetworkErrorKind::LocalFile => {
            format!("Local file error loading {resource_label}")
        }
        NetworkErrorKind::Read => format!("Read error loading {resource_label}"),
        NetworkErrorKind::MalformedResponse => {
            format!("Malformed response loading {resource_label}")
        }
        NetworkErrorKind::UnsupportedResponse => {
            format!("Unsupported response loading {resource_label}")
        }
        NetworkErrorKind::UnsupportedEncoding => {
            format!("Unsupported encoding loading {resource_label}")
        }
        NetworkErrorKind::ContentDecoding => {
            format!("Content decoding error loading {resource_label}")
        }
        NetworkErrorKind::ResourceLimit(_) => {
            format!("Resource limit loading {resource_label}")
        }
        NetworkErrorKind::Redirect(_) => format!("Redirect error loading {resource_label}"),
    };
    match status_code {
        Some(code) => format!("{message} [HTTP {code}]"),
        None => message,
    }
}

impl super::Tab {
    /// Presentation only: never a lifecycle cache or an active-frame gate.
    pub(super) fn visible_load_status(&self) -> Option<String> {
        let active = self.active_document_request_id().and_then(|_| {
            self.document_load
                .failure()
                .map(document_failure_status)
                .or_else(|| self.last_status.clone())
        });
        let navigation =
            self.navigation_attempt
                .as_ref()
                .and_then(|attempt| match &attempt.terminal {
                    // Production Cancelled events originate only from the
                    // runtime's Browser-commanded token. Browser first latches
                    // its terminal or retires the request; see the AJ3 audit.
                    // Keep that diagnostic without a misleading failure notice.
                    Some(Err(DocumentLoadFailure::Network {
                        kind: NetworkErrorKind::Cancelled,
                        ..
                    })) => None,
                    Some(Err(failure)) => Some(format!(
                        "Navigation failed • {}",
                        document_failure_status(failure)
                    )),
                    Some(Ok(_)) => Some(if self.active_document_request_id().is_some() {
                        "Navigation completed • active document preserved".into()
                    } else {
                        "Navigation completed • no document installed".into()
                    }),
                    None if attempt.non_replacing_response.is_some() => {
                        Some("Navigation • awaiting non-replacing body completion".into())
                    }
                    None => Some("Navigation • fetching document".into()),
                });
        let lifecycle = match (active, navigation) {
            (Some(active), Some(navigation)) => {
                Some(format!("{navigation} • Active document: {active}"))
            }
            (active, navigation) => navigation.or(active),
        };
        match (&self.navigation_notice, lifecycle) {
            (Some(notice), Some(lifecycle)) => {
                Some(format!("Navigation submission: {notice} • {lifecycle}"))
            }
            (Some(notice), None) => Some(format!("Navigation submission: {notice}")),
            (None, lifecycle) => lifecycle,
        }
    }
}

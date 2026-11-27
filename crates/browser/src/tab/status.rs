use super::state::DocumentLoadFailure;
use net::{NetworkErrorKind, NetworkResponseInfo};

/// Fixed, bounded presentation of owning typed failures. Parser/backend details,
/// URLs, headers and bodies never enter UI text or nested Debug through here.
pub(super) fn document_failure_status(failure: &DocumentLoadFailure) -> String {
    let mut out = String::new();
    write_document_failure_status(&mut out, failure).expect("String formatting");
    out
}
pub(super) fn write_document_failure_status(
    out: &mut impl std::fmt::Write,
    failure: &DocumentLoadFailure,
) -> std::fmt::Result {
    use DocumentLoadFailure::*;
    match failure {
        Network {
            kind,
            response_context,
            ..
        } => write_network_error(
            out,
            "document",
            *kind,
            response_context
                .as_ref()
                .and_then(NetworkResponseInfo::status_code),
        ),
        StylesheetInventory(cause) => {
            write!(out, "Document stylesheet inventory unavailable: {cause:?}")
        }
        ParserCommandDeliveryUnavailable => {
            out.write_str("Document parser command delivery unavailable")
        }
        UnexpectedNoContentBody => {
            out.write_str("Unexpected body for a non-replacing document response")
        }
        ResponseRejected { reason, .. } => write!(out, "Document response rejected: {reason:?}"),
        Parser(error) => {
            use bus::HtmlParseFailure::*;
            let (stage, detail) = match error {
                Initialization(e) => ("Initialization", Some(e)),
                Execution(e) => ("Execution", Some(e)),
                Finalization(e) => ("Finalization", Some(e)),
                DomHandleExhausted => ("DomHandleExhausted", None),
                PreSelectionBudgetExceeded => ("PreSelectionBudgetExceeded", None),
                DocumentModeUnavailable => ("DocumentModeUnavailable", None),
                DocumentModeChanged { .. } => ("DocumentModeChanged", None),
                InputClosed => ("InputClosed", None),
            };
            let detail = detail.map(|e| match e {
                html::HtmlParseError::Decode => "Decode",
                html::HtmlParseError::DocumentModeUnavailable => "DocumentModeUnavailable",
                html::HtmlParseError::Fatal(_) => "Fatal",
                html::HtmlParseError::PatchValidation(_) => "PatchValidation",
            });
            match detail {
                Some(d) => write!(out, "Document parsing failed: {stage}({d})"),
                None => write!(out, "Document parsing failed: {stage}"),
            }
        }
        Publication(error) => write!(out, "Document publication failed: {error:?}"),
        ResponseRequestMismatch(_) => {
            out.write_str("Document response does not match the current request")
        }
        ResponseUrlUnavailable(_) => out.write_str("Document response URL is missing or invalid"),
        UnsupportedDocumentCreation { .. } => {
            out.write_str("Unsupported document creation context")
        }
        ChunkBeforeAdmission => out.write_str("HTML chunk received before document admission"),
        ChunkProvenanceMismatch => out.write_str("HTML chunk does not match the accepted response"),
        CompletionBeforeAdmission => {
            out.write_str("HTML completion received before document admission")
        }
        CompletionMetadataMismatch(_) => {
            out.write_str("HTML completion does not match the accepted response")
        }
        CompletionAccountingMismatch(_) => {
            out.write_str("Document completion body accounting does not match delivery")
        }
        ParserCompletionBeforeBody => {
            out.write_str("Document parser completed before accepted body completion")
        }
        LoadStateInvariantViolation {
            relationship,
            slot,
            slot_records,
            request_records,
        } => write!(
            out,
            "Document load state invariant violated: {relationship:?} (slot_present={} slots={slot_records} issued={request_records})",
            slot.is_some()
        ),
    }
}
fn write_network_error(
    out: &mut impl std::fmt::Write,
    label: &str,
    kind: NetworkErrorKind,
    status: Option<u16>,
) -> std::fmt::Result {
    match kind {
        NetworkErrorKind::Cancelled => write!(out, "Cancelled {label} load")?,
        NetworkErrorKind::Backend => write!(out, "Backend error loading {label}")?,
        NetworkErrorKind::DnsFailure => write!(out, "DNS error loading {label}")?,
        NetworkErrorKind::ConnectionFailure => write!(out, "Connection error loading {label}")?,
        NetworkErrorKind::TlsFailure => write!(out, "TLS error loading {label}")?,
        NetworkErrorKind::Timeout => write!(out, "Timed out loading {label}")?,
        NetworkErrorKind::LocalFile => write!(out, "Local file error loading {label}")?,
        NetworkErrorKind::Read => write!(out, "Read error loading {label}")?,
        NetworkErrorKind::MalformedResponse => write!(out, "Malformed response loading {label}")?,
        NetworkErrorKind::UnsupportedResponse => {
            write!(out, "Unsupported response loading {label}")?
        }
        NetworkErrorKind::UnsupportedEncoding => {
            write!(out, "Unsupported encoding loading {label}")?
        }
        NetworkErrorKind::ContentDecoding => write!(out, "Content decoding error loading {label}")?,
        NetworkErrorKind::ResourceLimit(_) => write!(out, "Resource limit loading {label}")?,
        NetworkErrorKind::Redirect(_) => write!(out, "Redirect error loading {label}")?,
    }
    if let Some(code) = status {
        write!(out, " [HTTP {code}]")?;
    }
    Ok(())
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
    let mut message = String::new();
    write_network_error(&mut message, resource_label, error_kind, status_code)
        .expect("String formatting");
    message
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

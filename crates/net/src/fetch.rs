use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::{
    BodyProgress, HttpStatus, NetworkErrorKind, ResponseAdmissionError, body::ContentCoding,
};
use core_types::ResourceKind;

use crate::{
    FetchRequest, HttpClientPolicy, NetEvent, RequestAdmissionError,
    backend::{
        AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend, UreqBackend, backend_failure,
    },
    file::fetch_file_url,
    log::{log_network_error, log_redirect_decision},
    redirect::next_target,
    stream::{StreamFailure, http_body_reader, stream_reader, verify_empty_body},
};

pub fn fetch_stream(
    request: FetchRequest,
    kind: ResourceKind,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
) -> Result<(), RequestAdmissionError> {
    fetch_stream_with_policy(
        request,
        kind,
        HttpClientPolicy::default(),
        cancel_token,
        callback,
    )
}

pub(crate) fn fetch_stream_with_policy(
    request: FetchRequest,
    kind: ResourceKind,
    policy: HttpClientPolicy,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
) -> Result<(), RequestAdmissionError> {
    if request.current_url().scheme() == "file" {
        request.validate_route(kind)?;
        let url = request.legacy_issued_url(kind)?;
        thread::spawn(move || {
            if cancel_token.load(Ordering::Relaxed) {
                callback(NetEvent::Error {
                    request_id: request.request_id(),
                    url,
                    error_kind: NetworkErrorKind::Cancelled,
                    response: None,
                    body: None,
                    error: "cancelled".into(),
                });
                return;
            }
            fetch_file_url(&request, &url, kind, &cancel_token, &callback);
        });
        return Ok(());
    }
    fetch_http_with_backend(
        request,
        kind,
        UreqBackend::new(policy),
        cancel_token,
        callback,
    )
}

/// Shared production HTTP lifecycle. There is no file dispatch or real-backend
/// fallback here; the concrete acquisition implementation is supplied by value.
pub(crate) fn fetch_http_with_backend<B: HttpResponseBackend>(
    request: FetchRequest,
    kind: ResourceKind,
    backend: B,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
) -> Result<(), RequestAdmissionError> {
    request.validate_route(kind)?;
    if !matches!(request.current_url().scheme(), "http" | "https") {
        return Err(RequestAdmissionError::UnsupportedScheme);
    }
    let url = request.legacy_issued_url(kind)?;
    let request_id = request.request_id();
    thread::spawn(move || {
        let mut request = request;
        let mut history = Vec::new();
        let mut pending: Option<crate::RedirectHop> = None;
        let mut last_response: Option<crate::NetworkResponseInfo> = None;
        let zero = BodyProgress::try_new(kind, 0).expect("zero body progress");
        let AcquiredHttpResponse {
            metadata: response_info,
            mut reader,
        } = loop {
            // Includes the checkpoint after dropping an intermediate reader.
            // An admitted pending target is not yet acquisition history.
            if cancel_token.load(Ordering::Relaxed) {
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind: NetworkErrorKind::Cancelled,
                    body: last_response.as_ref().map(|_| zero),
                    response: last_response,
                    error: "cancelled".into(),
                });
                return;
            }
            // Entry is the commitment boundary, not proof of transmission.
            // Materialize the entered transition on either returned outcome,
            // before interpreting the result. Nothing publishes history while
            // this synchronous acquisition is blocked.
            let acquired = backend.acquire(&request, kind);
            if let Some(hop) = pending.take() {
                let status = hop.status();
                history.push(hop);
                log_redirect_decision(request_id, kind, history.len(), status, "entered");
            }
            let AcquiredHttpResponse { metadata, reader } = match acquired {
                Ok(acquired) => acquired,
                Err(failure) => {
                    let (error_kind, error) = match failure {
                        HttpAcquireError::Response(error) => {
                            (admission_kind(error), error.to_string())
                        }
                        HttpAcquireError::Backend(error) => {
                            let (kind, message) = backend_failure(&error);
                            (kind, message.into())
                        }
                        HttpAcquireError::Network { kind, error } => (kind, error),
                    };
                    log_network_error(request_id, kind, "acquisition", error_kind);
                    callback(NetEvent::Error {
                        request_id,
                        url,
                        error_kind,
                        body: last_response.as_ref().map(|_| zero),
                        response: last_response
                            .map(|response| response.with_redirect_chain(&history)),
                        error,
                    });
                    return;
                }
            };
            let response_info = metadata.with_redirect_chain(&history);
            if let Err(error) = response_info.validate_backend_http() {
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind: admission_kind(error),
                    response: Some(response_info),
                    body: None,
                    error: error.to_string(),
                });
                return;
            }
            let candidate = response_info.status().is_some_and(HttpStatus::is_redirect)
                && response_info.headers().values("location").next().is_some();
            if candidate && cancel_token.load(Ordering::Relaxed) {
                drop(reader);
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind: NetworkErrorKind::Cancelled,
                    response: Some(response_info),
                    body: Some(zero),
                    error: "cancelled".into(),
                });
                return;
            }
            match next_target(&request, &response_info, history.len()) {
                Err(error) => {
                    drop(reader);
                    let error_kind = NetworkErrorKind::Redirect(error);
                    log_network_error(request_id, kind, "redirect", error_kind);
                    callback(NetEvent::Error {
                        request_id,
                        url,
                        error_kind,
                        response: Some(response_info),
                        body: Some(zero),
                        error: error.to_string(),
                    });
                    return;
                }
                Ok(Some((target, hop))) => {
                    let next = request
                        .for_redirect(target)
                        .expect("immutable admitted redirect target");
                    log_redirect_decision(request_id, kind, history.len(), hop.status(), "pending");
                    last_response = Some(response_info);
                    // No extra body-reader consumption or decoder construction.
                    // Head acquisition may already have buffered body bytes.
                    // Unfinished readers drop their connection, without draining.
                    drop(reader);
                    request = next;
                    pending = Some(hop);
                }
                Ok(None) => {
                    if let Some(status) = response_info.status().filter(|s| s.is_redirection()) {
                        log_redirect_decision(request_id, kind, history.len(), status, "selected");
                    }
                    break AcquiredHttpResponse {
                        metadata: response_info,
                        reader,
                    };
                }
            }
        };

        let null_body = response_info
            .status()
            .is_some_and(HttpStatus::has_null_body);
        let coding = if null_body {
            // 204/304 are head-terminated; 205 verifies framed raw content.
            // Representation metadata never selects a decoder for these statuses.
            Ok(ContentCoding::Identity)
        } else {
            ContentCoding::admit(response_info.headers())
        };
        let coding = match coding {
            Ok(coding) => coding,
            Err(error_kind) => {
                log_network_error(request_id, kind, "admission", error_kind);
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind,
                    response: Some(response_info),
                    body: Some(BodyProgress::try_new(kind, 0).expect("zero body progress")),
                    error: if error_kind == NetworkErrorKind::MalformedResponse {
                        "malformed response content coding"
                    } else {
                        "unsupported response operation"
                    }
                    .into(),
                });
                return;
            }
        };

        callback(NetEvent::Start {
            request_id,
            response: response_info.clone(),
        });

        // 205 has no content but still owns its actual framing reader. 204/304
        // are head-terminated and must never drain a nonexistent message body.
        let transfer = if cancel_token.load(Ordering::Relaxed) {
            // flate2 parses the first header during construction. Preserve the
            // existing cancellation checkpoint before that first source read.
            Err(StreamFailure {
                kind: NetworkErrorKind::Cancelled,
                body: BodyProgress::try_new(kind, 0).expect("zero body progress"),
                error: "cancelled".into(),
            })
        } else if response_info.status_code() == Some(205) {
            verify_empty_body(kind, &cancel_token, &mut reader)
        } else if response_info
            .status()
            .is_some_and(HttpStatus::has_null_body)
        {
            drop(reader);
            stream_reader(
                request_id,
                &url,
                kind,
                &cancel_token,
                &callback,
                &mut std::io::empty(),
            )
        } else {
            let mut reader = http_body_reader(reader, kind, coding);
            stream_reader(
                request_id,
                &url,
                kind,
                &cancel_token,
                &callback,
                &mut reader,
            )
        };
        let body = match transfer {
            Ok(body) => body,
            Err(failure) => {
                log_network_error(request_id, kind, "body", failure.kind);
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind: failure.kind,
                    response: Some(response_info),
                    body: Some(failure.body),
                    error: failure.error,
                });
                return;
            }
        };
        callback(NetEvent::Done {
            request_id,
            response: response_info,
            body,
        });
    });
    Ok(())
}

fn admission_kind(error: ResponseAdmissionError) -> NetworkErrorKind {
    match error {
        ResponseAdmissionError::InformationalResponseUnsupported
        | ResponseAdmissionError::HeaderValuesUnavailable
        | ResponseAdmissionError::UnsupportedResponseScheme => {
            NetworkErrorKind::UnsupportedResponse
        }
        ResponseAdmissionError::Headers(
            crate::HeaderError::TooManyFields
            | crate::HeaderError::NameTooLong
            | crate::HeaderError::ValueTooLong
            | crate::HeaderError::ListTooLarge,
        ) => NetworkErrorKind::ResourceLimit,
        _ => NetworkErrorKind::MalformedResponse,
    }
}

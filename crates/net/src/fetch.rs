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
    log::log_network_error,
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
        if cancel_token.load(Ordering::Relaxed) {
            callback(NetEvent::Error {
                request_id,
                url,
                error_kind: NetworkErrorKind::Cancelled,
                response: None,
                body: None,
                error: "cancelled".into(),
            });
            return;
        }
        let AcquiredHttpResponse {
            metadata: response_info,
            mut reader,
        } = match backend.acquire(&request, kind) {
            Ok(acquired) => acquired,
            Err(failure) => {
                let (error_kind, error) = match failure {
                    HttpAcquireError::Response(error) => (admission_kind(error), error.to_string()),
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
                    response: None,
                    body: None,
                    error,
                });
                return;
            }
        };
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

        let null_body = response_info
            .status()
            .is_some_and(HttpStatus::has_null_body);
        let coding = if response_info.status().is_some_and(HttpStatus::is_redirect)
            && response_info.headers().values("location").next().is_some()
        {
            Err(NetworkErrorKind::UnsupportedResponse)
        } else if null_body {
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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::{HttpStatus, NetworkErrorKind, ResponseAdmissionError};
use core_types::ResourceKind;

use crate::{
    FetchRequest, HttpClientPolicy, NetEvent, RequestAdmissionError,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend, UreqBackend},
    file::fetch_file_url,
    stream::stream_reader,
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
                    HttpAcquireError::Network { kind, error } => (kind, error),
                };
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

        callback(NetEvent::Start {
            request_id,
            response: response_info.clone(),
        });

        // Null-body completion describes consumer-visible semantics, not an
        // independent framing or connection-reuse qualification of ureq.
        let transfer = if response_info
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

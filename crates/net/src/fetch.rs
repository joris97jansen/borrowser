use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::{
    HttpHeaders, HttpStatus, NetworkErrorKind, NetworkResponseInfo, ResponseAdmissionError,
};
use core_types::ResourceKind;

use crate::{
    FetchRequest, HttpClientPolicy, NetEvent, RequestAdmissionError, agent::agent_for_policy,
    file::fetch_file_url, log::log_network_error, stream::stream_reader,
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
    request.validate_route(kind)?;
    let url = request.legacy_issued_url(kind)?;
    let request_id = request.request_id();
    let agent = agent_for_policy(&policy);

    thread::spawn(move || {
        if cancel_token.load(Ordering::Relaxed) {
            callback(NetEvent::Error {
                request_id,
                url: url.clone(),
                error_kind: NetworkErrorKind::Cancelled,
                response: None,
                body: None,
                error: "cancelled".into(),
            });
            return;
        }

        if request.current_url().scheme() == "file" {
            fetch_file_url(&request, &url, kind, &cancel_token, &callback);
            return;
        }

        let response = match build_http_request(&agent, &request, kind).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(err) if err.kind() == ureq::ErrorKind::BadHeader => {
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind: NetworkErrorKind::MalformedResponse,
                    response: None,
                    body: None,
                    error: "malformed HTTP response headers".into(),
                });
                return;
            }
            Err(err) => {
                log_network_error(request_id, kind, &url, "transport", &err.to_string());
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: NetworkErrorKind::Transport,
                    response: None,
                    body: None,
                    error: err.to_string(),
                });
                return;
            }
        };

        let response_info = match response_metadata(&request, &url, &response) {
            Ok(info) => info,
            Err(error) => {
                callback(NetEvent::Error {
                    request_id,
                    url,
                    error_kind: admission_kind(error),
                    response: None,
                    body: None,
                    error: error.to_string(),
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
            drop(response);
            stream_reader(
                request_id,
                &url,
                kind,
                &cancel_token,
                &callback,
                &mut std::io::empty(),
            )
        } else {
            let mut reader = response.into_reader();
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

// The full request stays owned by the worker. Only admitted fields enter ureq;
// initiator/origin facts never implicitly generate policy or credential headers.
// Keep the complete legacy input URL for response metadata compatibility; ureq
// excludes the fragment from the wire request target. AI5 owns backend evolution.
pub(crate) fn build_http_request(
    agent: &ureq::Agent,
    request: &FetchRequest,
    kind: ResourceKind,
) -> ureq::Request {
    let url = request
        .legacy_issued_url(kind)
        .expect("validated execution route");
    let mut built = agent.get(&url);
    for (name, value) in request.headers().iter() {
        let value = std::str::from_utf8(value).expect("admitted ASCII request field");
        built = built.set(name, value);
    }
    built
}

// This projects ureq's exposed list, not wire bytes: ureq trims values and
// removes encoding/length during gzip processing. The pinned parser correction
// rejects malformed field construction before any backend accessor runs.
// Reconstruct interleaved occurrences without sorting, joining, or omission.
pub(crate) fn response_metadata(
    request: &FetchRequest,
    issued_url: &str,
    response: &ureq::Response,
) -> Result<NetworkResponseInfo, ResponseAdmissionError> {
    let status = HttpStatus::try_from(response.status())?;
    if status.is_informational() {
        return Err(ResponseAdmissionError::InformationalResponseUnsupported);
    }
    let names = response.headers_names();
    let mut fields = HttpHeaders::new();
    for (index, name) in names.iter().enumerate() {
        let occurrence = names[..index]
            .iter()
            .filter(|n| n.eq_ignore_ascii_case(name))
            .count();
        let values = response.all(name);
        let count = names
            .iter()
            .filter(|n| n.eq_ignore_ascii_case(name))
            .count();
        if values.len() != count {
            return Err(ResponseAdmissionError::HeaderValuesUnavailable);
        }
        fields
            .append(name, values[occurrence].as_bytes())
            .map_err(ResponseAdmissionError::Headers)?;
    }
    NetworkResponseInfo::http(
        request.original_url().clone(),
        issued_url.to_owned(),
        Some(response.get_url().to_owned()),
        status,
        fields,
    )
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

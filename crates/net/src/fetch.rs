use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};

use crate::{
    FetchRequest, HttpClientPolicy, NetEvent, RequestAdmissionError, agent::agent_for_policy,
    file::fetch_file_url, limits::should_stream_http_status, log::log_network_error,
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
                status_code: None,
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
            Err(ureq::Error::Status(code, response)) if should_stream_http_status(kind, code) => {
                response
            }
            Err(ureq::Error::Status(code, _)) => {
                log_network_error(
                    request_id,
                    kind,
                    &url,
                    "http-status",
                    &format!("HTTP {code}"),
                );
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: NetworkErrorKind::HttpStatus,
                    status_code: Some(code),
                    error: format!("HTTP {code}"),
                });
                return;
            }
            Err(err) => {
                log_network_error(request_id, kind, &url, "transport", &err.to_string());
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: NetworkErrorKind::Transport,
                    status_code: None,
                    error: err.to_string(),
                });
                return;
            }
        };

        let response_info = NetworkResponseInfo {
            requested_url: url.clone(),
            final_url: response.get_url().to_string(),
            status_code: Some(response.status()),
            content_type: response.header("Content-Type").map(ToOwned::to_owned),
        };

        callback(NetEvent::Start {
            request_id,
            response: response_info.clone(),
        });

        let mut reader = response.into_reader();
        let bytes_received = match stream_reader(
            request_id,
            &url,
            kind,
            &cancel_token,
            &callback,
            &mut reader,
        ) {
            Ok(total) => total,
            Err(kind_and_error) => {
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: kind_and_error.0,
                    status_code: response_info.status_code,
                    error: kind_and_error.1,
                });
                return;
            }
        };

        callback(NetEvent::Done {
            request_id,
            response: response_info,
            bytes_received,
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

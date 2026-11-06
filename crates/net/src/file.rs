use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};

use crate::{
    FetchRequest, NetEvent, content_type::guess_content_type_from_path, stream::stream_reader,
};

pub(crate) fn fetch_file_url(
    request: &FetchRequest,
    url: &str,
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    callback: &Arc<dyn Fn(NetEvent) + Send + Sync>,
) {
    let request_id = request.request_id();
    let path = match request.current_url().to_file_path() {
        Ok(path) => path,
        Err(_) => {
            callback(NetEvent::Error {
                request_id,
                url: url.to_owned(),
                error_kind: NetworkErrorKind::LocalFile,
                status_code: None,
                error: "unsupported local file URL".into(),
            });
            return;
        }
    };
    let response = NetworkResponseInfo {
        requested_url: url.to_string(),
        final_url: url.to_string(),
        status_code: None,
        content_type: guess_content_type_from_path(&path),
    };

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => {
            callback(NetEvent::Error {
                request_id,
                url: url.to_string(),
                error_kind: NetworkErrorKind::LocalFile,
                status_code: None,
                error: "local file open failed".into(),
            });
            return;
        }
    };

    callback(NetEvent::Start {
        request_id,
        response: response.clone(),
    });

    let bytes_received =
        match stream_reader(request_id, url, kind, cancel_token, callback, &mut file) {
            Ok(total) => total,
            Err(kind_and_error) => {
                callback(NetEvent::Error {
                    request_id,
                    url: url.to_string(),
                    error_kind: kind_and_error.0,
                    status_code: None,
                    error: kind_and_error.1,
                });
                return;
            }
        };

    callback(NetEvent::Done {
        request_id,
        response,
        bytes_received,
    });
}

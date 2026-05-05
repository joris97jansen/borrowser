use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};

use crate::{NetEvent, content_type::guess_content_type_from_path, stream::stream_reader};

pub(crate) fn is_file_url(url: &str) -> bool {
    url.starts_with("file://")
}

pub(crate) fn fetch_file_url(
    request_id: u64,
    url: &str,
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    callback: &Arc<dyn Fn(NetEvent) + Send + Sync>,
) {
    let mut path_str = url
        .strip_prefix("file://")
        .expect("file fetch called with file URL");
    if cfg!(windows) {
        path_str = path_str.strip_prefix('/').unwrap_or(path_str);
    }

    let path = Path::new(path_str);
    let response = NetworkResponseInfo {
        requested_url: url.to_string(),
        final_url: url.to_string(),
        status_code: None,
        content_type: guess_content_type_from_path(path),
    };

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) => {
            callback(NetEvent::Error {
                request_id,
                url: url.to_string(),
                error_kind: NetworkErrorKind::LocalFile,
                status_code: None,
                error: format!("file open error: {err}"),
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

use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::{NetworkErrorKind, NetworkResponseInfo};
use core_types::ResourceKind;

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
                response: None,
                body: None,
                error: "unsupported local file URL".into(),
            });
            return;
        }
    };
    let response = NetworkResponseInfo::local_file(
        request.original_url().clone(),
        url.to_owned(),
        Some(url.to_owned()),
        guess_content_type_from_path(&path),
    )
    .expect("validated file route and fixed local MIME");

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => {
            callback(NetEvent::Error {
                request_id,
                url: url.to_string(),
                error_kind: NetworkErrorKind::LocalFile,
                response: None,
                body: None,
                error: "local file open failed".into(),
            });
            return;
        }
    };

    callback(NetEvent::Start {
        request_id,
        response: response.clone(),
    });

    let body = match stream_reader(request_id, url, kind, cancel_token, callback, &mut file) {
        Ok(body) => body,
        Err(kind_and_error) => {
            callback(NetEvent::Error {
                request_id,
                url: url.to_string(),
                error_kind: kind_and_error.kind,
                response: Some(response),
                body: Some(kind_and_error.body),
                error: kind_and_error.error,
            });
            return;
        }
    };

    callback(NetEvent::Done {
        request_id,
        response,
        body,
    });
}

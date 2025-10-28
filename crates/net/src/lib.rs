use tools::common::{
    MAX_HTML_BYTES,
};

use std::io::Read;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool,
    Ordering,
};

#[derive(Clone, Copy, Debug)]
pub enum ResourceKind{
    Html,
    Css,
}

pub enum NetEvent {
    Start { request_id: u64, kind: ResourceKind, url: String, content_type: Option<String> },
    Chunk { request_id: u64, kind: ResourceKind, url: String, chunk: Vec<u8> },
    Done { request_id: u64, kind: ResourceKind, url: String },
    Error { request_id: u64, kind: ResourceKind, url: String, error: String },
}

pub fn fetch_stream(
    request_id: u64,
    url: String,
    kind: ResourceKind,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>
) {
    thread::spawn(move || {
        // Check cancel before starting
        if cancel_token.load(Ordering::Relaxed) {
            callback(NetEvent::Error {
                request_id,
                kind,
                url: url.clone(),
                error: "cancelled".into(),
            });
            return;
        }

        // 1) Do request
        let response = match ureq::get(&url).call() {
            Ok(resp) => resp,
            Err(e) => {
                callback(NetEvent::Error {
                    request_id,
                    kind,
                    url: url.clone(),
                    error: e.to_string(),
                });
                return;
            }
        };

        let content_type = response
            .header("Content-Type")
            .map(|s| s.to_string());

        let mut reader = response.into_reader();

        // 2) Emit Start
        callback(NetEvent::Start {
            request_id,
            kind,
            url: url.clone(),
            content_type: content_type,
        });

        // 3) Read in Chunks bytes
        let mut buffer = [0u8; 32 * 1024];
        let mut total: usize = 0;

        loop {
            // Check cancel
            if cancel_token.load(Ordering::Relaxed) {
                callback(NetEvent::Error {
                    request_id,
                    kind,
                    url: url.clone(),
                    error: "cancelled".into(),
                });
                return;
            }

            match reader.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let remaining = MAX_HTML_BYTES.saturating_sub(total);
                    if remaining == 0 {
                        break;
                    }
                    let take = n.min(remaining);
                    if take > 0 {
                        total += take;
                        callback(NetEvent::Chunk {
                            request_id,
                            kind,
                            url: url.clone(),
                            chunk: buffer[..take].to_vec(),
                        });
                    }
                    if n > take || total >= MAX_HTML_BYTES {
                        break;
                    }
                }
                Err(e) => {
                    callback(NetEvent::Error {
                        request_id,
                        kind,
                        url: url.clone(),
                        error: format!("read error: {}", e),
                    });
                    return;
                }
            }
        }

        callback(NetEvent::Done{
            request_id,
            kind,
            url
        });
    });
}


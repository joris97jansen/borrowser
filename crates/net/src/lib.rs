use tools::common::{
    MAX_HTML_BYTES,
};

use std::io::Read;
use std::thread;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub enum ResourceKind{
    Html,
    Css,
}

pub enum NetEvent {
    Start { kind: ResourceKind, url: String, content_type: Option<String> },
    Chunk { kind: ResourceKind, url: String, chunk: Vec<u8> },
    Done { kind: ResourceKind, url: String },
    Error { kind: ResourceKind, url: String, error: String },
}

pub fn fetch_stream(url: String, kind: ResourceKind, callback: Arc<dyn Fn(NetEvent) + Send + Sync>) {
    thread::spawn(move || {
        // 1) Do request
        let response = match ureq::get(&url).call() {
            Ok(resp) => resp,
            Err(e) => {
                callback(NetEvent::Error {
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
            kind,
            url: url.clone(),
            content_type: content_type,
        });

        // 3) Read in Chunks bytes
        let mut buffer = [0u8; 32 * 1024];
        let mut total: usize = 0;

        loop {
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
                        kind,
                        url: url.clone(),
                        error: format!("read error: {}", e),
                    });
                    return;
                }
            }
        }

        callback(NetEvent::Done{
            kind,
            url
        });
    });
}


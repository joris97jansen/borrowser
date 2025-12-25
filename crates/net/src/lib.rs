use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use tools::common::MAX_HTML_BYTES;

pub enum NetEvent {
    Start {
        request_id: u64,
        url: String,
        content_type: Option<String>,
    },
    Chunk {
        request_id: u64,
        url: String,
        chunk: Vec<u8>,
    },
    Done {
        request_id: u64,
        url: String,
    },
    Error {
        request_id: u64,
        url: String,
        error: String,
    },
}

pub fn fetch_stream(
    request_id: u64,
    url: String,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
) {
    thread::spawn(move || {
        // Check cancel before starting
        if cancel_token.load(Ordering::Relaxed) {
            callback(NetEvent::Error {
                request_id,
                url: url.clone(),
                error: "cancelled".into(),
            });
            return;
        }

        // --- file:// Path ---
        if url.starts_with("file://") {
            // Strip the scheme
            let mut path_str = &url["file://".len()..];

            // On Windows, file://C:/path or file:///C:/path might show up;
            // for now we handle the common file:///... by stripping a
            // leading '/'. You can refine this later if needed.
            if cfg!(windows) && path_str.starts_with('/') {
                path_str = &path_str[1..];
            }

            let path = Path::new(path_str);

            let mut file = match File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    callback(NetEvent::Error {
                        request_id,
                        url: url.clone(),
                        error: format!("file open error: {}", e),
                    });
                    return;
                }
            };

            let content_type = guess_content_type_from_path(path);

            // Emit Start
            callback(NetEvent::Start {
                request_id,
                url: url.clone(),
                content_type,
            });

            let mut buffer = [0u8; 32 * 1024];
            let mut total: usize = 0;

            loop {
                // Check cancel
                if cancel_token.load(Ordering::Relaxed) {
                    callback(NetEvent::Error {
                        request_id,
                        url: url.clone(),
                        error: "cancelled".into(),
                    });
                    return;
                }

                match file.read(&mut buffer) {
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
                            url: url.clone(),
                            error: format!("file read error: {}", e),
                        });
                        return;
                    }
                }
            }

            callback(NetEvent::Done { request_id, url });

            return;
        }

        // --- HTTP Path ---
        // 1) Do request
        let response = match ureq::get(&url).call() {
            Ok(resp) => resp,
            Err(e) => {
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error: e.to_string(),
                });
                return;
            }
        };

        let content_type = response.header("Content-Type").map(|s| s.to_string());

        let mut reader = response.into_reader();

        // 2) Emit Start
        callback(NetEvent::Start {
            request_id,
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
                        url: url.clone(),
                        error: format!("read error: {}", e),
                    });
                    return;
                }
            }
        }

        callback(NetEvent::Done { request_id, url });
    });
}

fn guess_content_type_from_path(path: &Path) -> Option<String> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => Some("text/html".to_string()),
        Some("css") => Some("text/css".to_string()),
        Some("js") => Some("application/javascript".to_string()),
        _ => None,
    }
}

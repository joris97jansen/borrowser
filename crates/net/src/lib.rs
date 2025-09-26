use std::thread;
use std::sync::Arc;
pub struct FetchResult {
    pub url: String,
    pub status: Option<u16>,
    pub bytes: usize,
    pub snippet: String,
    pub error: Option<String>,
}

pub fn fetch_text(url: String, cb: Arc<dyn Fn(FetchResult) + Send + Sync>) {
    std::thread::spawn(move || {
        let out = match reqwest::blocking::get(&url) {
            Ok(resp) => {
                let status = resp.status().as_u16();
                match resp.text() {
                    Ok(body) => FetchResult {
                        url,
                        status: Some(status),
                        bytes: body.len(),
                        snippet: body.chars().take(500).collect(),
                        error: None,
                    },
                    Err(e) => FetchResult {
                        url,
                        status: Some(status),
                        bytes: 0,
                        snippet: String::new(),
                        error: Some(format!("Failed to read body: {e}")),
                    },
                }
            }
            Err(e) => FetchResult {
                url,
                status: None,
                bytes: 0,
                snippet: String::new(),
                error: Some(format!("Request failed: {e}")),
            },
        };
        cb(out);
    });
}


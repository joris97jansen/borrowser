use std::thread;
use std::sync::Arc;

pub struct FetchResult {
    pub url: String,          // final URL after redirects
    pub requested_url: String,// what we asked for
    pub status: Option<u16>,
    pub bytes: usize,
    pub snippet: String,
    pub content_type: Option<String>,
    pub duration_ms: u128,
    pub error: Option<String>,
}


pub fn fetch_text(url: String, cb: Arc<dyn Fn(FetchResult) + Send + Sync>) {
    thread::spawn(move || {
        let start = std::time::Instant::now();

        let requested_url = url.clone();

        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("Borrowser/0.1 (+https://borrowser)")
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                cb(FetchResult {
                    requested_url: requested_url.clone(),
                    url: url,
                    status: None,
                    bytes: 0,
                    snippet: String::new(),
                    content_type: None,
                    duration_ms: 0,
                    error: Some(format!("client build error: {e}")),
                });
                return;
            }
        };

        let result = (|| -> Result<FetchResult, String> {
            let mut resp = client.get(&requested_url).send().map_err(|e| e.to_string())?;
            let status = resp.status().as_u16();
            let final_url = resp.url().to_string();
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            use std::io::Read;
            let mut limited = resp.take(64 * 1024);
            let mut buf = Vec::with_capacity(64 * 1024);
            limited.read_to_end(&mut buf).map_err(|e| e.to_string())?;

            let preview = String::from_utf8_lossy(&buf).to_string();

            Ok(FetchResult {
                requested_url: requested_url.clone(),
                url: final_url,
                status: Some(status),
                bytes: buf.len(),
                snippet: preview.chars().take(500).collect(),
                content_type,
                duration_ms: start.elapsed().as_millis(),
                error: None,
            })
        })();

        match result {
            Ok(ok) => cb(ok),
            Err(err) => cb(FetchResult {
                requested_url: requested_url.clone(),
                url: requested_url.clone(),
                status: None,
                bytes: 0,
                snippet: String::new(),
                content_type: None,
                duration_ms: start.elapsed().as_millis(),
                error: Some(err),
        }),
        }
    });
}

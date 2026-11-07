//! Exercise the public typed Fetch handoff, not an independent decoder.
#![cfg(unix)]

use net::NetworkErrorKind;

use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use core_types::ResourceKind as Route;
use net::{FetchRequest, HttpHeaders, NetEvent, RequestMethod};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool, mpsc},
    time::Duration,
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("borrowser-ai2-file-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn execute(input: &str, purpose: Purpose, route: Route) -> Vec<NetEvent> {
    let request = FetchRequest::try_new(
        41,
        ResourceAddress::new(BrowserUrl::parse(input).unwrap().url, purpose),
        RequestMethod::Get,
        HttpHeaders::new(),
        None,
        None,
    )
    .unwrap();
    assert!(request.headers().is_empty());
    let (tx, rx) = mpsc::channel();
    net::fetch_stream(
        request,
        route,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    let mut events = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(event) => events.push(event),
            Err(mpsc::RecvTimeoutError::Disconnected) => return events,
            Err(error) => panic!("local fetch did not terminate: {error}"),
        }
    }
}

#[test]
fn typed_local_file_execution_distinguishes_filenames_and_preserves_provenance() {
    let files = Files::new();
    for (name, body) in [
        ("My Page.html", b"space".as_slice()),
        ("My%20Page.html", b"literal percent"),
        ("café.html", b"unicode"),
    ] {
        std::fs::write(files.0.join(name), body).unwrap();
    }
    let base = format!("file://{}", files.0.display());
    for (source, encoded, expected) in [
        ("My Page.html", "My%20Page.html", b"space".as_slice()),
        ("My%20Page.html", "My%20Page.html", b"space"),
        ("My%2520Page.html", "My%2520Page.html", b"literal percent"),
        ("café.html", "caf%C3%A9.html", b"unicode"),
        ("caf%C3%A9.html", "caf%C3%A9.html", b"unicode"),
    ] {
        for (purpose, route) in [
            (Purpose::Document, Route::Html),
            (Purpose::Stylesheet, Route::Css),
            (Purpose::Image, Route::Image),
        ] {
            let input = format!("{base}/{source}?query=kept#fragment");
            let issued = format!(
                "{base}/{encoded}?query=kept{}",
                if matches!(route, Route::Css) {
                    ""
                } else {
                    "#fragment"
                }
            );
            let events = execute(&input, purpose, route);
            let mut body = Vec::new();
            let mut accepted = None;
            let mut done = false;
            for event in events {
                match event {
                    NetEvent::Start {
                        request_id: 41,
                        response,
                    } => {
                        assert!(accepted.is_none());
                        assert_eq!(response.requested_url(), issued);
                        assert_eq!(response.reported_response_url().unwrap(), issued);
                        assert_eq!(response.status_code(), None);
                        assert_eq!(response.legacy_content_type(), Some("text/html"));
                        accepted = Some(response);
                    }
                    NetEvent::Chunk {
                        request_id: 41,
                        url,
                        chunk,
                    } => {
                        assert!(accepted.is_some());
                        assert_eq!(url, issued);
                        body.extend(chunk);
                    }
                    NetEvent::Done {
                        request_id: 41,
                        response,
                        body: progress,
                    } => {
                        assert_eq!(accepted.as_ref(), Some(&response));
                        assert_eq!(progress.delivered_bytes(), expected.len());
                        assert!(progress.matches_route(route));
                        done = true;
                    }
                    _ => panic!("unexpected synthetic file event"),
                }
            }
            assert!(done);
            assert_eq!(body, expected);
        }
    }
    let events = execute(
        &format!("file://localhost{}/My%20Page.html", files.0.display()),
        Purpose::Document,
        Route::Html,
    );
    assert!(matches!(&events[0], NetEvent::Start { response, .. }
        if response.requested_url() == format!("{base}/My%20Page.html")));
    let body = events
        .iter()
        .filter_map(|event| match event {
            NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(body, b"space");
}

#[test]
fn local_file_conversion_failure_is_terminal_redacted_and_has_no_literal_fallback() {
    // The old loader would open this literal encoded spelling. Conversion must
    // reject NUL, without trying that file or emitting a response start.
    let path = std::env::temp_dir().join(format!("ai2-private-{}%00.html", std::process::id()));
    std::fs::write(&path, b"must not load").unwrap();
    for input in [
        format!("file://{}", path.display()),
        "file://PRIVATE.test/SECRET.html".into(),
    ] {
        let events = execute(&input, Purpose::Document, Route::Html);
        assert_eq!(events.len(), 1);
        let NetEvent::Error {
            request_id: 41,
            url,
            error_kind,
            response,
            body,
            error,
        } = &events[0]
        else {
            panic!("conversion must fail before response start")
        };
        assert_eq!(url, &BrowserUrl::parse(&input).unwrap().url.as_str());
        assert_eq!(*error_kind, NetworkErrorKind::LocalFile);
        assert!(response.is_none());
        assert!(body.is_none());
        assert_eq!(error, "unsupported local file URL");
    }
    std::fs::remove_file(path).unwrap();
    let events = execute(
        "file:///PRIVATE-SECRET/missing.html",
        Purpose::Document,
        Route::Html,
    );
    assert!(
        matches!(&events[..], [NetEvent::Error { error, .. }] if error == "local file open failed")
    );
}

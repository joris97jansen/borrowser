use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use bus::{CoreCommand, CoreEvent};
use core_types::{ResourceKind as Route, StylesheetSlotId};
use net::{FetchRequest, HttpHeaders, RequestMethod};
use std::{sync::mpsc, time::Duration};

fn request(url: &str, purpose: Purpose) -> FetchRequest {
    FetchRequest::try_new(
        17,
        ResourceAddress::new(BrowserUrl::parse(url).unwrap().url, purpose),
        RequestMethod::Get,
        HttpHeaders::new(),
        None,
        None,
    )
    .unwrap()
}

#[test]
fn ai2_invalid_runtime_routes_never_emit_misclassified_events() {
    for (purpose, route, slot) in [
        (Purpose::Stylesheet, Route::Html, None),
        (Purpose::Document, Route::Css, Some(StylesheetSlotId(1))),
        (Purpose::Image, Route::Image, Some(StylesheetSlotId(1))),
        (Purpose::Stylesheet, Route::Css, None),
        (Purpose::Font, Route::Image, None),
    ] {
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        super::start_net_runtime(rx, events);
        tx.send(CoreCommand::FetchStream {
            tab_id: 2,
            stylesheet_slot_id: slot,
            kind: route,
            request: Box::new(request("https://example.invalid/", purpose)),
        })
        .unwrap();
        drop(tx);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap_err(),
            mpsc::RecvTimeoutError::Disconnected
        );
    }
}

#[test]
fn ai2_real_runtime_local_file_handoff_preserves_ids_and_body() {
    let path =
        std::env::temp_dir().join(format!("borrowser-ai2-runtime-{}.html", std::process::id()));
    std::fs::write(&path, b"AI2 local resource").unwrap();
    let url = format!("file://{}", path.display());
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    super::start_net_runtime(rx, events);
    tx.send(
        CoreCommand::try_fetch_stream(2, None, Route::Html, request(&url, Purpose::Document))
            .unwrap(),
    )
    .unwrap();
    drop(tx);
    let mut body = Vec::new();
    let mut started = false;
    loop {
        match received.recv_timeout(Duration::from_secs(5)).unwrap() {
            CoreEvent::NetworkStart {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id: None,
                kind: Route::Html,
                response,
            } => {
                assert!(!started);
                started = true;
                assert_eq!(response.requested_url(), url);
                assert_eq!(response.status_code(), None);
            }
            CoreEvent::NetworkChunk {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id: None,
                kind: Route::Html,
                url: issued,
                bytes,
            } => {
                assert!(started);
                assert_eq!(issued, url);
                body.extend(bytes);
            }
            CoreEvent::NetworkDone {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id: None,
                kind: Route::Html,
                response,
                body: progress,
            } => {
                assert!(started);
                assert_eq!(response.requested_url(), url);
                assert_eq!(body, b"AI2 local resource");
                assert_eq!(progress.delivered_bytes(), body.len());
                assert!(progress.matches_route(Route::Html));
                break;
            }
            event => panic!("unexpected local-file event: {event:?}"),
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn ai6_real_redirect_handoff_preserves_tab_generation_slot_and_original_correlation() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let worker = thread::spawn(move || {
        let mut seen = Vec::new();
        loop {
            let (mut socket, _) = listener.accept().unwrap();
            if stopping.load(Ordering::Acquire) {
                break;
            }
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap()).take(16384);
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            let path = line.split_whitespace().nth(1).unwrap().to_owned();
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
            }
            match path.as_str(){
                "/source?q=1"=>socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /selected?q=2#target\r\nContent-Length: 3\r\nConnection: close\r\n\r\nold").unwrap(),
                "/selected?q=2"=>socket.write_all(b"HTTP/1.1 500 Error\r\nContent-Length: 8\r\nConnection: close\r\n\r\nselected").unwrap(),
                _=>panic!("unexpected request {path}"),
            }
            seen.push(path);
        }
        seen
    });
    struct Cleanup(
        std::net::SocketAddr,
        Arc<AtomicBool>,
        Option<std::thread::JoinHandle<Vec<String>>>,
    );
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Some(worker) = self.2.take() {
                self.1.store(true, Ordering::Release);
                let _ = TcpStream::connect_timeout(&self.0, Duration::from_secs(1));
                let result = worker.join();
                if !thread::panicking() {
                    result.unwrap();
                }
            }
        }
    }
    let mut fixture = Cleanup(addr, stop, Some(worker));
    let source = format!("http://{addr}/source?q=1#original");
    let issued = format!("http://{addr}/source?q=1");
    let selected = format!("http://{addr}/selected?q=2#target");
    let slot = Some(StylesheetSlotId(88));
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    super::start_net_runtime(rx, events);
    tx.send(
        CoreCommand::try_fetch_stream(2, slot, Route::Css, request(&source, Purpose::Stylesheet))
            .unwrap(),
    )
    .unwrap();
    drop(tx);
    let (mut starts, mut terminals) = (0, 0);
    let mut bytes = Vec::new();
    let mut admitted = None;
    loop {
        match received.recv_timeout(Duration::from_secs(5)) {
            Ok(CoreEvent::NetworkStart {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id,
                kind: Route::Css,
                response,
            }) => {
                assert_eq!(stylesheet_slot_id, slot);
                assert_eq!(response.requested_url(), issued);
                assert_eq!(response.original_url().as_str(), source);
                assert_eq!(response.reported_response_url(), Some(selected.as_str()));
                assert_eq!(response.status_code(), Some(500));
                assert_eq!(response.redirect_chain().len(), 1);
                starts += 1;
                admitted = Some(response);
            }
            Ok(CoreEvent::NetworkChunk {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id,
                kind: Route::Css,
                url,
                bytes: chunk,
            }) => {
                assert_eq!(starts, 1);
                assert_eq!(stylesheet_slot_id, slot);
                assert_eq!(url, issued);
                bytes.extend(chunk);
            }
            Ok(CoreEvent::NetworkDone {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id,
                kind: Route::Css,
                response,
                body,
            }) => {
                assert_eq!(stylesheet_slot_id, slot);
                assert_eq!(admitted.as_ref(), Some(&response));
                assert_eq!(body.delivered_bytes(), 8);
                assert!(body.matches_route(Route::Css));
                terminals += 1;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            other => panic!("unexpected runtime event {other:?}"),
        }
    }
    assert_eq!((starts, terminals), (1, 1));
    assert_eq!(bytes, b"selected");
    fixture.1.store(true, Ordering::Release);
    let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(1));
    assert_eq!(
        fixture.2.take().unwrap().join().unwrap(),
        ["/source?q=1", "/selected?q=2"]
    );
}

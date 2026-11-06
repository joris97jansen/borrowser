use super::support::{HttpReply, TestHttpServer, collect_fetch_terminal};
use crate::{NetEvent, NetworkErrorKind, limits::resource_byte_limit, stream::stream_reader};
use core_types::ResourceKind;
use std::io::{self, Read};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn ai3_stream_exact_and_over_limit_counts_every_route() {
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        let cap = resource_byte_limit(route);
        for extra in [0, 1] {
            let total = Arc::new(Mutex::new(0));
            let observed = total.clone();
            let callback: Arc<dyn Fn(NetEvent) + Send + Sync> = Arc::new(move |event| {
                let NetEvent::Chunk { chunk, .. } = event else {
                    panic!("reader only delivers chunks")
                };
                assert!(chunk.iter().all(|&b| b == 0x80));
                *observed.lock().unwrap() += chunk.len();
            });
            let result = stream_reader(
                1,
                "https://fixture.test/",
                route,
                &Arc::new(AtomicBool::new(false)),
                &callback,
                &mut io::repeat(0x80).take((cap + extra) as u64),
            );
            assert_eq!(*total.lock().unwrap(), cap);
            let progress = if extra == 0 {
                result.ok().expect("exact limit completes")
            } else {
                let failure = result.expect_err("over limit fails");
                assert_eq!(failure.kind, NetworkErrorKind::ResourceLimit);
                failure.body
            };
            assert_eq!(progress.delivered_bytes(), cap);
            assert!(progress.matches_route(route));
            assert_eq!(progress.limit_bytes(), cap);
        }
    }
}

struct Interrupted(bool);
impl Read for Interrupted {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.0 {
            return Err(io::Error::other("private backend detail"));
        }
        self.0 = true;
        bytes[..3].copy_from_slice(&[0, 0xff, 0x80]);
        Ok(3)
    }
}

#[test]
fn ai3_stream_partial_interruption_and_typed_cancellation_keep_counts() {
    for cancel in [false, true] {
        let token = Arc::new(AtomicBool::new(false));
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let observed = delivered.clone();
        let flag = token.clone();
        let callback: Arc<dyn Fn(NetEvent) + Send + Sync> = Arc::new(move |event| {
            if let NetEvent::Chunk { chunk, .. } = event {
                observed.lock().unwrap().extend(chunk);
                if cancel {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        });
        let failure = stream_reader(
            1,
            "https://fixture.test/",
            ResourceKind::Css,
            &token,
            &callback,
            &mut Interrupted(false),
        )
        .expect_err("terminal failure");
        assert_eq!(
            failure.kind,
            if cancel {
                NetworkErrorKind::Cancelled
            } else {
                NetworkErrorKind::Read
            }
        );
        assert_eq!(failure.body.delivered_bytes(), 3);
        assert!(failure.body.matches_route(ResourceKind::Css));
        assert_eq!(*delivered.lock().unwrap(), [0, 0xff, 0x80]);
    }
}

#[test]
fn ai3_real_reader_failure_retains_http_error_metadata_and_partial_bytes() {
    let server = TestHttpServer::spawn(|_| {
        HttpReply::truncated("500 Internal Server Error", b"partial".to_vec(), 12)
    });
    let result = collect_fetch_terminal(
        server.url("/truncated"),
        ResourceKind::Html,
        crate::HttpClientPolicy::default(),
    );
    assert_eq!(result.body, b"partial");
    assert!(result.done.is_none());
    let start = result.start.expect("acquired metadata");
    let failure = result.error.expect("read failure");
    assert_eq!(failure.error_kind, NetworkErrorKind::Read);
    assert_eq!(failure.status_code, Some(500));
    assert_eq!(failure.response.as_ref(), Some(&start.response));
    assert_eq!(failure.body.unwrap().delivered_bytes(), result.body.len());
    assert_eq!(
        start.response.headers().values("Content-Length").next(),
        Some(b"12".as_slice())
    );
}

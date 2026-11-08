//! A concrete read-ahead fault fixture at ureq's existing transport seam. It
//! co-delivers a response and unsolicited HTTP bytes in one Read, independently
//! of TCP segmentation. This is HTTP parser/pool qualification, NOT a TLS mock
//! claimed as HTTPS qualification; real TLS persistence is tested in ai5.rs.
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Debug, Default)]
struct Observations {
    connections: AtomicUsize,
    paths: Mutex<Vec<String>>,
}

struct ReadAheadConnector {
    poison: bool,
    status: u16,
    observed: Arc<Observations>,
}

impl ureq::TlsConnector for ReadAheadConnector {
    fn connect(
        &self,
        _: &str,
        socket: Box<dyn ureq::ReadWrite>,
    ) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
        self.observed.connections.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(ReadAheadTransport {
            socket,
            input: Cursor::new(Vec::new()),
            request: Vec::new(),
            poison: self.poison,
            status: self.status,
            observed: self.observed.clone(),
        }))
    }
}

#[derive(Debug)]
struct ReadAheadTransport {
    socket: Box<dyn ureq::ReadWrite>,
    input: Cursor<Vec<u8>>,
    request: Vec<u8>,
    poison: bool,
    status: u16,
    observed: Arc<Observations>,
}

impl Read for ReadAheadTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for ReadAheadTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.request.extend_from_slice(buf);
        if self.request.ends_with(b"\r\n\r\n") {
            let head = std::str::from_utf8(&self.request).unwrap();
            let path = head.split_whitespace().nth(1).unwrap();
            self.observed.paths.lock().unwrap().push(path.into());
            match path {
                "/one" => {
                    let response: &[u8] = match self.status {
                        200 => b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none",
                        204 => b"HTTP/1.1 204 No Content\r\n\r\n",
                        205 => b"HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
                        304 => b"HTTP/1.1 304 Not Modified\r\nContent-Length: 123\r\n\r\n",
                        _ => panic!("unsupported fixture status"),
                    };
                    self.input.get_mut().extend_from_slice(response);
                    if self.poison {
                        self.input.get_mut().extend_from_slice(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nPOISON",
                        );
                    }
                }
                "/two" => self
                    .input
                    .get_mut()
                    .extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo"),
                _ => panic!("unexpected request in read-ahead fixture"),
            }
            self.request.clear();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ureq::ReadWrite for ReadAheadTransport {
    fn socket(&self) -> Option<&TcpStream> {
        self.socket.socket()
    }

    fn is_idle_for_reuse(&mut self) -> io::Result<bool> {
        if self.input.position() != self.input.get_ref().len() as u64 {
            return Ok(false);
        }
        self.socket.is_idle_for_reuse()
    }
}

fn bodies(poison: bool, consume_first: bool, status: u16) -> (Vec<u8>, usize) {
    // No server writes to this socket: kernel peek is quiet while the production
    // HTTP BufReader demonstrably owns the co-delivered bytes. No sleeps/races.
    // Keeping the listener alive allows ureq's actual loopback TCP connects.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let observed = Arc::new(Observations::default());
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .tls_connector(Arc::new(ReadAheadConnector {
            poison,
            status,
            observed: observed.clone(),
        }))
        .build();
    let first = agent.get(&format!("https://{addr}/one")).call().unwrap();
    assert_eq!(first.status(), status);
    if consume_first {
        let mut body = Vec::new();
        first.into_reader().read_to_end(&mut body).unwrap();
        assert_eq!(
            body,
            if status == 200 {
                b"one".as_slice()
            } else {
                b""
            }
        );
    } else {
        // Construction is not consumer completion, even when the library has
        // already copied a small body into a Cursor. Abandon this acquisition.
        drop(first);
    }
    let second = agent.get(&format!("https://{addr}/two")).call().unwrap();
    assert_eq!(second.status(), 200);
    let mut body = Vec::new();
    second.into_reader().read_to_end(&mut body).unwrap();
    assert_eq!(*observed.paths.lock().unwrap(), ["/one", "/two"]);
    (body, observed.connections.load(Ordering::SeqCst))
}

#[test]
fn known_buffered_http_bytes_cannot_become_the_next_response() {
    let (second, connections) = bodies(true, true, 200);
    assert_eq!(
        second, b"two",
        "an unsolicited buffered response contaminated the next request"
    );
    assert_eq!(
        connections, 2,
        "the contaminated connection must be discarded"
    );
}

#[test]
fn completed_clean_read_ahead_transport_remains_reusable() {
    // This control prevents isolation tests becoming vacuous if the future
    // transport-eligibility hook simply disables reuse for this fixture. Once
    // that hook exists, this inspectable Cursor transport must implement it.
    let (second, connections) = bodies(false, true, 200);
    assert_eq!(second, b"two");
    assert_eq!(connections, 1);
}

#[test]
fn unread_small_response_must_not_be_pooled_during_construction() {
    let (second, connections) = bodies(false, false, 200);
    assert_eq!(second, b"two");
    assert_eq!(
        connections, 2,
        "an abandoned acquisition was pooled during response construction"
    );
}

macro_rules! null_body_contamination {
    ($name:ident, $status:literal) => {
        #[test]
        fn $name() {
            let (second, connections) = bodies(true, true, $status);
            assert_eq!(
                second, b"two",
                "a null-body reader cannot prove absence of surplus wire bytes"
            );
            assert_eq!(connections, 2, "known surplus bytes must disqualify reuse");
        }
    };
}

null_body_contamination!(status_204_cannot_hide_buffered_contamination, 204);
null_body_contamination!(status_205_cannot_hide_buffered_contamination, 205);
null_body_contamination!(status_304_cannot_hide_buffered_contamination, 304);

//! AI5 Step C exercises the selected, patched production HTTP readers.
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

const LINE: usize = 8 * 1024;
const TRAILERS: usize = 64 * 1024;
const TRAILER_FIELDS: usize = 64;
const FRAMING: usize = 1024 * 1024;

fn response(body: &str) -> ureq::Response {
    format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Head: retained\r\n\r\n{body}")
        .parse()
        .unwrap()
}

fn read(body: &str) -> (io::Result<usize>, Vec<u8>) {
    let mut bytes = Vec::new();
    let result = response(body).into_reader().read_to_end(&mut bytes);
    (result, bytes)
}

fn limit(error: &io::Error) {
    let source = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<ureq::Error>())
        .expect("typed backend limit");
    assert_eq!(source.kind(), ureq::ErrorKind::BodyMetadataTooLarge);
    assert!(!error.to_string().contains("PRIVATE"));
}

#[test]
fn ai5c_payload_is_returned_before_later_framing_errors() {
    for wire in [
        "3\r\nabcX\n0\r\n\r\n",
        "3\r\nabc\rX0\r\n\r\n",
        "3\r\nabc\r\n",
        "3\r\nabc\r\n0\r\nX: partial",
        "5\r\nabc",
    ] {
        let (result, bytes) = read(wire);
        assert!(result.is_err(), "{wire:?}");
        assert_eq!(bytes, b"abc");
    }
}

#[test]
fn ai5c_chunk_extensions_follow_token_quoted_string_and_bws_grammar() {
    for extension in [
        ";flag",
        ";name=token",
        ";name=\"a,b\\\"c\"",
        " \t; flag \t; name \t= \t\"value\"",
        ";name=\"\u{00ff}\"",
    ] {
        let (result, bytes) = read(&format!("3{extension}\r\nabc\r\n0;end\r\n\r\n"));
        result.unwrap();
        assert_eq!(bytes, b"abc");
    }
    for line in [
        " 3",
        "+3",
        "-3",
        "0x3",
        "3 ",
        "3;",
        "3;;flag",
        "3;bad name=x",
        "3;name=",
        "3;name=\"unterminated",
        "3;name=\"x\"junk",
        "3;name=\"\0\"",
        "3;name=\"x\\\0\"",
    ] {
        let (result, bytes) = read(&format!("{line}\r\nabc\r\n0\r\n\r\n"));
        assert_eq!(
            result.unwrap_err().kind(),
            io::ErrorKind::InvalidData,
            "{line:?}"
        );
        assert!(bytes.is_empty());
    }
}

#[test]
fn ai5c_chunk_lengths_are_checked_u64_without_declared_size_allocation() {
    for size in ["100000000", "ffffffffffffffff"] {
        let (result, bytes) = read(&format!("{size}\r\nabc"));
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(bytes, b"abc");
    }
    let (result, bytes) = read("10000000000000000\r\nabc");
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(bytes.is_empty());
}

#[test]
fn ai5c_trailers_are_validated_discarded_and_never_merged_into_headers() {
    let acquired = response(
        "3\r\nabc\r\n0\r\nX-Trailer: value\r\nContent-Digest: ignored\r\nX-Trailer: second\r\n\r\n",
    );
    assert_eq!(acquired.header("x-head"), Some("retained"));
    assert!(!acquired.has("x-trailer"));
    let mut bytes = Vec::new();
    acquired.into_reader().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"abc");
    for field in [
        "Content-Length: 0",
        "Transfer-Encoding: chunked",
        "HOST: example.test",
        "Connection: close",
        "Trailer: X",
        "Content-Type: text/plain",
        "Content-Encoding: gzip",
        "Content-Range: bytes 0-1/2",
        "Location: /new",
        "Cache-Control: no-store",
        "Set-Cookie: private",
        "Authorization: private",
        "Bad Name: private",
        " X: private",
        "X: private\0",
    ] {
        let (result, bytes) = read(&format!("3\r\nabc\r\n0\r\n{field}\r\n\r\n"));
        let error = result.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains("private"));
        assert_eq!(bytes, b"abc");
    }
}

#[test]
fn ai5c_size_extension_and_trailer_lines_have_independent_exact_bounds() {
    for extra in [0, 1] {
        let wire = format!("3;x={}\r\nabc\r\n0\r\n\r\n", "a".repeat(LINE - 6 + extra));
        let (result, bytes) = read(&wire);
        if extra == 0 {
            result.unwrap();
            assert_eq!(bytes, b"abc");
        } else {
            limit(&result.unwrap_err());
            assert!(bytes.is_empty());
        }
        let wire = format!(
            "3\r\nabc\r\n0\r\nX: PRIVATE{}\r\n\r\n",
            "a".repeat(LINE - "X: PRIVATE\r\n".len() + extra)
        );
        let (result, bytes) = read(&wire);
        if extra == 0 {
            result.unwrap();
        } else {
            limit(&result.unwrap_err());
        }
        assert_eq!(bytes, b"abc");
    }
}

#[test]
fn ai5c_trailer_section_exact_and_excess_budgets_are_independent_of_line_limit() {
    // Keep every line strictly below 8 KiB and the field count below 64, so
    // this isolates the aggregate byte budget at both boundaries.
    let mut section = format!("X: {}\r\n", "a".repeat(7 * 1024 - 5)).repeat(8);
    let padding = TRAILERS - section.len() - "X: \r\n\r\n".len();
    section.push_str(&format!("X: {}\r\n\r\n", "a".repeat(padding)));
    assert_eq!(section.len(), TRAILERS);
    assert!(section.split_inclusive('\n').all(|line| line.len() < LINE));
    read(&format!("0\r\n{section}")).0.unwrap();
    section.insert(section.len() - 4, 'a');
    assert_eq!(section.len(), TRAILERS + 1);
    assert!(section.split_inclusive('\n').all(|line| line.len() < LINE));
    limit(&read(&format!("0\r\n{section}")).0.unwrap_err());
}

#[test]
fn ai5c_trailer_field_count_excludes_the_final_empty_line() {
    for count in [TRAILER_FIELDS, TRAILER_FIELDS + 1] {
        let section = format!("{}\r\n", "X: PRIVATE\r\n".repeat(count));
        assert!(section.len() < TRAILERS);
        assert!(section.split_inclusive('\n').all(|line| line.len() < LINE));
        let mut reader = response(&format!("3\r\nabc\r\n0\r\n{section}")).into_reader();
        let mut bytes = Vec::new();
        let result = reader.read_to_end(&mut bytes);
        if count == TRAILER_FIELDS {
            result.unwrap();
        } else {
            limit(&result.unwrap_err());
            assert!(
                reader.read(&mut [0]).is_err(),
                "limit failure stays latched"
            );
        }
        assert_eq!(bytes, b"abc");
    }
    // Reaching the count limit is not itself verified trailer termination.
    let (result, bytes) = read(&format!(
        "3\r\nabc\r\n0\r\n{}",
        "X: valid\r\n".repeat(TRAILER_FIELDS)
    ));
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(bytes, b"abc");
}

#[test]
fn ai5c_cumulative_framing_budget_counts_delimiters_and_final_termination() {
    let full_chunks = (FRAMING - 5) / (LINE + 2);
    let payload_bytes = full_chunks + 1;
    let mut wire = format!("1;x={}\r\na\r\n", "a".repeat(LINE - 6)).repeat(full_chunks);
    let remainder = FRAMING - full_chunks * (LINE + 2) - 5;
    wire.push_str(&format!(
        "1;x={}\r\na\r\n0\r\n\r\n",
        "a".repeat(remainder - 8)
    ));
    assert_eq!(wire.len() - payload_bytes, FRAMING);
    assert!(wire.split_inclusive('\n').all(|line| line.len() <= LINE));
    let (result, bytes) = read(&wire);
    result.unwrap();
    assert_eq!(bytes, vec![b'a'; payload_bytes]);
    // Add one byte to the last extension, preserving each line's own budget.
    wire.insert(full_chunks * (LINE + 3) + 4, 'a');
    assert_eq!(wire.len() - payload_bytes, FRAMING + 1);
    assert!(wire.split_inclusive('\n').all(|line| line.len() <= LINE));
    let (result, bytes) = read(&wire);
    limit(&result.unwrap_err());
    assert_eq!(bytes, vec![b'a'; payload_bytes]);
}

// Concrete production-transport fixture for byte fragmentation and an injected
// read error. No TLS handshake is simulated or claimed by this fixture.
struct FragmentedConnector {
    wire: Vec<u8>,
    reset: bool,
    reads: Arc<AtomicUsize>,
    listener: Arc<TcpListener>,
}

impl ureq::TlsConnector for FragmentedConnector {
    fn connect(
        &self,
        _: &str,
        socket: Box<dyn ureq::ReadWrite>,
    ) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
        Ok(Box::new(FragmentedTransport {
            input: Cursor::new(self.wire.clone()),
            socket,
            reset: self.reset,
            interrupt: false,
            reads: self.reads.clone(),
            _listener: self.listener.clone(),
        }))
    }
}

#[derive(Debug)]
struct FragmentedTransport {
    input: Cursor<Vec<u8>>,
    socket: Box<dyn ureq::ReadWrite>,
    reset: bool,
    interrupt: bool,
    reads: Arc<AtomicUsize>,
    // Keep the peer's listening socket alive for the entire reader lifetime.
    // Closing it after acquisition resets the unaccepted TCP connection and
    // can make real socket timeout operations fail before the injected read.
    _listener: Arc<TcpListener>,
}

impl Read for FragmentedTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.interrupt = !self.interrupt;
        if self.interrupt {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        if self.reset && self.input.position() == self.input.get_ref().len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected read failure",
            ));
        }
        self.input.read(&mut buf[..1])
    }
}

impl Write for FragmentedTransport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ureq::ReadWrite for FragmentedTransport {
    fn socket(&self) -> Option<&TcpStream> {
        self.socket.socket()
    }
}

fn fragmented(wire: String, reset: bool, reads: Arc<AtomicUsize>) -> ureq::Response {
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0").unwrap());
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .tls_connector(Arc::new(FragmentedConnector {
            wire: wire.into_bytes(),
            reset,
            reads,
            listener: listener.clone(),
        }))
        .build();
    agent
        .get(&format!("https://{}/", listener.local_addr().unwrap()))
        .call()
        .unwrap()
}

#[test]
fn ai5c_fragmented_reads_empty_buffers_and_verified_eof_keep_distinct_meanings() {
    for (fields, wire_body) in [
        ("Content-Length: 3", "abc"),
        (
            "Transfer-Encoding: chunked",
            "3;x=\"a,b\"\r\nabc\r\n0\r\nX: value\r\n\r\n",
        ),
    ] {
        let reads = Arc::new(AtomicUsize::new(0));
        let acquired = fragmented(
            format!("HTTP/1.1 200 OK\r\n{fields}\r\n\r\n{wire_body}"),
            false,
            reads.clone(),
        );
        let mut reader = acquired.into_reader();
        let before = reads.load(Ordering::SeqCst);
        assert_eq!(reader.read(&mut []).unwrap(), 0);
        assert_eq!(reads.load(Ordering::SeqCst), before);
        let mut bytes = [0; 3];
        reader.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, *b"abc");
        let before = reads.load(Ordering::SeqCst);
        assert_eq!(reader.read(&mut []).unwrap(), 0);
        assert_eq!(reads.load(Ordering::SeqCst), before);
        let mut remaining = Vec::new();
        reader.read_to_end(&mut remaining).unwrap();
        assert!(remaining.is_empty());
        assert_eq!(reader.read(&mut bytes).unwrap(), 0);
    }
}

#[test]
fn ai5c_fragmented_transport_failure_is_not_normal_eof_or_lost_payload() {
    for fields in ["Content-Length: 5", "Transfer-Encoding: chunked"] {
        let body = if fields.starts_with("Content") {
            "abc"
        } else {
            "5\r\nabc"
        };
        let mut reader = fragmented(
            format!("HTTP/1.1 200 OK\r\n{fields}\r\n\r\n{body}"),
            true,
            Arc::new(AtomicUsize::new(0)),
        )
        .into_reader();
        let mut bytes = Vec::new();
        assert_eq!(
            reader.read_to_end(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::ConnectionReset
        );
        assert_eq!(bytes, b"abc");
        assert!(
            reader.read(&mut [0]).is_err(),
            "a failed reader must not later claim EOF"
        );
    }
}

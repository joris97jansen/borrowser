use crate::{
    BodyProgress, NetEvent, NetworkErrorKind,
    body::ContentCoding,
    limits::{encoded_byte_limit, resource_byte_limit},
};
use core_types::ResourceKind;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Concrete body causes only. Display never includes an underlying diagnostic;
/// the owned I/O cause remains inspectable without erasing its type.
#[derive(Debug)]
enum HttpBodyError {
    EncodedLimit,
    SourceRead(io::Error),
    Gzip(io::Error),
}
impl std::fmt::Display for HttpBodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::EncodedLimit => "encoded HTTP body limit exceeded",
            Self::SourceRead(_) => "HTTP body source read failed",
            Self::Gzip(_) => "invalid or incomplete gzip content",
        })
    }
}
impl std::error::Error for HttpBodyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceRead(e) | Self::Gzip(e) => Some(e),
            Self::EncodedLimit => None,
        }
    }
}
impl HttpBodyError {
    fn into_io(self) -> io::Error {
        io::Error::other(self)
    }
}

struct EncodedReader<R> {
    source: R,
    remaining: usize,
    failed: bool,
    exceeded: bool,
}
impl<R: Read> EncodedReader<R> {
    fn new(source: R, limit: usize) -> Self {
        Self {
            source,
            remaining: limit,
            failed: false,
            exceeded: false,
        }
    }
}
impl<R: Read> Read for EncodedReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.exceeded {
            return Err(HttpBodyError::EncodedLimit.into_io());
        }
        if self.failed {
            return Err(HttpBodyError::SourceRead(io::Error::other(
                "body reader previously failed",
            ))
            .into_io());
        }
        let mut probe = [0];
        let probing = self.remaining == 0;
        let amount = output.len().min(self.remaining);
        let destination = if probing {
            &mut probe[..]
        } else {
            &mut output[..amount]
        };
        let count = loop {
            match self.source.read(destination) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    self.failed = true;
                    return Err(HttpBodyError::SourceRead(error).into_io());
                }
                Ok(count) => break count,
            }
        };
        if probing && count != 0 {
            self.exceeded = true;
            return Err(HttpBodyError::EncodedLimit.into_io());
        }
        self.remaining -= count;
        Ok(count)
    }
}

struct GzipReader<R: Read> {
    decoder: flate2::read::MultiGzDecoder<R>,
    failed: bool,
}
impl<R: Read> GzipReader<R> {
    fn new(source: R) -> Self {
        Self {
            decoder: flate2::read::MultiGzDecoder::new(source),
            failed: false,
        }
    }
}
impl<R: Read> Read for GzipReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.failed {
            return Err(
                HttpBodyError::Gzip(io::Error::other("gzip reader previously failed")).into_io(),
            );
        }
        self.decoder.read(output).map_err(|error| {
            self.failed = true;
            // Pinned flate2 forwards source I/O errors intact, including errors
            // stored during constructor-time header parsing. Preserve that tag
            // through header, deflate, checksum and member-transition reads.
            if error.get_ref().is_some_and(|e| e.is::<HttpBodyError>()) {
                error
            } else {
                HttpBodyError::Gzip(error).into_io()
            }
        })
    }
}

/// Called after shared admission and Start/cancellation handling. This decoder
/// owns the framed source; it never interprets HTTP transfer metadata itself.
pub(crate) fn http_body_reader(
    reader: Box<dyn Read>,
    kind: ResourceKind,
    coding: ContentCoding,
) -> Box<dyn Read> {
    let encoded = EncodedReader::new(reader, encoded_byte_limit(kind));
    match coding {
        ContentCoding::Identity => Box::new(encoded),
        ContentCoding::Gzip => Box::new(GzipReader::new(encoded)),
    }
}

pub(crate) struct StreamFailure {
    pub kind: NetworkErrorKind,
    pub body: BodyProgress,
    pub error: String,
}

fn read_failure(error: io::Error, body: BodyProgress) -> StreamFailure {
    let cause = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<HttpBodyError>());
    let kind = match cause {
        Some(HttpBodyError::EncodedLimit) => {
            NetworkErrorKind::ResourceLimit(crate::ResourceLimitKind::EncodedBody)
        }
        Some(HttpBodyError::SourceRead(source)) => {
            crate::backend::typed_failure(source).unwrap_or(NetworkErrorKind::Read)
        }
        Some(HttpBodyError::Gzip(_)) => NetworkErrorKind::ContentDecoding,
        // Bare readers also serve local files. They provide no HTTP provenance.
        None => NetworkErrorKind::Read,
    };
    StreamFailure {
        kind,
        body,
        error: kind.diagnostic().into(),
    }
}

/// A 205 owns a framing reader but must deliver no content to its consumer.
/// One nonempty read either verifies EOF or observes prohibited content; no
/// payload is forwarded, and all outcomes retain zero delivered progress.
pub(crate) fn verify_empty_body<R: Read>(
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    reader: &mut R,
) -> Result<BodyProgress, StreamFailure> {
    let body = BodyProgress::try_new(kind, 0).expect("zero body progress");
    let mut byte = [0];
    loop {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(StreamFailure {
                kind: NetworkErrorKind::Cancelled,
                body,
                error: "cancelled".into(),
            });
        }
        match reader.read(&mut byte) {
            Ok(0) => return Ok(body),
            Ok(_) => {
                return Err(StreamFailure {
                    kind: NetworkErrorKind::MalformedResponse,
                    body,
                    error: "205 response contained content".into(),
                });
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(read_failure(
                    HttpBodyError::SourceRead(error).into_io(),
                    body,
                ));
            }
        }
    }
}

pub(crate) fn stream_reader<R: Read>(
    request_id: u64,
    url: &str,
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    callback: &Arc<dyn Fn(NetEvent) + Send + Sync>,
    reader: &mut R,
) -> Result<BodyProgress, StreamFailure> {
    let mut buffer = [0_u8; 32 * 1024];
    let mut total = 0_usize;
    let byte_limit = resource_byte_limit(kind);
    loop {
        let progress = || BodyProgress::try_new(kind, total).expect("bounded stream accounting");
        if cancel_token.load(Ordering::Relaxed) {
            return Err(StreamFailure {
                kind: NetworkErrorKind::Cancelled,
                body: progress(),
                error: "cancelled".into(),
            });
        }
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(progress()),
            Ok(n) => {
                let take = n.min(byte_limit - total);
                if take > 0 {
                    total += take;
                    callback(NetEvent::Chunk {
                        request_id,
                        url: url.to_string(),
                        chunk: buffer[..take].to_vec(),
                    });
                }
                if n > take {
                    return Err(StreamFailure {
                        kind: NetworkErrorKind::ResourceLimit(
                            crate::ResourceLimitKind::DecodedBody,
                        ),
                        body: BodyProgress::try_new(kind, total)
                            .expect("bounded stream accounting"),
                        error: format!(
                            "{} response exceeded byte limit of {} bytes",
                            kind.as_str(),
                            byte_limit
                        ),
                    });
                }
            }
            Err(err) => {
                return Err(read_failure(err, progress()));
            }
        }
    }
}

#[cfg(test)]
mod http_body_tests {
    use super::*;
    use std::io::{Cursor, Write};

    #[test]
    fn ai7_bare_and_http_wrapped_source_errors_have_distinct_provenance() {
        for (source, expected) in [
            (io::ErrorKind::TimedOut, NetworkErrorKind::Timeout),
            (
                io::ErrorKind::ConnectionReset,
                NetworkErrorKind::ConnectionFailure,
            ),
            (io::ErrorKind::WouldBlock, NetworkErrorKind::Read),
            (io::ErrorKind::UnexpectedEof, NetworkErrorKind::Read),
            (io::ErrorKind::InvalidData, NetworkErrorKind::Read),
        ] {
            let progress = BodyProgress::try_new(ResourceKind::Html, 3).unwrap();
            let error = || io::Error::new(source, "https://user:password@PRIVATE/");
            assert_eq!(read_failure(error(), progress).kind, NetworkErrorKind::Read);
            let failure = read_failure(HttpBodyError::SourceRead(error()).into_io(), progress);
            assert_eq!(failure.kind, expected);
            assert_eq!(failure.body, progress);
            assert!(!failure.error.contains("PRIVATE"));
            // Exercise the actual shared reader path as well as its projection:
            // the same finite prefix/error is bare for files and tagged for HTTP.
            for http in [false, true] {
                let source = SourceFault {
                    bytes: Cursor::new(b"abc".to_vec()),
                    offset: 3,
                    metadata: false,
                    kind: source,
                };
                let mut reader: Box<dyn Read> = if http {
                    http_body_reader(
                        Box::new(source),
                        ResourceKind::Html,
                        ContentCoding::Identity,
                    )
                } else {
                    Box::new(source)
                };
                let callback: Arc<dyn Fn(NetEvent) + Send + Sync> = Arc::new(|event| {
                    assert!(matches!(event, NetEvent::Chunk { chunk, .. } if chunk == b"abc"));
                });
                let failure = stream_reader(
                    1,
                    "fixture",
                    ResourceKind::Html,
                    &Arc::new(AtomicBool::new(false)),
                    &callback,
                    &mut reader,
                )
                .err()
                .unwrap();
                assert_eq!(
                    failure.kind,
                    if http {
                        expected
                    } else {
                        NetworkErrorKind::Read
                    }
                );
                assert_eq!(failure.body.delivered_bytes(), 3);
            }
        }
        let error = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer),
            )
        };
        let progress = BodyProgress::try_new(ResourceKind::Html, 0).unwrap();
        assert_eq!(read_failure(error(), progress).kind, NetworkErrorKind::Read);
        assert_eq!(
            read_failure(HttpBodyError::SourceRead(error()).into_io(), progress).kind,
            NetworkErrorKind::TlsFailure
        );
    }

    #[test]
    fn ai7_205_source_timeouts_use_http_provenance_and_keep_zero_progress() {
        struct Timeout;
        impl Read for Timeout {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::TimedOut, "PRIVATE"))
            }
        }
        let failure = verify_empty_body(
            ResourceKind::Html,
            &Arc::new(AtomicBool::new(false)),
            &mut Timeout,
        )
        .err()
        .unwrap();
        assert_eq!(failure.kind, NetworkErrorKind::Timeout);
        assert_eq!(failure.body.delivered_bytes(), 0);
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn encoded_identity_limits_probe_real_eof_for_every_route_without_allocating_the_body() {
        for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
            let cap = encoded_byte_limit(route);
            for adjustment in [-1isize, 0, 1] {
                let length = (cap as isize + adjustment) as usize;
                let mut reader = EncodedReader::new(io::repeat(0x80).take(length as u64), cap);
                let mut delivered = 0;
                let mut buffer = [0; 32768];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => {
                            assert!(adjustment <= 0);
                            break;
                        }
                        Ok(n) => {
                            delivered += n;
                            assert!(buffer[..n].iter().all(|b| *b == 0x80));
                        }
                        Err(error) => {
                            assert_eq!(adjustment, 1);
                            assert!(matches!(
                                error.get_ref().unwrap().downcast_ref::<HttpBodyError>(),
                                Some(HttpBodyError::EncodedLimit)
                            ));
                            assert_eq!(
                                read_failure(error, BodyProgress::try_new(route, 0).unwrap()).kind,
                                NetworkErrorKind::ResourceLimit(
                                    crate::ResourceLimitKind::EncodedBody
                                )
                            );
                            assert!(reader.read(&mut buffer).is_err());
                            break;
                        }
                    }
                }
                assert_eq!(delivered, length.min(cap));
                assert_eq!(reader.read(&mut []).unwrap(), 0);
            }
        }
    }

    struct SourceFault {
        bytes: Cursor<Vec<u8>>,
        offset: usize,
        metadata: bool,
        kind: io::ErrorKind,
    }
    impl Read for SourceFault {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            let position = self.bytes.position() as usize;
            if position == self.offset {
                return Err(if self.metadata {
                    // Obtain the concrete marker from the actual pinned
                    // framing reader, without a public error-construction API.
                    let wire = format!(
                        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0;x={}\r\n\r\n",
                        "a".repeat(8192)
                    );
                    let response: ureq::Response = wire.parse().unwrap();
                    response.into_reader().read(&mut [0]).unwrap_err()
                } else {
                    io::Error::new(self.kind, "https://user:pass@PRIVATE/source")
                });
            }
            let n = buf.len().min(self.offset - position).min(7);
            self.bytes.read(&mut buf[..n])
        }
    }

    #[test]
    fn typed_encoded_excess_survives_gzip_header_deflate_crc_and_member_transitions() {
        let one = gzip(&[b'a'; 4096]);
        let mut two = one.clone();
        two.extend_from_slice(&one);
        for cap in [0, 5, 10, one.len() - 4, one.len(), one.len() + 2] {
            let mut reader = GzipReader::new(EncodedReader::new(Cursor::new(two.clone()), cap));
            let mut delivered = Vec::new();
            let error = reader.read_to_end(&mut delivered).unwrap_err();
            assert!(
                matches!(
                    error.get_ref().unwrap().downcast_ref::<HttpBodyError>(),
                    Some(HttpBodyError::EncodedLimit)
                ),
                "budget {cap}: {error:?}"
            );
            assert_eq!(
                read_failure(
                    error,
                    BodyProgress::try_new(ResourceKind::Html, delivered.len()).unwrap()
                )
                .kind,
                NetworkErrorKind::ResourceLimit(crate::ResourceLimitKind::EncodedBody)
            );
            assert!(
                reader.read(&mut [0]).is_err(),
                "failure must not become successful EOF"
            );
        }
    }

    #[test]
    fn ai7_typed_source_causes_survive_every_gzip_stage_without_becoming_decoding_failures() {
        let one = gzip(&[b'a'; 4096]);
        let mut two = one.clone();
        two.extend_from_slice(&one);
        for (kind, expected) in [
            (io::ErrorKind::TimedOut, NetworkErrorKind::Timeout),
            (
                io::ErrorKind::ConnectionReset,
                NetworkErrorKind::ConnectionFailure,
            ),
            (io::ErrorKind::WouldBlock, NetworkErrorKind::Read),
            (io::ErrorKind::UnexpectedEof, NetworkErrorKind::Read),
            (io::ErrorKind::InvalidData, NetworkErrorKind::Read),
        ] {
            for offset in [0, 10, one.len() - 4, one.len(), one.len() + 2] {
                let source = SourceFault {
                    bytes: Cursor::new(two.clone()),
                    offset,
                    metadata: false,
                    kind,
                };
                let mut reader = GzipReader::new(EncodedReader::new(source, two.len() + 1));
                let mut delivered = Vec::new();
                let error = reader.read_to_end(&mut delivered).unwrap_err();
                let failure = read_failure(
                    error,
                    BodyProgress::try_new(ResourceKind::Html, delivered.len()).unwrap(),
                );
                assert_eq!(failure.kind, expected, "{kind:?} at {offset}");
                assert_eq!(failure.body.delivered_bytes(), delivered.len());
                assert!(!failure.error.contains("PRIVATE"));
                assert!(reader.read(&mut [0]).is_err());
            }
        }
    }

    #[test]
    fn typed_source_failures_survive_all_gzip_read_stages_and_hide_source_diagnostics() {
        let one = gzip(&[b'a'; 4096]);
        let mut two = one.clone();
        two.extend_from_slice(&one);
        for offset in [0, 10, one.len() - 4, one.len(), one.len() + 2] {
            let source = SourceFault {
                bytes: Cursor::new(two.clone()),
                offset,
                metadata: false,
                kind: io::ErrorKind::Other,
            };
            let mut reader = GzipReader::new(EncodedReader::new(source, two.len() + 1));
            let mut delivered = Vec::new();
            let error = reader.read_to_end(&mut delivered).unwrap_err();
            assert!(
                matches!(error.get_ref().unwrap().downcast_ref::<HttpBodyError>(), Some(HttpBodyError::SourceRead(cause)) if cause.kind() == io::ErrorKind::Other)
            );
            let failure = read_failure(
                error,
                BodyProgress::try_new(ResourceKind::Html, delivered.len()).unwrap(),
            );
            assert_eq!(failure.kind, NetworkErrorKind::Read);
            assert_eq!(failure.body.delivered_bytes(), delivered.len());
            assert!(!failure.error.contains("PRIVATE"));
            assert!(!failure.error.contains("user:pass"));
        }
    }

    #[test]
    fn framing_metadata_limits_remain_typed_through_gzip() {
        let bytes = gzip(b"hello");
        for offset in [0, 10, bytes.len()] {
            let source = SourceFault {
                bytes: Cursor::new(bytes.clone()),
                offset,
                metadata: true,
                kind: io::ErrorKind::Other,
            };
            let mut reader = GzipReader::new(EncodedReader::new(source, bytes.len() + 1));
            let mut body = Vec::new();
            let error = reader.read_to_end(&mut body).unwrap_err();
            let failure = read_failure(
                error,
                BodyProgress::try_new(ResourceKind::Html, body.len()).unwrap(),
            );
            assert_eq!(
                failure.kind,
                NetworkErrorKind::ResourceLimit(crate::ResourceLimitKind::FramingMetadata)
            );
            assert!(!failure.error.contains("PRIVATE"));
        }
    }

    #[test]
    fn exact_encoded_budget_truncation_is_a_gzip_cause_not_encoded_excess() {
        let mut bytes = gzip(b"hello");
        bytes.pop();
        let cap = bytes.len();
        let mut reader = GzipReader::new(EncodedReader::new(Cursor::new(bytes), cap));
        let mut body = Vec::new();
        let error = reader.read_to_end(&mut body).unwrap_err();
        assert!(
            matches!(error.get_ref().unwrap().downcast_ref::<HttpBodyError>(), Some(HttpBodyError::Gzip(cause)) if cause.kind() == io::ErrorKind::UnexpectedEof)
        );
        assert_eq!(
            read_failure(
                error,
                BodyProgress::try_new(ResourceKind::Html, body.len()).unwrap()
            )
            .kind,
            NetworkErrorKind::ContentDecoding
        );
    }

    #[test]
    fn identity_source_errors_are_typed_and_latched_without_successful_eof() {
        let mut reader = EncodedReader::new(
            SourceFault {
                bytes: Cursor::new(b"abc".to_vec()),
                offset: 3,
                metadata: false,
                kind: io::ErrorKind::Other,
            },
            3,
        );
        let mut bytes = Vec::new();
        let error = reader.read_to_end(&mut bytes).unwrap_err();
        assert_eq!(bytes, b"abc");
        assert!(matches!(
            error.get_ref().unwrap().downcast_ref::<HttpBodyError>(),
            Some(HttpBodyError::SourceRead(_))
        ));
        assert!(reader.read(&mut [0]).is_err());
        assert_eq!(reader.read(&mut []).unwrap(), 0);
    }
}

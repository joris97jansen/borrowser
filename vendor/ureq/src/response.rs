use std::convert::TryFrom;
use std::io::{self, Read};
use std::net::SocketAddr;
use std::num::NonZeroU64;
use std::str::FromStr;
use std::{fmt, io::BufRead};

use log::debug;
use url::Url;

use crate::body::SizedReader;
use crate::chunked::Decoder as ChunkDecoder;
use crate::error::{Error, ErrorKind::BadStatus};
use crate::header::{get_all_headers, get_header, is_tchar, Header, HeaderLine};
use crate::pool::{PoolReturnRead, PoolReturner};
use crate::stream::{DeadlineStream, ReadOnlyStream, Stream};
use crate::unit::Unit;
use crate::{stream, Agent, ErrorKind};

#[cfg(feature = "json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "charset")]
use encoding_rs::Encoding;

#[cfg(feature = "gzip")]
use flate2::read::MultiGzDecoder;

#[cfg(feature = "brotli")]
use brotli_decompressor::Decompressor as BrotliDecoder;

pub const DEFAULT_CONTENT_TYPE: &str = "text/plain";
pub const DEFAULT_CHARACTER_SET: &str = "utf-8";
const INTO_STRING_LIMIT: usize = 10 * 1_024 * 1_024;
// Wire bytes in one response-head line, including its terminating CRLF. This
// acquisition bound is independent of Borrowser's later typed-field bounds.
const MAX_HEADER_SIZE: usize = 10 * 1_024;
const MAX_HEADER_COUNT: usize = 100;
// Includes the status line, every field line, and the terminating CRLF. This
// bounds acquisition before framing or automatic decoding can inspect fields.
const MAX_RESPONSE_HEAD_SIZE: usize = 64 * 1_024;

#[derive(Copy, Clone, Debug, PartialEq)]
enum ConnectionOption {
    KeepAlive,
    Close,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum BodyType {
    LengthDelimited(u64),
    Chunked,
    CloseDelimited,
}

/// Response instances are created as results of firing off requests.
///
/// The `Response` is used to read response headers and decide what to do with the body.
/// Note that the socket connection is open and the body not read until one of
/// [`into_reader()`](#method.into_reader), [`into_json()`](#method.into_json), or
/// [`into_string()`](#method.into_string) consumes the response.
///
/// When dropping a `Response` instance, one one of two things can happen. If
/// the response has unread bytes, the underlying socket cannot be reused,
/// and the connection is closed. If there are no unread bytes, the connection
/// is returned to the [`Agent`] connection pool used (notice there is always
/// an agent present, even when not explicitly configured by the user).
///
/// ```
/// # fn main() -> Result<(), ureq::Error> {
/// # ureq::is_test(true);
/// let response = ureq::get("http://example.com/").call()?;
///
/// // socket is still open and the response body has not been read.
///
/// let text = response.into_string()?;
///
/// // response is consumed, and body has been read.
/// # Ok(())
/// # }
/// ```
pub struct Response {
    pub(crate) url: Url,
    pub(crate) status_line: String,
    pub(crate) index: ResponseStatusIndex,
    pub(crate) status: u16,
    pub(crate) headers: Vec<Header>,
    pub(crate) reader: Box<dyn Read + Send + Sync + 'static>,
    /// The socket address of the server that sent the response.
    pub(crate) remote_addr: SocketAddr,
    /// The socket address of the client that sent the request.
    pub(crate) local_addr: SocketAddr,
    /// The redirect history of this response, if any. The history starts with
    /// the first response received and ends with the response immediately
    /// previous to this one.
    ///
    /// If this response was not redirected, the history is empty.
    pub(crate) history: Vec<Url>,
}

/// index into status_line where we split: HTTP/1.1 200 OK
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ResponseStatusIndex {
    pub(crate) http_version: usize,
    pub(crate) response_code: usize,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Response[status: {}, status_text: {}, url: {}]",
            self.status(),
            self.status_text(),
            self.url,
        )
    }
}

impl Response {
    /// Construct a response with a status, status text and a string body.
    ///
    /// This is hopefully useful for unit tests.
    ///
    /// Example:
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::Response::new(401, "Authorization Required", "Please log in")?;
    ///
    /// assert_eq!(resp.status(), 401);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(status: u16, status_text: &str, body: &str) -> Result<Response, Error> {
        let r = format!("HTTP/1.1 {} {}\r\n\r\n{}", status, status_text, body);
        (r.as_ref() as &str).parse()
    }

    /// The URL we ended up at. This can differ from the request url when
    /// we have followed redirects.
    pub fn get_url(&self) -> &str {
        &self.url[..]
    }

    /// The http version: `HTTP/1.1`
    pub fn http_version(&self) -> &str {
        &self.status_line.as_str()[0..self.index.http_version]
    }

    /// The status as a u16: `200`
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The status text: `OK`
    ///
    /// The HTTP spec allows for non-utf8 status texts. This uses from_utf8_lossy to
    /// convert such lines to &str.
    pub fn status_text(&self) -> &str {
        self.status_line.as_str()[self.index.response_code + 1..].trim()
    }

    /// The header value for the given name, or None if not found.
    ///
    /// For historical reasons, the HTTP spec allows for header values
    /// to be encoded using encodings like iso-8859-1. Such encodings
    /// means the values are not possible to interpret as utf-8.
    ///
    /// In case the header value can't be read as utf-8, this function
    /// returns `None` (while the name is visible in [`Response::headers_names()`]).
    pub fn header(&self, name: &str) -> Option<&str> {
        get_header(&self.headers, name)
    }

    /// A list of the header names in this response.
    /// Lowercased to be uniform.
    ///
    /// It's possible for a header name to be returned by this function, and
    /// still give a `None` value. See [`Response::header()`] for an explanation
    /// as to why.
    pub fn headers_names(&self) -> Vec<String> {
        self.headers
            .iter()
            .map(|h| h.name().to_lowercase())
            .collect()
    }

    /// Tells if the response has the named header.
    pub fn has(&self, name: &str) -> bool {
        self.header(name).is_some()
    }

    /// All headers corresponding values for the give name, or empty vector.
    pub fn all(&self, name: &str) -> Vec<&str> {
        get_all_headers(&self.headers, name)
    }

    /// Admitted field occurrences in response order, with opaque value bytes.
    /// Only leading/trailing SP and HTAB have been removed from values. This
    /// does not restore fields removed by automatic content decoding.
    pub fn headers_bytes(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.headers
            .iter()
            .map(|header| (header.name(), header.value_raw()))
    }

    /// The content type part of the "Content-Type" header without
    /// the charset.
    ///
    /// Example:
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::get("http://example.com/charset/iso").call()?;
    /// assert_eq!(resp.header("content-type"), Some("text/html; charset=ISO-8859-1"));
    /// assert_eq!("text/html", resp.content_type());
    /// # Ok(())
    /// # }
    /// ```
    pub fn content_type(&self) -> &str {
        self.header("content-type")
            .map(|header| {
                header
                    .find(';')
                    .map(|index| &header[0..index])
                    .unwrap_or(header)
            })
            .unwrap_or(DEFAULT_CONTENT_TYPE)
    }

    /// The character set part of the "Content-Type".
    ///
    /// Example:
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::get("http://example.com/charset/iso").call()?;
    /// assert_eq!(resp.header("content-type"), Some("text/html; charset=ISO-8859-1"));
    /// assert_eq!("ISO-8859-1", resp.charset());
    /// # Ok(())
    /// # }
    /// ```
    pub fn charset(&self) -> &str {
        charset_from_content_type(self.header("content-type"))
    }

    /// The socket address of the server that sent the response.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// The local address the request was made from.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Turn this response into a `impl Read` of the body.
    ///
    /// 1. If `Transfer-Encoding: chunked`, the returned reader will unchunk it
    ///    and any `Content-Length` header is ignored.
    /// 2. If `Content-Length` is set, the returned reader is limited to this byte
    ///    length regardless of how many bytes the server sends.
    /// 3. If no length header, the reader is until server stream end.
    ///
    /// Note: If you use `read_to_end()` on the resulting reader, a malicious
    /// server might return enough bytes to exhaust available memory. If you're
    /// making requests to untrusted servers, you should use `.take()` to
    /// limit the response bytes read.
    ///
    /// Example:
    ///
    /// ```
    /// use std::io::Read;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # ureq::is_test(true);
    /// let resp = ureq::get("http://httpbin.org/bytes/100")
    ///     .call()?;
    ///
    /// assert!(resp.has("Content-Length"));
    /// let len: usize = resp.header("Content-Length")
    ///     .unwrap()
    ///     .parse()?;
    ///
    /// let mut bytes: Vec<u8> = Vec::with_capacity(len);
    /// resp.into_reader()
    ///     .take(10_000_000)
    ///     .read_to_end(&mut bytes)?;
    ///
    /// assert_eq!(bytes.len(), len);
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_reader(self) -> Box<dyn Read + Send + Sync + 'static> {
        self.reader
    }

    // Determine what to do with the connection after we've read the body.
    fn connection_option(
        response_version: &str,
        headers: &[Header],
    ) -> Result<ConnectionOption, Error> {
        let close = crate::header::connection_close(headers)?;
        Ok(
            if response_version.eq_ignore_ascii_case("HTTP/1.0") || close {
                ConnectionOption::Close
            } else {
                ConnectionOption::KeepAlive
            },
        )
    }

    /// Determine how the body should be read, based on
    /// <https://datatracker.ietf.org/doc/html/rfc9112#name-message-body-length>
    fn body_type(
        request_method: &str,
        response_status: u16,
        response_version: &str,
        headers: &[Header],
    ) -> Result<BodyType, Error> {
        if !matches!(response_version, "HTTP/1.0" | "HTTP/1.1") {
            return Err(ErrorKind::UnsupportedResponse.msg("unsupported HTTP response version"));
        }
        if !(100..=599).contains(&response_status) {
            return Err(BadStatus.msg("invalid HTTP response status"));
        }
        if response_status < 200 {
            return Err(
                ErrorKind::UnsupportedResponse.msg("informational responses are unsupported")
            );
        }

        let length = content_length(headers)?;
        let has_transfer = headers
            .iter()
            .any(|header| header.is_name("transfer-encoding"));
        if has_transfer && length.is_some() {
            return Err(ErrorKind::BadHeader.msg("conflicting HTTP response framing fields"));
        }
        if response_status == 204 && (has_transfer || length.is_some()) {
            return Err(ErrorKind::BadHeader.msg("prohibited framing fields on 204 response"));
        }
        if response_status == 205 && length.is_some_and(|length| length != 0) {
            return Err(ErrorKind::BadHeader.msg("nonzero length on 205 response"));
        }
        if has_transfer && response_version == "HTTP/1.0" {
            return Err(ErrorKind::BadHeader.msg("transfer encoding on HTTP/1.0 response"));
        }

        let head_terminated =
            request_method.eq_ignore_ascii_case("head") || matches!(response_status, 204 | 304);
        validate_transfer_encoding(headers, head_terminated)?;
        if head_terminated {
            return Ok(BodyType::LengthDelimited(0));
        }
        // A 205 still owns its framing reader. Consumer-visible no-content
        // semantics do not establish chunk termination or transport EOF.
        Ok(if has_transfer {
            BodyType::Chunked
        } else {
            length
                .map(BodyType::LengthDelimited)
                .unwrap_or(BodyType::CloseDelimited)
        })
    }

    fn stream_to_reader(
        mut stream: DeadlineStream,
        unit: &Unit,
        body_type: BodyType,
        compression: Option<Compression>,
        connection_option: ConnectionOption,
    ) -> Result<Box<dyn Read + Send + Sync + 'static>, Error> {
        if connection_option == ConnectionOption::Close
            || matches!(body_type, BodyType::CloseDelimited)
        {
            stream.inner_mut().set_unpoolable();
        }
        let inner = stream.inner_ref();
        inner.set_read_timeout(unit.agent.config.timeout_read)?;
        let body_reader: Box<dyn Read + Send + Sync> = match body_type {
            // Chunked responses have an unknown length, but do have an end of body
            // marker. When we encounter the marker, we can return the underlying stream
            // to the connection pool.
            BodyType::Chunked => {
                debug!("Chunked body in response");
                Box::new(PoolReturnRead::new(ChunkDecoder::new(stream)))
            }
            // Responses with a content-length header means we should limit the reading
            // of the body to the number of bytes in the header. Once done, we can
            // return the underlying stream to the connection pool.
            BodyType::LengthDelimited(len) => {
                match NonZeroU64::new(len) {
                    None => {
                        debug!("zero-length body returning stream directly to pool");
                        let stream: Stream = stream.into();
                        // The validated head completes this HTTP message. The
                        // optional pool return still checks all observable
                        // buffering and transport eligibility before insertion.
                        stream.return_to_pool();
                        Box::new(std::io::empty())
                    }
                    Some(len) => {
                        // Do not allocate from the declared length or eagerly
                        // copy an already buffered fixed-length body here.
                        Box::new(LimitedRead::new(stream, len))
                    }
                }
            }
            BodyType::CloseDelimited => {
                debug!("Body of unknown size - read until socket close");
                Box::new(stream)
            }
        };

        Ok(match compression {
            None => body_reader,
            Some(c) => c.wrap_reader(body_reader),
        })
    }

    /// Turn this response into a String of the response body. By default uses `utf-8`,
    /// but can work with charset, see below.
    ///
    /// This is potentially memory inefficient for large bodies since the
    /// implementation first reads the reader to end into a `Vec<u8>` and then
    /// attempts to decode it using the charset.
    ///
    /// If the response is larger than 10 megabytes, this will return an error.
    ///
    /// Example:
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let text = ureq::get("http://httpbin.org/get?success")
    ///     .call()?
    ///     .into_string()?;
    ///
    /// assert!(text.contains("success"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Charset support
    ///
    /// If you enable feature `ureq = { version = "*", features = ["charset"] }`, into_string()
    /// attempts to respect the character encoding of the `Content-Type` header. If there is no
    /// Content-Type header, or the Content-Type header does not specify a charset, into_string()
    /// uses `utf-8`.
    ///
    /// I.e. `Content-Type: text/plain; charset=iso-8859-1` would be decoded in latin-1.
    ///
    pub fn into_string(self) -> io::Result<String> {
        #[cfg(feature = "charset")]
        let encoding = Encoding::for_label(self.charset().as_bytes())
            .or_else(|| Encoding::for_label(DEFAULT_CHARACTER_SET.as_bytes()))
            .unwrap();

        let mut buf: Vec<u8> = vec![];
        self.into_reader()
            .take((INTO_STRING_LIMIT + 1) as u64)
            .read_to_end(&mut buf)?;
        if buf.len() > INTO_STRING_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "response too big for into_string",
            ));
        }

        #[cfg(feature = "charset")]
        {
            let (text, _, _) = encoding.decode(&buf);
            Ok(text.into_owned())
        }
        #[cfg(not(feature = "charset"))]
        {
            Ok(String::from_utf8_lossy(&buf).to_string())
        }
    }

    /// Read the body of this response into a serde_json::Value, or any other type that
    /// implements the [serde::Deserialize] trait.
    ///
    /// You must use either a type annotation as shown below (`message: Message`), or the
    /// [turbofish operator] (`::<Type>`) so Rust knows what type you are trying to read.
    ///
    /// [turbofish operator]: https://matematikaadit.github.io/posts/rust-turbofish.html
    ///
    /// Example:
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// // This import requires the `derive` feature on `serde`.
    /// // Put this in Cargo.toml: serde = { version = "1", features = ["derive"] }
    /// use serde::{Deserialize};
    ///
    /// #[derive(Deserialize)]
    /// struct Message {
    ///     text: String,
    /// }
    ///
    /// let message: Message =
    ///     ureq::get("http://example.com/get/hello_world.json")
    ///         .call()?
    ///         .into_json()?;
    ///
    /// assert_eq!(message.text, "Ok");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Or, if you don't want to define a struct to read your JSON into, you can
    /// use the convenient `serde_json::Value` type to parse arbitrary or unknown
    /// JSON.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let json: serde_json::Value = ureq::get("http://example.com/get/hello_world.json")
    ///     .call()?
    ///     .into_json()?;
    ///
    /// assert_eq!(json["text"], "Ok");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "json")]
    pub fn into_json<T: DeserializeOwned>(self) -> io::Result<T> {
        use crate::stream::io_err_timeout;

        let reader = self.into_reader();
        serde_json::from_reader(reader).map_err(|e| {
            // This is to unify TimedOut io::Error in the API.
            if let Some(kind) = e.io_error_kind() {
                if kind == io::ErrorKind::TimedOut {
                    return io_err_timeout(e.to_string());
                }
            }

            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to read JSON: {}", e),
            )
        })
    }

    /// Create a response from a Read trait impl.
    ///
    /// This is hopefully useful for unit tests.
    ///
    /// Example:
    ///
    /// use std::io::Cursor;
    ///
    /// let text = "HTTP/1.1 401 Authorization Required\r\n\r\nPlease log in\n";
    /// let read = Cursor::new(text.to_string().into_bytes());
    /// let resp = ureq::Response::do_from_read(read);
    ///
    /// assert_eq!(resp.status(), 401);
    pub(crate) fn do_from_stream(stream: Stream, unit: Unit) -> Result<Response, Error> {
        let remote_addr = stream.remote_addr;

        let local_addr = match stream.socket() {
            Some(sock) => sock.local_addr().map_err(Error::from)?,
            None => std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), 0).into(),
        };

        //
        // HTTP/1.1 200 OK\r\n
        let mut stream = stream::DeadlineStream::new(stream, unit.deadline);

        // The status line we can ignore non-utf8 chars and parse as_str_lossy().
        let mut remaining_head = MAX_RESPONSE_HEAD_SIZE;
        let status_line = read_next_line(&mut stream, "the status line", &mut remaining_head)?
            .into_string_lossy();
        let (index, status) = parse_status_line(status_line.as_str())?;
        let http_version = &status_line.as_str()[0..index.http_version];

        let mut headers: Vec<Header> = Vec::new();
        loop {
            let line = read_next_line(&mut stream, "a header", &mut remaining_head)?;
            if line.is_empty() {
                break;
            }
            if headers.len() == MAX_HEADER_COUNT {
                return Err(ErrorKind::ResponseHeadTooLarge.msg("too many HTTP response fields"));
            }
            headers.push(line.into_header()?);
        }
        // Framing is authoritative before constructing readers, decoding any
        // body, or allowing a zero-length response to return its connection.
        let body_type = Self::body_type(&unit.method, status, http_version, &headers)?;
        let head_terminated =
            unit.method.eq_ignore_ascii_case("head") || matches!(status, 204 | 304);
        // 205 must verify that its framed HTTP payload is empty. Decoding a
        // nonempty gzip member to zero bytes would conceal prohibited content.
        let compression = if head_terminated || status == 205 {
            None
        } else {
            get_header(&headers, "content-encoding").and_then(Compression::from_header_value)
        };

        let connection_option = Self::connection_option(http_version, &headers)?;

        // remove Content-Encoding and length due to automatic decompression
        if compression.is_some() {
            headers.retain(|h| !h.is_name("content-encoding") && !h.is_name("content-length"));
        }

        let reader =
            Self::stream_to_reader(stream, &unit, body_type, compression, connection_option)?;

        let url = unit.url.clone();

        let response = Response {
            url,
            status_line,
            index,
            status,
            headers,
            reader,
            remote_addr,
            local_addr,
            history: vec![],
        };
        Ok(response)
    }

    #[cfg(test)]
    pub fn set_url(&mut self, url: Url) {
        self.url = url;
    }

    #[cfg(test)]
    pub fn history_from_previous(&mut self, previous: Response) {
        self.history = previous.history;
        self.history.push(previous.url);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Compression {
    #[cfg(feature = "brotli")]
    Brotli,
    #[cfg(feature = "gzip")]
    Gzip,
}

impl Compression {
    /// Convert a string like "br" to an enum value
    fn from_header_value(value: &str) -> Option<Compression> {
        match value {
            #[cfg(feature = "brotli")]
            "br" => Some(Compression::Brotli),
            #[cfg(feature = "gzip")]
            "gzip" | "x-gzip" => Some(Compression::Gzip),
            _ => None,
        }
    }

    /// Wrap the raw reader with a decompressing reader
    #[allow(unused_variables)] // when no features enabled, reader is unused (unreachable)
    pub(crate) fn wrap_reader(
        self,
        reader: Box<dyn Read + Send + Sync + 'static>,
    ) -> Box<dyn Read + Send + Sync + 'static> {
        match self {
            #[cfg(feature = "brotli")]
            Compression::Brotli => Box::new(BrotliDecoder::new(reader, 4096)),
            #[cfg(feature = "gzip")]
            Compression::Gzip => Box::new(MultiGzDecoder::new(reader)),
        }
    }
}

/// parse a line like: HTTP/1.1 200 OK\r\n
fn parse_status_line(line: &str) -> Result<(ResponseStatusIndex, u16), Error> {
    //

    if !line.is_ascii() {
        return Err(BadStatus.msg("Status line not ASCII"));
    }
    // https://tools.ietf.org/html/rfc7230#section-3.1.2
    //      status-line = HTTP-version SP status-code SP reason-phrase CRLF
    let mut split: Vec<&str> = line.splitn(3, ' ').collect();
    if split.len() == 2 {
        // As a special case, we are lenient parsing lines without a space after the code.
        // This is technically against spec. "HTTP/1.1 200\r\n"
        split.push("");
    }
    if split.len() != 3 {
        return Err(BadStatus.msg("Wrong number of tokens in status line"));
    }

    // https://tools.ietf.org/html/rfc7230#appendix-B
    //    HTTP-name = %x48.54.54.50 ; HTTP
    //    HTTP-version = HTTP-name "/" DIGIT "." DIGIT
    let http_version = split[0];
    if !http_version.starts_with("HTTP/") {
        return Err(BadStatus.msg("HTTP version did not start with HTTP/"));
    }
    if http_version.len() != 8 {
        return Err(BadStatus.msg("HTTP version was wrong length"));
    }
    if !http_version.as_bytes()[5].is_ascii_digit()
        || http_version.as_bytes()[6] != b'.'
        || !http_version.as_bytes()[7].is_ascii_digit()
    {
        return Err(BadStatus.msg("HTTP version did not match format"));
    }

    let status_str: &str = split[1];
    if !split[2]
        .bytes()
        .all(|byte| byte == b'\t' || (byte >= b' ' && byte != 0x7f))
    {
        return Err(BadStatus.msg("invalid HTTP reason phrase"));
    }
    //      status-code    = 3DIGIT
    if status_str.len() != 3 || !status_str.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BadStatus.msg("Status code was wrong length"));
    }

    let status: u16 = status_str
        .parse()
        .map_err(|_| BadStatus.msg("invalid HTTP status code"))?;

    Ok((
        ResponseStatusIndex {
            http_version: http_version.len(),
            response_code: http_version.len() + status_str.len(),
        },
        status,
    ))
}

impl FromStr for Response {
    type Err = Error;
    /// Parse a response from a string.
    ///
    /// Example:
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// let s = "HTTP/1.1 200 OK\r\n\
    ///     X-Forwarded-For: 1.2.3.4\r\n\
    ///     Content-Type: text/plain\r\n\
    ///     \r\n\
    ///     Hello World!!!";
    /// let resp: ureq::Response = s.parse()?;
    /// assert!(resp.has("X-Forwarded-For"));
    /// let body = resp.into_string()?;
    /// assert_eq!(body, "Hello World!!!");
    /// # Ok(())
    /// # }
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let remote_addr = "0.0.0.0:0".parse().unwrap();
        let stream = Stream::new(
            ReadOnlyStream::new(s.into()),
            remote_addr,
            PoolReturner::none(),
        );
        let request_url = "https://example.com".parse().unwrap();
        let request_reader = SizedReader {
            size: crate::body::BodySize::Empty,
            reader: Box::new(std::io::empty()),
        };
        let unit = Unit::new(
            &Agent::new(),
            "GET",
            &request_url,
            vec![],
            &request_reader,
            None,
        );
        Self::do_from_stream(stream, unit)
    }
}

fn read_next_line(
    reader: &mut impl BufRead,
    context: &str,
    remaining_head: &mut usize,
) -> Result<HeaderLine, Error> {
    let mut buf = Vec::new();
    let limit = MAX_HEADER_SIZE.min(*remaining_head);
    let n = reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut buf)
        .map_err(|error| {
            ErrorKind::Io
                .msg(format!("error reading {context}"))
                .src(error)
        })?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "unexpected EOF in HTTP response head",
        )
        .into());
    }
    if n > limit {
        return Err(ErrorKind::ResponseHeadTooLarge.msg("HTTP response head limit exceeded"));
    }
    *remaining_head -= n;
    if !buf.ends_with(b"\r\n") {
        return Err(ErrorKind::BadHeader.msg("HTTP response line requires CRLF"));
    }
    buf.truncate(buf.len() - 2);
    Ok(buf.into())
}

// RFC 9112 section 6.3: every occurrence participates in a single framing
// decision. The original fields are retained rather than rewritten or joined.
fn content_length(headers: &[Header]) -> Result<Option<u64>, Error> {
    let mut length = None;
    for header in headers
        .iter()
        .filter(|header| header.is_name("content-length"))
    {
        for value in header.value_raw().split(|byte| *byte == b',') {
            let value = trim_ows(value);
            if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
                return Err(ErrorKind::BadHeader.msg("invalid Content-Length"));
            }
            let parsed = value
                .iter()
                .try_fold(0u64, |n, digit| {
                    n.checked_mul(10)?.checked_add(u64::from(*digit - b'0'))
                })
                .ok_or_else(|| ErrorKind::BadHeader.msg("Content-Length overflow"))?;
            if length.is_some_and(|length| length != parsed) {
                return Err(ErrorKind::BadHeader.msg("conflicting Content-Length values"));
            }
            length = Some(parsed);
        }
    }
    Ok(length)
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while matches!(value.first(), Some(b' ' | b'\t')) {
        value = &value[1..];
    }
    while matches!(value.last(), Some(b' ' | b'\t')) {
        value = &value[..value.len() - 1];
    }
    value
}

// Parse only Transfer-Encoding's concrete token/parameter/list grammar. A
// head-terminated response may retain valid representation metadata without
// requiring support for executing that coding stack.
fn validate_transfer_encoding(headers: &[Header], head_terminated: bool) -> Result<(), Error> {
    fn malformed() -> Error {
        ErrorKind::BadHeader.msg("invalid Transfer-Encoding")
    }
    fn token<'a>(rest: &mut &'a [u8]) -> Result<&'a [u8], Error> {
        let length = rest.iter().take_while(|byte| is_tchar(byte)).count();
        if length == 0 {
            return Err(malformed());
        }
        let (value, remaining) = rest.split_at(length);
        *rest = remaining;
        Ok(value)
    }
    let mut present = false;
    let mut count = 0;
    let mut chunked = false;
    for header in headers
        .iter()
        .filter(|header| header.is_name("transfer-encoding"))
    {
        present = true;
        let mut rest = trim_ows(header.value_raw());
        loop {
            // RFC 9110 section 5.6.1.2: ignore empty list members, including
            // empty field occurrences. The existing 10 KiB line / 64 KiB head
            // budgets bound this allocation-free, linear scan; no additional
            // empty-member budget is needed. Only actual codings are counted.
            if rest.is_empty() {
                break;
            }
            if rest[0] == b',' {
                rest = trim_ows(&rest[1..]);
                continue;
            }
            let coding = token(&mut rest)?;
            let is_chunked = coding.eq_ignore_ascii_case(b"chunked");
            // RFC 9112 section 6.1 prohibits applying chunked more than once.
            // Non-final chunked in a response is instead close-delimited under
            // section 6.3; it is not malformed solely because of that order.
            if is_chunked && chunked {
                return Err(malformed());
            }
            chunked |= is_chunked;
            rest = trim_ows(rest);
            let mut parameters = false;
            while rest.first() == Some(&b';') {
                parameters = true;
                rest = trim_ows(&rest[1..]);
                token(&mut rest)?;
                rest = trim_ows(rest);
                if rest.first() != Some(&b'=') {
                    return Err(malformed());
                }
                rest = trim_ows(&rest[1..]);
                if rest.first() == Some(&b'"') {
                    rest = &rest[1..];
                    loop {
                        match rest.first() {
                            None => return Err(malformed()),
                            Some(b'"') => {
                                rest = &rest[1..];
                                break;
                            }
                            Some(b'\\') => {
                                if rest.len() < 2 {
                                    return Err(malformed());
                                }
                                rest = &rest[2..];
                            }
                            Some(_) => rest = &rest[1..],
                        }
                    }
                } else {
                    token(&mut rest)?;
                }
                rest = trim_ows(rest);
            }
            // Sections 7.1 and 7.2 define no parameters for these codings and
            // recommend treating their presence as an error. Use that strict
            // policy for body-bearing responses and representation metadata.
            if parameters
                && (is_chunked
                    || [
                        b"gzip".as_slice(),
                        b"x-gzip",
                        b"compress",
                        b"x-compress",
                        b"deflate",
                    ]
                    .iter()
                    .any(|name| coding.eq_ignore_ascii_case(name)))
            {
                return Err(malformed());
            }
            count += 1;
            if rest.is_empty() {
                break;
            }
            if rest[0] != b',' {
                return Err(malformed());
            }
            rest = trim_ows(&rest[1..]);
        }
    }
    // An absent field is valid. A present logical list with no actual coding
    // remains rejected under AI5's explicit framing policy.
    if present && count == 0 {
        return Err(malformed());
    }
    if count != 0 && !(head_terminated || count == 1 && chunked) {
        return Err(ErrorKind::UnsupportedResponse.msg("unsupported HTTP transfer coding"));
    }
    Ok(())
}

/// Limits a `Read` to a content size (as set by a "Content-Length" header).
pub(crate) struct LimitedRead<R> {
    reader: Option<R>,
    limit: u64,
    position: u64,
    failed: bool,
}

impl<R: Read + Sized + Into<Stream>> LimitedRead<R> {
    pub(crate) fn new(reader: R, limit: NonZeroU64) -> Self {
        LimitedRead {
            reader: Some(reader),
            limit: limit.get(),
            position: 0,
            failed: false,
        }
    }

    pub(crate) fn remaining(&self) -> u64 {
        self.limit - self.position
    }

    fn return_stream_to_pool(&mut self) {
        if let Some(reader) = self.reader.take() {
            // Transfer ownership for optional qualified reuse. Ineligibility
            // drops the connection without failing the completed response.
            let stream: Stream = reader.into();
            stream.return_to_pool();
        }
    }
}

impl<R: Read + Sized + Into<Stream>> Read for LimitedRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response body previously failed",
            ));
        }
        if self.remaining() == 0 {
            return Ok(0);
        }
        // A declared length can exceed addressable memory. Narrow only the
        // per-call slice bound, never the stored framing length or progress.
        let amount = usize::try_from(self.remaining())
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let from = &mut buf[..amount];
        let reader = match self.reader.as_mut() {
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing response reader",
                ))
            }
            Some(r) => r,
        };
        match reader.read(from) {
            // https://tools.ietf.org/html/rfc7230#page-33
            // If the sender closes the connection or
            // the recipient times out before the indicated number of octets are
            // received, the recipient MUST consider the message to be
            // incomplete and close the connection.
            Ok(0) => {
                self.failed = true;
                self.reader.take();
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "response body closed before all bytes were read",
                ))
            }
            Ok(amount) => {
                self.position += u64::try_from(amount).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "response read size exceeds u64")
                })?;
                if self.remaining() == 0 {
                    // HTTP framing is complete. Failed optional pool
                    // maintenance discards the connection, never the payload.
                    self.return_stream_to_pool();
                }
                Ok(amount)
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::Interrupted {
                    self.failed = true;
                    self.reader.take();
                }
                Err(e)
            }
        }
    }
}

/// Extract the charset from a "Content-Type" header.
///
/// "Content-Type: text/plain; charset=iso8859-1" -> "iso8859-1"
///
/// *Internal API*
pub(crate) fn charset_from_content_type(header: Option<&str>) -> &str {
    header
        .and_then(|header| {
            header.find(';').and_then(|semi| {
                header[semi + 1..]
                    .find('=')
                    .map(|equal| header[semi + equal + 2..].trim())
            })
        })
        .unwrap_or(DEFAULT_CHARACTER_SET)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{body::Payload, pool::PoolKey};

    use super::*;

    #[test]
    fn short_read() {
        use std::io::Cursor;
        let test_stream = crate::test::TestStream::new(Cursor::new(vec![b'a'; 3]), std::io::sink());
        let stream = Stream::new(
            test_stream,
            "1.1.1.1:4343".parse().unwrap(),
            PoolReturner::none(),
        );
        let mut lr = LimitedRead::new(stream, std::num::NonZeroU64::new(10).unwrap());
        let mut buf = vec![0; 1000];
        let result = lr.read_to_end(&mut buf);
        assert!(result.err().unwrap().kind() == io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn content_type_without_charset() {
        let s = "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json\r\n\
                 \r\n\
                 OK";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("application/json", resp.content_type());
    }

    #[test]
    fn content_type_without_cr() {
        let s = "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json\n\
                 \r\n\
                 OK";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("application/json", resp.content_type());
    }

    #[test]
    fn content_type_with_charset() {
        let s = "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json; charset=iso-8859-4\r\n\
                 \r\n\
                 OK";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("application/json", resp.content_type());
    }

    #[test]
    fn content_type_default() {
        let s = "HTTP/1.1 200 OK\r\n\r\nOK";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("text/plain", resp.content_type());
    }

    #[test]
    fn charset() {
        let s = "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json; charset=iso-8859-4\r\n\
                 \r\n\
                 OK";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("iso-8859-4", resp.charset());
    }

    #[test]
    fn charset_default() {
        let s = "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json\r\n\
                 \r\n\
                 OK";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("utf-8", resp.charset());
    }

    #[test]
    fn chunked_transfer() {
        let s = "HTTP/1.1 200 OK\r\n\
                 Transfer-Encoding: Chunked\r\n\
                 \r\n\
                 3\r\n\
                 hel\r\n\
                 b\r\n\
                 lo world!!!\r\n\
                 0\r\n\
                 \r\n";
        let resp = s.parse::<Response>().unwrap();
        assert_eq!("hello world!!!", resp.into_string().unwrap());
    }

    #[test]
    fn into_string_large() {
        const LEN: usize = INTO_STRING_LIMIT + 1;
        let s = format!(
            "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n
                 \r\n
                 {}",
            LEN,
            "A".repeat(LEN),
        );
        let result = s.parse::<Response>().unwrap();
        let err = result
            .into_string()
            .expect_err("didn't error with too-long body");
        assert_eq!(err.to_string(), "response too big for into_string");
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    #[cfg(feature = "json")]
    fn parse_simple_json() {
        let s = "HTTP/1.1 200 OK\r\n\
             \r\n\
             {\"hello\":\"world\"}";
        let resp = s.parse::<Response>().unwrap();
        let v: serde_json::Value = resp.into_json().unwrap();
        let compare = "{\"hello\":\"world\"}"
            .parse::<serde_json::Value>()
            .unwrap();
        assert_eq!(v, compare);
    }

    #[test]
    #[cfg(feature = "json")]
    fn parse_deserialize_json() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Hello {
            hello: String,
        }

        let s = "HTTP/1.1 200 OK\r\n\
             \r\n\
             {\"hello\":\"world\"}";
        let resp = s.parse::<Response>().unwrap();
        let v: Hello = resp.into_json::<Hello>().unwrap();
        assert_eq!(v.hello, "world");
    }

    #[test]
    fn parse_borked_header() {
        let s = "HTTP/1.1 BORKED\r\n".to_string();
        let err = s.parse::<Response>().unwrap_err();
        assert_eq!(err.kind(), BadStatus);
    }

    #[test]
    fn parse_header_without_reason() {
        let s = "HTTP/1.1 302\r\n\r\n".to_string();
        let resp = s.parse::<Response>().unwrap();
        assert_eq!(resp.status_text(), "");
    }

    #[test]
    fn read_next_line_large() {
        const LEN: usize = MAX_HEADER_SIZE + 1;
        let s = format!("Long-Header: {}\r\n", "A".repeat(LEN),);
        let mut cursor = Cursor::new(s);
        let mut budget = MAX_HEADER_SIZE + 1;
        let result = read_next_line(&mut cursor, "some context", &mut budget);
        let err = result.expect_err("did not error on too-large header");
        assert_eq!(err.kind(), ErrorKind::ResponseHeadTooLarge);
    }

    #[test]
    fn too_many_headers() {
        const LEN: usize = MAX_HEADER_COUNT + 1;
        let s = format!(
            "HTTP/1.1 200 OK\r\n{}\r\nhi",
            "Header: value\r\n".repeat(LEN),
        );
        let err = s
            .parse::<Response>()
            .expect_err("did not error on too many headers");
        assert_eq!(err.kind(), ErrorKind::ResponseHeadTooLarge);
    }

    #[test]
    #[cfg(feature = "charset")]
    fn read_next_line_non_ascii_reason() {
        let (cow, _, _) =
            encoding_rs::WINDOWS_1252.encode("HTTP/1.1 302 Déplacé Temporairement\r\n");
        let bytes = cow.to_vec();
        let mut reader = io::BufReader::new(io::Cursor::new(bytes));
        let mut budget = MAX_RESPONSE_HEAD_SIZE;
        let r = read_next_line(&mut reader, "test status line", &mut budget);
        let h = r.unwrap();
        assert_eq!(h.to_string(), "HTTP/1.1 302 D�plac� Temporairement");
    }

    #[test]
    #[cfg(feature = "charset")]
    fn parse_header_with_non_utf8() {
        let (cow, _, _) = encoding_rs::WINDOWS_1252.encode(
            "HTTP/1.1 200 OK\r\n\
            x-geo-header: gött mos!\r\n\
            \r\n\
            OK",
        );
        let v = cow.to_vec();
        let s = Stream::new(
            ReadOnlyStream::new(v),
            crate::stream::remote_addr_for_test(),
            PoolReturner::none(),
        );
        let request_url = "https://example.com".parse().unwrap();
        let request_reader = SizedReader {
            size: crate::body::BodySize::Empty,
            reader: Box::new(std::io::empty()),
        };
        let unit = Unit::new(
            &Agent::new(),
            "GET",
            &request_url,
            vec![],
            &request_reader,
            None,
        );
        let resp = Response::do_from_stream(s.into(), unit).unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.header("x-geo-header"), None);
    }

    #[test]
    fn history() {
        let mut response0 = Response::new(302, "Found", "").unwrap();
        response0.set_url("http://1.example.com/".parse().unwrap());
        assert!(response0.history.is_empty());

        let mut response1 = Response::new(302, "Found", "").unwrap();
        response1.set_url("http://2.example.com/".parse().unwrap());
        response1.history_from_previous(response0);

        let mut response2 = Response::new(404, "NotFound", "").unwrap();
        response2.set_url("http://2.example.com/".parse().unwrap());
        response2.history_from_previous(response1);

        let hist: Vec<String> = response2.history.iter().map(|r| r.to_string()).collect();
        assert_eq!(hist, ["http://1.example.com/", "http://2.example.com/"])
    }

    #[test]
    fn response_implements_send_and_sync() {
        let _response: Box<dyn Send> = Box::new(Response::new(302, "Found", "").unwrap());
        let _response: Box<dyn Sync> = Box::new(Response::new(302, "Found", "").unwrap());
    }

    #[test]
    fn ensure_response_size() {
        // This is platform dependent, so we can't be too strict or precise.
        let size = std::mem::size_of::<Response>();
        println!("Response size: {}", size);
        assert!(size < 400); // 200 on Macbook M1
    }

    // Test that a stream gets returned to the pool immediately for a zero-length response, and
    // that reads from the response's body consistently return Ok(0).
    #[test]
    fn zero_length_body_immediate_return() {
        use std::io::Cursor;
        let response_bytes = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
            .as_bytes()
            .to_vec();
        let test_stream =
            crate::test::TestStream::new(Cursor::new(response_bytes), std::io::sink());
        let agent = Agent::new();
        let agent2 = agent.clone();
        let stream = Stream::new(
            test_stream,
            "1.1.1.1:4343".parse().unwrap(),
            PoolReturner::new(&agent, PoolKey::from_parts("https", "example.com", 443)),
        );
        Response::do_from_stream(
            stream,
            Unit::new(
                &agent,
                "GET",
                &"https://example.com/".parse().unwrap(),
                vec![],
                &Payload::Empty.into_read(),
                None,
            ),
        )
        .unwrap();
        assert_eq!(agent2.state.pool.len(), 1);
    }

    #[test]
    #[cfg(feature = "gzip")]
    fn gzip_content_length() {
        use std::io::Cursor;
        let response_bytes =
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 23\r\n\r\n\
\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\x03\xcb\xc8\xe4\x02\x00\x7a\x7a\x6f\xed\x03\x00\x00\x00";
        // Follow the response with an infinite stream of 0 bytes, so the content-length
        // is important.
        let reader = Cursor::new(response_bytes).chain(std::io::repeat(0u8));
        let test_stream = crate::test::TestStream::new(reader, std::io::sink());
        let agent = Agent::new();
        let stream = Stream::new(
            test_stream,
            "1.1.1.1:4343".parse().unwrap(),
            PoolReturner::none(),
        );
        let resp = Response::do_from_stream(
            stream,
            Unit::new(
                &agent,
                "GET",
                &"https://example.com/".parse().unwrap(),
                vec![],
                &Payload::Empty.into_read(),
                None,
            ),
        )
        .unwrap();
        let body = resp.into_string().unwrap();
        assert_eq!(body, "hi\n");
    }

    #[test]
    fn connection_option() {
        use ConnectionOption::*;
        for (version, values, expected) in [
            ("HTTP/1.0", vec![], Close),
            ("HTTP/1.0", vec!["keep-alive"], Close),
            ("HTTP/1.1", vec![], KeepAlive),
            ("HTTP/1.1", vec!["keep-alive"], KeepAlive),
            ("HTTP/1.1", vec!["x-close"], KeepAlive),
            ("HTTP/1.1", vec!["keep-alive", "CLOSE"], Close),
            ("HTTP/1.1", vec![" , keep-alive,\tClOsE ,"], Close),
        ] {
            let headers: Vec<_> = values
                .iter()
                .map(|v| Header::new("Connection", v))
                .collect();
            assert_eq!(
                Response::connection_option(version, &headers).unwrap(),
                expected
            );
        }
        assert!(
            Response::connection_option("HTTP/1.1", &[Header::new("Connection", "close;bad")])
                .is_err()
        );
    }

    #[test]
    fn body_type() {
        use BodyType::*;
        for (method, status, version, headers, expected) in [
            ("GET", 200, "HTTP/1.1", vec![], CloseDelimited),
            ("HEAD", 200, "HTTP/1.1", vec![], LengthDelimited(0)),
            ("hEaD", 200, "HTTP/1.1", vec![], LengthDelimited(0)),
            ("head", 200, "HTTP/1.1", vec![], LengthDelimited(0)),
            ("GET", 204, "HTTP/1.1", vec![], LengthDelimited(0)),
            ("GET", 304, "HTTP/1.1", vec![], LengthDelimited(0)),
            (
                "GET",
                304,
                "HTTP/1.1",
                vec![Header::new("Transfer-Encoding", "gzip, chunked")],
                LengthDelimited(0),
            ),
            (
                "GET",
                200,
                "HTTP/1.1",
                vec![Header::new("Transfer-Encoding", "Chunked")],
                Chunked,
            ),
            (
                "GET",
                205,
                "HTTP/1.1",
                vec![Header::new("Transfer-Encoding", "chunked")],
                Chunked,
            ),
            (
                "GET",
                200,
                "HTTP/1.1",
                vec![Header::new("Content-Length", "123")],
                LengthDelimited(123),
            ),
            (
                "GET",
                200,
                "HTTP/1.0",
                vec![Header::new("Content-Length", "123")],
                LengthDelimited(123),
            ),
        ] {
            assert_eq!(
                Response::body_type(method, status, version, &headers).unwrap(),
                expected
            );
        }
        for method in ["GET", "HEAD"] {
            for headers in [
                vec![
                    Header::new("Content-Length", "123"),
                    Header::new("Transfer-Encoding", "chunked"),
                ],
                vec![
                    Header::new("Transfer-Encoding", "chunked"),
                    Header::new("Content-Length", "123"),
                ],
            ] {
                assert_eq!(
                    Response::body_type(method, 200, "HTTP/1.1", &headers)
                        .unwrap_err()
                        .kind(),
                    ErrorKind::BadHeader
                );
            }
        }
        assert_eq!(
            Response::body_type(
                "GET",
                200,
                "HTTP/1.0",
                &[Header::new("Transfer-Encoding", "chunked")]
            )
            .unwrap_err()
            .kind(),
            ErrorKind::BadHeader
        );
    }
}

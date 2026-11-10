// Copyright 2015 The tiny-http Contributors
// Copyright 2015 The rust-chunked-transfer Contributors
// Forked into ureq, 2024, from https://github.com/frewsxcv/rust-chunked-transfer
// Forked under dual MIT and Apache 2.0 license (see adjacent LICENSE-MIT and LICENSE-APACHE file)

use std::convert::TryFrom;
use std::io::{self, Read};

use crate::header::{is_tchar, HeaderLine};

// Wire budgets include CRLF. Framing includes size/extension lines, payload
// delimiters and trailers, but never content bytes. No allocation uses a size
// declared by the peer; at most one bounded metadata line is retained.
const MAX_METADATA_LINE: usize = 8 * 1024;
const MAX_TRAILERS: usize = 64 * 1024;
const MAX_TRAILER_FIELDS: usize = 64;
const MAX_FRAMING: usize = 1024 * 1024;

#[derive(Clone, Copy)]
enum State {
    Size,
    Data(u64),
    DataCrlf,
    Trailers,
    End,
    Failed,
}

/// Reads payload bytes only after parsing bounded HTTP chunk framing.
pub struct Decoder<R> {
    source: R,
    state: State,
    framing_bytes: usize,
    trailer_bytes: usize,
    trailer_fields: usize,
}

fn malformed() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP chunk framing")
}

fn metadata_limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        crate::ErrorKind::BodyMetadataTooLarge.msg("HTTP chunk metadata limit exceeded"),
    )
}

fn skip_ows(bytes: &[u8]) -> &[u8] {
    let length = bytes
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    &bytes[length..]
}

fn token(rest: &mut &[u8]) -> io::Result<()> {
    let length = rest.iter().take_while(|b| is_tchar(b)).count();
    if length == 0 {
        return Err(malformed());
    }
    *rest = &rest[length..];
    Ok(())
}

impl<R: Read> Decoder<R> {
    pub fn new(source: R) -> Self {
        Self {
            source,
            state: State::Size,
            framing_bytes: 0,
            trailer_bytes: 0,
            trailer_fields: 0,
        }
    }

    pub fn into_inner(self) -> R {
        self.source
    }

    fn metadata_byte(&mut self, trailer: bool) -> io::Result<u8> {
        if self.framing_bytes == MAX_FRAMING || (trailer && self.trailer_bytes == MAX_TRAILERS) {
            return Err(metadata_limit());
        }
        let mut byte = [0];
        self.source.read_exact(&mut byte)?;
        self.framing_bytes += 1;
        if trailer {
            self.trailer_bytes += 1;
        }
        Ok(byte[0])
    }

    fn metadata_line(&mut self, trailer: bool) -> io::Result<Vec<u8>> {
        let mut line = Vec::new();
        loop {
            if line.len() == MAX_METADATA_LINE {
                return Err(metadata_limit());
            }
            line.push(self.metadata_byte(trailer)?);
            if line.last() == Some(&b'\n') {
                if !line.ends_with(b"\r\n") {
                    return Err(malformed());
                }
                line.truncate(line.len() - 2);
                return Ok(line);
            }
        }
    }

    fn read_chunk_size(&mut self) -> io::Result<u64> {
        let line = self.metadata_line(false)?;
        let digits = line.iter().take_while(|b| b.is_ascii_hexdigit()).count();
        if digits == 0 {
            return Err(malformed());
        }
        let size = line[..digits]
            .iter()
            .try_fold(0u64, |size, byte| {
                let digit = match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => byte - b'A' + 10,
                };
                size.checked_mul(16)?.checked_add(u64::from(digit))
            })
            .ok_or_else(malformed)?;
        let mut rest = &line[digits..];
        // RFC 9112 section 7.1.1: BWS is accepted at extension delimiters,
        // extension names are tokens and values are tokens or quoted strings.
        while !rest.is_empty() {
            rest = skip_ows(rest);
            if rest.first() != Some(&b';') {
                return Err(malformed());
            }
            rest = skip_ows(&rest[1..]);
            token(&mut rest)?;
            let after_name = skip_ows(rest);
            if after_name.first() == Some(&b'=') {
                rest = skip_ows(&after_name[1..]);
                if rest.first() == Some(&b'"') {
                    rest = &rest[1..];
                    loop {
                        match rest.first() {
                            Some(b'"') => {
                                rest = &rest[1..];
                                break;
                            }
                            Some(b'\\') => {
                                if rest.len() < 2
                                    || !matches!(rest[1], b'\t' | b' '..=b'~' | 0x80..=0xff)
                                {
                                    return Err(malformed());
                                }
                                rest = &rest[2..];
                            }
                            Some(b'\t' | b' ' | b'!' | b'#'..=b'[' | b']'..=b'~' | 0x80..=0xff) => {
                                rest = &rest[1..]
                            }
                            _ => return Err(malformed()),
                        }
                    }
                } else {
                    token(&mut rest)?;
                }
            }
        }
        Ok(size)
    }

    fn read_body(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.state {
                State::Size => {
                    let size = self.read_chunk_size()?;
                    self.state = if size == 0 {
                        State::Trailers
                    } else {
                        State::Data(size)
                    };
                }
                State::Data(remaining) => {
                    let amount = usize::try_from(remaining)
                        .unwrap_or(usize::MAX)
                        .min(buf.len());
                    let count = self.source.read(&mut buf[..amount])?;
                    if count == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "incomplete HTTP chunk payload",
                        ));
                    }
                    let remaining = remaining - u64::try_from(count).map_err(|_| malformed())?;
                    self.state = if remaining == 0 {
                        State::DataCrlf
                    } else {
                        State::Data(remaining)
                    };
                    // Return payload before attempting any subsequent framing:
                    // Read cannot report both payload bytes and an error.
                    return Ok(count);
                }
                State::DataCrlf => {
                    if self.metadata_byte(false)? != b'\r' || self.metadata_byte(false)? != b'\n' {
                        return Err(malformed());
                    }
                    self.state = State::Size;
                }
                State::Trailers => {
                    let line = self.metadata_line(true)?;
                    if line.is_empty() {
                        self.state = State::End;
                        continue;
                    }
                    // The terminating empty line is framing, not a field.
                    // Count nonempty lines independently of the byte budgets.
                    if self.trailer_fields == MAX_TRAILER_FIELDS {
                        return Err(metadata_limit());
                    }
                    self.trailer_fields += 1;
                    let header = HeaderLine::from(line)
                        .into_header()
                        .map_err(|_| malformed())?;
                    // Validate and discard trailer fields separately. Framing,
                    // routing, authentication, response control and content
                    // processing fields cannot replace admitted head facts.
                    if [
                        "content-length",
                        "transfer-encoding",
                        "host",
                        "connection",
                        "keep-alive",
                        "proxy-connection",
                        "te",
                        "trailer",
                        "upgrade",
                        "authorization",
                        "proxy-authorization",
                        "www-authenticate",
                        "proxy-authenticate",
                        "authentication-info",
                        "proxy-authentication-info",
                        "cookie",
                        "set-cookie",
                        "content-encoding",
                        "content-type",
                        "content-range",
                        "content-location",
                        "cache-control",
                        "age",
                        "expires",
                        "date",
                        "location",
                        "retry-after",
                        "vary",
                        "range",
                        "if-range",
                        "if-match",
                        "if-none-match",
                        "if-modified-since",
                        "if-unmodified-since",
                        "expect",
                        "max-forwards",
                    ]
                    .iter()
                    .any(|name| header.is_name(name))
                    {
                        return Err(malformed());
                    }
                }
                State::End => return Ok(0),
                State::Failed => return Err(malformed()),
            }
        }
    }
}

impl<R: Read> Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Empty reads do not consume framing, report completion, or mutate state.
        if buf.is_empty() {
            return Ok(0);
        }
        let result = self.read_body(buf);
        if result
            .as_ref()
            .is_err_and(|error| error.kind() != io::ErrorKind::Interrupted)
        {
            self.state = State::Failed;
        }
        result
    }
}

#[cfg(test)]
mod test {
    use super::Decoder;
    use std::io;
    use std::io::Read;

    /// This unit test is taken from from Hyper
    /// https://github.com/hyperium/hyper
    /// Copyright (c) 2014 Sean McArthur
    #[test]
    fn test_read_chunk_size() {
        fn read(s: &str, expected: u64) {
            let mut decoded = Decoder::new(s.as_bytes());
            assert_eq!(expected, decoded.read_chunk_size().unwrap());
        }
        fn read_err(s: &str, expected: io::ErrorKind) {
            let mut decoded = Decoder::new(s.as_bytes());
            assert_eq!(decoded.read_chunk_size().unwrap_err().kind(), expected);
        }
        for (wire, size) in [
            ("1\r\n", 1),
            ("01\r\n", 1),
            ("0\r\n", 0),
            ("00\r\n", 0),
            ("A\r\n", 10),
            ("a\r\n", 10),
            ("Ff\r\n", 255),
            ("1;extension\r\n", 1),
            ("1;extension;extension2\r\n", 1),
            ("2; extension...\r\n", 2),
            ("3   ; extension=123\r\n", 3),
        ] {
            read(wire, size);
        }
        for wire in ["F\rF", "F", "1;no CRLF"] {
            read_err(wire, io::ErrorKind::UnexpectedEof);
        }
        // Former compatibility recovery accepted invalid syntax. Preserve the
        // inputs as rejection cases under AI5's supported RFC 9112 grammar.
        for wire in [
            "Ff   \r\n",
            "X\r\n",
            "1X\r\n",
            "-\r\n",
            "-1\r\n",
            "a;ext name=value\r\n",
            "1;;;  ;\r\n",
            "3   ;\r\n",
            "3   ;   \r\n",
            "1 invalid extension\r\n",
            "1 A\r\n",
        ] {
            read_err(wire, io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn test_valid_chunk_decode() {
        let source = io::Cursor::new(
            "3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n\r\n"
                .to_string()
                .into_bytes(),
        );
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        decoded.read_to_string(&mut string).unwrap();

        assert_eq!(string, "hello world!!!");
    }

    #[test]
    fn test_decode_zero_length() {
        let mut decoder = Decoder::new(b"0\r\n\r\n" as &[u8]);

        let mut decoded = String::new();
        decoder.read_to_string(&mut decoded).unwrap();

        assert_eq!(decoded, "");
    }

    #[test]
    fn test_decode_invalid_chunk_length() {
        let mut decoder = Decoder::new(b"m\r\n\r\n" as &[u8]);

        let mut decoded = String::new();
        assert!(decoder.read_to_string(&mut decoded).is_err());
    }

    #[test]
    fn invalid_input1() {
        let source = io::Cursor::new(
            "2\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n"
                .to_string()
                .into_bytes(),
        );
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        assert!(decoded.read_to_string(&mut string).is_err());
    }

    #[test]
    fn invalid_input2() {
        let source = io::Cursor::new(
            "3\rhel\r\nb\r\nlo world!!!\r\n0\r\n"
                .to_string()
                .into_bytes(),
        );
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        assert!(decoded.read_to_string(&mut string).is_err());
    }

    #[test]
    fn test_decode_end_missing_last_crlf() {
        // This has been observed in the wild.
        // See https://github.com/algesten/ureq/issues/325

        // Missing last \r\n
        let source = io::Cursor::new(
            "3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n"
                .to_string()
                .into_bytes(),
        );
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        assert_eq!(
            decoded.read_to_string(&mut string).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );

        assert_eq!(string, "hello world!!!");
    }
}

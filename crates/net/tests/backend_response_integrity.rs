//! AI5 regression specifications against the production ureq parser and readers.
//! These are ordinary tests, including the invariants that fail before AI5 fixes.
use std::io::{self, Read};

fn chunked(body: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{body}")
}

fn read(wire: &str) -> (io::Result<usize>, Vec<u8>) {
    let response: ureq::Response = wire.parse().expect("valid response head");
    let mut body = Vec::new();
    let result = response.into_reader().read_to_end(&mut body);
    (result, body)
}

#[test]
fn valid_chunked_statuses_preserve_ordered_headers_and_binary_bytes() {
    for status in [200, 404, 500] {
        let wire = format!(
            "HTTP/1.1 {status} Response\r\nX-A: one\r\nX-B: middle\r\nX-A: two\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nA\0\u{007f}\r\n0\r\n\r\n"
        );
        let response: ureq::Response = wire.parse().unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers_names(),
            ["x-a", "x-b", "x-a", "transfer-encoding"]
        );
        assert_eq!(response.all("x-a"), ["one", "two"]);
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        assert_eq!(body, [b'A', 0, 0x7f]);
    }
}

#[test]
fn fixed_length_truncation_retains_prefix_and_reports_unexpected_eof() {
    let (result, body) = read("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nabc");
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(body, b"abc");
}

#[test]
fn close_delimited_eof_is_framing_completion_not_application_completeness() {
    let (result, body) = read("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nabc");
    result.unwrap();
    assert_eq!(body, b"abc");
}

macro_rules! incomplete_chunk {
    ($name:ident, $wire:expr) => {
        #[test]
        fn $name() {
            let (result, body) = read(&chunked($wire));
            assert!(
                result.is_err(),
                "incomplete chunk framing was accepted: {body:?}"
            );
            assert_eq!(
                body, b"abc",
                "known payload prefix must survive the failure"
            );
        }
    };
}

incomplete_chunk!(truncated_chunk_payload_is_not_successful_eof, "5\r\nabc");
incomplete_chunk!(missing_zero_chunk_is_not_successful_eof, "3\r\nabc\r\n");
incomplete_chunk!(zero_chunk_requires_final_crlf, "3\r\nabc\r\n0\r\n");
incomplete_chunk!(half_final_crlf_is_not_successful_eof, "3\r\nabc\r\n0\r\n\r");

#[test]
fn leading_whitespace_in_chunk_size_is_rejected() {
    let (result, _) = read(&chunked(" 3\r\nabc\r\n0\r\n\r\n"));
    assert!(
        result.is_err(),
        "chunk-size is 1*HEXDIG, without leading OWS"
    );
}

#[test]
fn invalid_and_overflowing_chunk_sizes_are_rejected() {
    for size in ["xyz", "100000000000000000000000000000000"] {
        let (result, body) = read(&chunked(&format!("{size}\r\nabc\r\n0\r\n\r\n")));
        assert!(result.is_err(), "accepted chunk size {size}");
        assert!(body.is_empty());
    }
}

#[test]
fn valid_chunk_extensions_and_empty_chunked_bodies_are_accepted() {
    for (payload, expected) in [
        (
            "3;name=token;quoted=\"value\"\r\nabc\r\n0\r\n\r\n",
            b"abc".as_slice(),
        ),
        ("0\r\n\r\n", b"".as_slice()),
    ] {
        let (result, body) = read(&chunked(payload));
        result.unwrap();
        assert_eq!(body, expected);
    }
}

#[test]
fn malformed_chunk_payload_delimiters_are_rejected() {
    for payload in ["3\r\nabcX\n0\r\n\r\n", "3\r\nabc\rX0\r\n\r\n"] {
        let (result, _) = read(&chunked(payload));
        assert!(result.is_err());
    }
}

macro_rules! invalid_head {
    ($name:ident, $status:literal, $fields:literal) => {
        #[test]
        fn $name() {
            let wire = concat!("HTTP/1.1 ", $status, " Response\r\n", $fields, "\r\nabc");
            assert!(
                wire.parse::<ureq::Response>().is_err(),
                "unsafe framing must be rejected during response construction"
            );
        }
    };
}

invalid_head!(
    malformed_content_length_is_rejected_at_head,
    "200",
    "Content-Length: nope\r\n"
);
invalid_head!(
    signed_content_length_is_rejected_at_head,
    "200",
    "Content-Length: +3\r\n"
);
invalid_head!(
    conflicting_content_length_list_is_rejected_at_head,
    "200",
    "Content-Length: 3, 4\r\n"
);
invalid_head!(
    malformed_later_content_length_is_not_ignored,
    "200",
    "Content-Length: 3\r\nContent-Length: nope\r\n"
);
invalid_head!(
    overflowing_content_length_is_rejected_at_head,
    "200",
    "Content-Length: 18446744073709551616\r\n"
);
invalid_head!(
    conflicting_content_lengths_are_rejected_at_head,
    "200",
    "Content-Length: 3\r\nContent-Length: 4\r\n"
);
invalid_head!(
    transfer_encoding_with_content_length_is_rejected_at_head,
    "200",
    "Transfer-Encoding: chunked\r\nContent-Length: 3\r\n"
);
invalid_head!(
    unsupported_transfer_coding_is_rejected_at_head,
    "200",
    "Transfer-Encoding: gzip\r\n"
);
invalid_head!(
    non_final_chunked_is_rejected_at_head,
    "200",
    "Transfer-Encoding: chunked, gzip\r\n"
);
invalid_head!(
    repeated_chunked_is_rejected_at_head,
    "200",
    "Transfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n"
);
invalid_head!(
    malformed_transfer_encoding_list_is_rejected_at_head,
    "200",
    // A trailing empty member is permitted by RFC 9110 recipient grammar.
    // Keep rejection coverage for an actually invalid nonempty member instead.
    "Transfer-Encoding: chunked, @\r\n"
);
invalid_head!(
    status_204_prohibits_even_zero_content_length,
    "204",
    "Content-Length: 0\r\n"
);
invalid_head!(
    status_204_prohibits_transfer_encoding,
    "204",
    "Transfer-Encoding: chunked\r\n"
);
invalid_head!(
    status_205_prohibits_nonzero_content_length,
    "205",
    "Content-Length: 3\r\n"
);
invalid_head!(
    status_304_rejects_conflicting_representation_framing,
    "304",
    "Transfer-Encoding: chunked\r\nContent-Length: 123\r\n"
);

#[test]
fn identical_repeated_content_lengths_bound_the_body() {
    let (result, body) =
        read("HTTP/1.1 200 OK\r\nContent-Length: 3\r\nContent-Length: 3\r\n\r\nabcEXTRA");
    result.unwrap();
    assert_eq!(body, b"abc");
}

#[test]
fn identical_comma_list_content_lengths_bound_the_body() {
    let (result, body) = read("HTTP/1.1 200 OK\r\nContent-Length: 3, 3\r\n\r\nabcEXTRA");
    result.unwrap();
    assert_eq!(body, b"abc");
}

#[test]
fn status_304_transfer_encoding_is_representation_metadata_without_body() {
    let response: ureq::Response =
        "HTTP/1.1 304 Not Modified\r\nTransfer-Encoding: gzip, chunked\r\n\r\n"
            .parse()
            .unwrap();
    assert_eq!(response.header("transfer-encoding"), Some("gzip, chunked"));
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body).unwrap();
    assert!(body.is_empty(), "304 must not wait for a chunk terminator");
}

#[test]
fn status_304_gzip_representation_metadata_does_not_start_a_decoder() {
    let response: ureq::Response =
        "HTTP/1.1 304 Not Modified\r\nContent-Length: 123\r\nContent-Encoding: gzip\r\n\r\n"
            .parse()
            .unwrap();
    assert_eq!(response.header("content-length"), Some("123"));
    assert_eq!(response.header("content-encoding"), Some("gzip"));
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body).unwrap();
    assert!(body.is_empty());
}

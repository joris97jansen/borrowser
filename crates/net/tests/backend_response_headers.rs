//! Exercise the pinned backend's real response parser, including accesses made
//! before Borrowser can project response metadata. No model is mocked here.
use std::io::Read;

#[test]
fn malformed_field_lines_fail_before_backend_framing_or_projection() {
    for field in [
        "X-Broken",
        "Content-Length",
        "Transfer-Encoding",
        "Content-Encoding",
        "Connection",
        "content-length",
        "PRIVATE_BROKEN_FIELD",
        ": PRIVATE_VALUE",
        "Bad Name: PRIVATE_VALUE",
        "X-Invalid@: PRIVATE_VALUE",
    ] {
        let wire = format!(
            "HTTP/1.1 200 OK\r\nX-Good: retained-only-if-valid\r\n{field}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let failure = wire
            .parse::<ureq::Response>()
            .expect_err("a malformed field must reject the entire response head");
        assert_eq!(failure.kind(), ureq::ErrorKind::BadHeader, "{field}");
        for diagnostic in [
            format!("{failure}"),
            format!("{failure:?}"),
            format!("{failure:#?}"),
        ] {
            assert!(!diagnostic.contains("PRIVATE"));
            assert!(!diagnostic.contains("retained-only-if-valid"));
        }
    }
}

#[test]
fn empty_repeated_and_interleaved_fields_preserve_backend_order_and_body() {
    for status in [200, 404, 500] {
        let wire = format!(
            "HTTP/1.1 {status} Response\r\nX-A: first\r\nSet-Cookie: a=1\r\nX-Empty:\r\nX-A: second\r\nSet-Cookie: b=2\r\nX-Empty:\r\nContent-Length: 3\r\nConnection: close\r\n\r\n\0\u{007f}A"
        );
        let response: ureq::Response = wire.parse().unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers_names(),
            [
                "x-a",
                "set-cookie",
                "x-empty",
                "x-a",
                "set-cookie",
                "x-empty",
                "content-length",
                "connection"
            ]
        );
        assert_eq!(response.header("x-empty"), Some(""));
        assert_eq!(response.all("x-empty"), ["", ""]);
        assert_eq!(response.all("x-a"), ["first", "second"]);
        assert_eq!(response.all("set-cookie"), ["a=1", "b=2"]);
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        assert_eq!(body, [0, 0x7f, b'A']);
    }
}

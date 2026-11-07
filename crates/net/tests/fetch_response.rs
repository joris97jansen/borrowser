use addressing::BrowserUrl;
use core_types::ResourceKind;
use net::{
    BodyProgress, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind, NetworkResponseInfo,
    ResponseAdmissionError,
};

fn url(value: &str) -> BrowserUrl {
    BrowserUrl::parse(value).unwrap().url
}
fn response(code: u16, reported: Option<&str>, headers: HttpHeaders) -> NetworkResponseInfo {
    NetworkResponseInfo::http(
        url("https://example.test/a#original"),
        "https://example.test/a".into(),
        reported.map(str::to_owned),
        HttpStatus::try_from(code).unwrap(),
        headers,
    )
    .unwrap()
}

#[test]
fn ai3_statuses_preserve_http_facts_without_fake_failures() {
    for code in [200, 404, 500, 302, 308, 299, 399, 471, 599] {
        let r = response(code, Some("https://final.test/b"), HttpHeaders::new());
        let status = r.status().unwrap();
        assert_eq!(status.code(), code);
        assert_eq!(status.is_success(), (200..300).contains(&code));
        assert_eq!(status.is_http_error(), code >= 400);
        assert_eq!(status.is_redirection(), (300..400).contains(&code));
        assert_eq!(
            status.is_redirect(),
            matches!(code, 301 | 302 | 303 | 307 | 308)
        );
        assert_eq!(r.original_url().as_str(), "https://example.test/a#original");
        assert_eq!(r.requested_url(), "https://example.test/a");
        assert_eq!(
            r.response_url().unwrap().as_ref().unwrap().url.as_str(),
            "https://final.test/b"
        );
    }
    for code in [0, 99, 600, 999, u16::MAX] {
        assert_eq!(
            HttpStatus::try_from(code),
            Err(ResponseAdmissionError::InvalidStatus)
        );
    }
    for code in [100, 103, 199] {
        let status = HttpStatus::try_from(code).unwrap();
        assert!(status.is_informational());
        assert_eq!(
            NetworkResponseInfo::http(
                url("https://a.test/"),
                "https://a.test/".into(),
                None,
                status,
                HttpHeaders::new()
            )
            .unwrap_err(),
            ResponseAdmissionError::InformationalResponseUnsupported
        );
    }
    assert!(HttpStatus::try_from(304).unwrap().is_redirection());
    assert!(!HttpStatus::try_from(304).unwrap().is_redirect());
}

#[test]
fn ai3_url_provenance_is_exact_optional_and_parsed_once() {
    let absent = response(200, None, HttpHeaders::new());
    assert!(absent.reported_response_url().is_none());
    assert!(absent.response_url().is_none());
    let empty = response(200, Some(""), HttpHeaders::new());
    assert_eq!(empty.reported_response_url(), Some(""));
    assert!(empty.response_url().unwrap().is_err());
    assert_ne!(absent, empty);
    let invalid = response(200, Some("https://host.test:bad/a"), HttpHeaders::new());
    assert!(invalid.response_url().unwrap().is_err());
    let raw = "HTTPS://EXAMPLE.TEST:443/a%zz?q=1#f";
    let valid = response(200, Some(raw), HttpHeaders::new());
    assert_eq!(valid.reported_response_url(), Some(raw));
    assert_eq!(valid.response_url().unwrap(), &BrowserUrl::parse(raw));
    let normalized = response(
        200,
        Some("https://example.test/a%zz?q=1#f"),
        HttpHeaders::new(),
    );
    assert_ne!(
        valid, normalized,
        "equal parsed URLs must not hide changed reported provenance"
    );
}

#[test]
fn ai3_local_results_are_not_http_responses() {
    let r = NetworkResponseInfo::local_file(
        url("file:///tmp/a.css#f"),
        "file:///tmp/a.css".into(),
        Some("file:///tmp/a.css".into()),
        Some("text/css".into()),
    )
    .unwrap();
    assert_eq!(r.status(), None);
    assert!(r.headers().is_empty());
    assert_eq!(r.declared_content_types().count(), 0);
    assert_eq!(r.legacy_content_type(), Some("text/css"));
    assert_eq!(
        NetworkResponseInfo::http(
            url("file:///tmp/a"),
            "file:///tmp/a".into(),
            None,
            HttpStatus::try_from(200).unwrap(),
            HttpHeaders::new()
        )
        .unwrap_err(),
        ResponseAdmissionError::IncompatibleOriginalScheme
    );
    assert_eq!(
        NetworkResponseInfo::local_file(
            url("https://a.test/"),
            "https://a.test/".into(),
            None,
            None
        )
        .unwrap_err(),
        ResponseAdmissionError::IncompatibleOriginalScheme
    );
}

#[test]
fn ai3_fields_keep_order_casing_and_arbitrary_bytes() {
    let mut h = HttpHeaders::new();
    h.append("Set-Cookie", b"a=1").unwrap();
    h.append("X-Bytes", &[0xff, 0x80, b'\t']).unwrap();
    h.append("SET-cookie", b"b=2").unwrap();
    let r = response(404, Some("https://final.test/"), h.clone());
    assert_eq!(r.headers(), &h);
    assert_eq!(
        r.headers().iter().collect::<Vec<_>>(),
        vec![
            ("set-cookie", b"a=1".as_slice()),
            ("x-bytes", [0xff, 0x80, b'\t'].as_slice()),
            ("set-cookie", b"b=2".as_slice())
        ]
    );
    assert_eq!(
        r.headers().values("SET-COOKIE").collect::<Vec<_>>(),
        vec![b"a=1".as_slice(), b"b=2".as_slice()]
    );
}

#[test]
fn ai3_mime_charset_uses_independent_whatwg_cases() {
    // WHATWG MIME §4.4; WPT mimesniff/mime-types/resources/mime-types.json,
    // inspected blob 8692d77ebf4f5d28b1358cf65b1fe89e60f8a77e.
    // Expectations are literal projections, not generated by the parser.
    for (input, essence, charset) in [
        ("TEXT/HTML;CHARSET=GBK", "text/html", Some("GBK")),
        (
            "text/html;charset=gbk;charset=windows-1255",
            "text/html",
            Some("gbk"),
        ),
        ("text/html;charset =gbk", "text/html", None),
        ("text/html ;charset=gbk", "text/html", Some("gbk")),
        ("text/html;charset= gbk", "text/html", Some(" gbk")),
        ("text/html;x=(;charset=gbk", "text/html", Some("gbk")),
        ("text/html;charset=();charset=GBK", "text/html", Some("()")),
        (
            "text/css; x=1; charset=\"utf-8\"",
            "text/css",
            Some("utf-8"),
        ),
        (
            "text/html;charset=\"shift_jis\"iso-2022-jp",
            "text/html",
            Some("shift_jis"),
        ),
        (
            "image/png; charset=\"unclosed",
            "image/png",
            Some("unclosed"),
        ),
        (
            "text/html;charset=\"a\\\"b;z\"",
            "text/html",
            Some("a\"b;z"),
        ),
        (
            "text/html;charset=\"\";charset=utf-8",
            "text/html",
            Some(""),
        ),
        (
            "text/html;charset=;charset=unknown-label",
            "text/html",
            Some("unknown-label"),
        ),
        ("application/example;", "application/example", None),
    ] {
        let mut h = HttpHeaders::new();
        h.append("Content-Type", input.as_bytes()).unwrap();
        let r = response(200, None, h);
        let parsed = r.declared_content_types().next().unwrap().unwrap();
        assert_eq!(parsed.mime_type(), essence, "{input}");
        assert_eq!(parsed.charset(), charset, "{input}");
        assert_eq!(
            r.headers().values("content-type").next().unwrap(),
            input.as_bytes()
        );
    }
    let mut h = HttpHeaders::new();
    h.append("content-type", b"invalid").unwrap();
    h.append("Content-Type", b"text/plain;charset=\"\xff\"")
        .unwrap();
    let r = response(200, None, h);
    let mut fields = r.declared_content_types();
    assert!(fields.next().unwrap().is_err());
    assert_eq!(fields.next().unwrap().unwrap().charset(), Some("ÿ"));
    assert!(fields.next().is_none());
    assert_eq!(
        response(200, None, HttpHeaders::new())
            .declared_content_types()
            .count(),
        0
    );
    for invalid in ["", "text", "/plain", "text/", "text /html", "text/html,x"] {
        let mut h = HttpHeaders::new();
        h.append("Content-Type", invalid.as_bytes()).unwrap();
        let r = response(200, None, h);
        assert!(
            r.declared_content_types().next().unwrap().is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn ai3_body_accounting_is_route_bound() {
    for (route, cap) in [
        (ResourceKind::Html, 10 * 1024 * 1024),
        (ResourceKind::Css, 2 * 1024 * 1024),
        (ResourceKind::Image, 20 * 1024 * 1024),
    ] {
        let p = BodyProgress::try_new(route, cap).unwrap();
        assert_eq!(p.limit_bytes(), cap);
        assert_eq!(p.delivered_bytes(), cap);
        assert!(p.matches_route(route));
        assert_eq!(
            BodyProgress::try_new(route, cap + 1),
            Err(NetworkErrorKind::ResourceLimit)
        );
    }
    assert!(
        !BodyProgress::try_new(ResourceKind::Image, 1)
            .unwrap()
            .matches_route(ResourceKind::Html)
    );
    for code in [204, 205, 304] {
        assert!(HttpStatus::try_from(code).unwrap().has_null_body());
    }
    assert!(!HttpStatus::try_from(206).unwrap().has_null_body());
}

#[test]
fn ai3_debug_is_literal_bounded_and_redacted_when_nested() {
    let r = response(200, Some("https://SECRET.test/PRIVATE"), HttpHeaders::new());
    assert_eq!(
        format!("{r:?}"),
        "NetworkResponseInfo { original_url: \"<redacted>\", requested_url: \"<redacted>\", response_url: \"parsed\", status: Some(HttpStatus(200)), headers: [], local_content_type_present: false }"
    );
    let mut h = HttpHeaders::new();
    h.append("PRIVATE_NAME", b"SECRET_HEADER").unwrap();
    h.append("content-type", b"application/SECRET; charset=PRIVATE")
        .unwrap();
    let r = response(500, Some("https://SECRET.test/PRIVATE"), h);
    let ct = r.declared_content_types().next().unwrap().unwrap();
    for output in [
        format!("{r:?}"),
        format!("{r:#?}"),
        format!("{:?}", Some(Ok::<_, ()>(&r))),
        format!("{ct:#?}"),
    ] {
        assert!(!output.contains("SECRET"));
        assert!(!output.contains("PRIVATE"));
        assert!(output.len() < 10000);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    drop(rx);
    let event = NetEvent::Error {
        request_id: 7654321,
        url: "SECRET_URL".into(),
        error_kind: NetworkErrorKind::Read,
        response: Some(r),
        body: Some(BodyProgress::try_new(ResourceKind::Html, 3).unwrap()),
        error: "SECRET_ERROR".into(),
    };
    let error = tx.send(event).unwrap_err();
    for output in [format!("{error:?}"), format!("{error:#?}")] {
        assert!(!output.contains("SECRET"));
        assert!(!output.contains("7654321"));
    }
    let event = NetEvent::Chunk {
        request_id: 7654321,
        url: "SECRET_URL".into(),
        chunk: b"SECRET_BYTES".to_vec(),
    };
    assert_eq!(format!("{event:?}"), "Chunk { bytes: 12, .. }");
}

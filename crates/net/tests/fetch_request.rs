use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use core_types::ResourceKind as Route;
use net::{FetchRequest, HttpHeaders, RedirectPolicy, RequestAdmissionError as E, RequestMethod};

fn url(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).unwrap().url
}
fn request(input: &str, purpose: Purpose, headers: HttpHeaders) -> Result<FetchRequest, E> {
    FetchRequest::try_new(
        7,
        ResourceAddress::new(url(input), purpose),
        RequestMethod::Get,
        headers,
        None,
        None,
    )
}

#[test]
fn ai2_document_stylesheet_image_gets_preserve_purpose_and_provenance() {
    for (purpose, route, issued) in [
        (
            Purpose::Document,
            Route::Html,
            "https://example.test/a?x=1&x=2#f",
        ),
        (
            Purpose::Stylesheet,
            Route::Css,
            "https://example.test/a?x=1&x=2",
        ),
        (
            Purpose::Image,
            Route::Image,
            "https://example.test/a?x=1&x=2#f",
        ),
    ] {
        let r = request(
            "HTTPS://EXAMPLE.TEST:443/a?x=1&x=2#f",
            purpose,
            HttpHeaders::new(),
        )
        .unwrap();
        assert_eq!(r.address().kind(), purpose);
        assert_eq!(
            r.original_url().as_str(),
            "https://example.test/a?x=1&x=2#f"
        );
        assert_eq!(r.current_url(), r.original_url());
        assert_eq!(
            r.transport_target().as_str(),
            "https://example.test/a?x=1&x=2"
        );
        assert_eq!(r.legacy_issued_url(route).unwrap(), issued);
        assert_eq!(r.method(), RequestMethod::Get);
        assert_eq!(r.redirect_policy(), RedirectPolicy::Follow);
        assert!(!r.has_body());
        assert_eq!(r.request_id(), 7);
        assert_eq!(
            r.headers().iter().collect::<Vec<_>>(),
            [("accept", b"*/*".as_slice())]
        );
        assert!(r.requesting_origin().is_none());
        assert!(r.initiator_url().is_none());
    }
}

#[test]
fn ai2_unsupported_methods_schemes_and_credentials_are_admission_errors() {
    assert_eq!(RequestMethod::try_from("GET"), Ok(RequestMethod::Get));
    for m in [
        "HEAD", "POST", "PUT", "DELETE", "OPTIONS", "CONNECT", "TRACE", "PATCH", "get", "SECRET",
        "",
    ] {
        assert_eq!(RequestMethod::try_from(m), Err(E::UnsupportedMethod));
    }
    for input in [
        "data:text/plain,x",
        "blob:https://example.test/id",
        "about:blank",
        "ws://example.test",
        "wss://example.test",
        "ftp://example.test",
        "javascript:secret",
        "custom:secret",
    ] {
        assert_eq!(
            request(input, Purpose::Document, HttpHeaders::new()).unwrap_err(),
            E::UnsupportedScheme
        );
    }
    for input in [
        "https://user@example.test",
        "http://:password@example.test",
        "https://user:password@example.test",
        "https://%75ser@example.test",
    ] {
        assert_eq!(
            request(input, Purpose::Document, HttpHeaders::new()).unwrap_err(),
            E::UrlCredentialsUnsupported
        );
    }
    // Empty authored userinfo normalizes away; admission uses AH components.
    assert!(
        request(
            "https://@example.test",
            Purpose::Document,
            HttpHeaders::new()
        )
        .is_ok()
    );
}

#[test]
fn ai2_header_admission_is_explicit_and_ordered() {
    let mut headers = HttpHeaders::new();
    headers.append("Accept-Language", b"en").unwrap();
    headers.append("Accept", b"text/plain").unwrap();
    let r = request("https://example.test/", Purpose::Image, headers.clone()).unwrap();
    assert_eq!(
        r.headers().iter().collect::<Vec<_>>(),
        [
            ("accept-language", b"en".as_slice()),
            ("accept", b"text/plain".as_slice())
        ]
    );
    headers.append("ACCEPT", b"image/png").unwrap();
    assert_eq!(
        request("https://example.test/", Purpose::Image, headers).unwrap_err(),
        E::RepeatedHeaderUnsupported
    );
    for name in [
        "Authorization",
        "Proxy-Authorization",
        "Cookie",
        "Referer",
        "Origin",
        "Host",
        "Content-Length",
        "Connection",
        "Accept-Encoding",
        "If-None-Match",
        "Sec-Fetch-Site",
        "X-Secret",
    ] {
        let mut h = HttpHeaders::new();
        h.append(name, b"secret").unwrap();
        assert_eq!(
            request("https://example.test/", Purpose::Image, h).unwrap_err(),
            E::UnsupportedHeader
        );
    }
    for value in [b" leading".as_slice(), b"trailing\t", &[0xff], b"\t"] {
        let mut h = HttpHeaders::new();
        h.append("accept", value).unwrap();
        assert_eq!(
            request("https://example.test/", Purpose::Image, h).unwrap_err(),
            E::UnsupportedHeaderValue
        );
    }
    let file = request(
        "file:///tmp/example#fragment",
        Purpose::Image,
        HttpHeaders::new(),
    )
    .unwrap();
    assert!(file.headers().is_empty());
    let mut h = HttpHeaders::new();
    h.append("accept", b"*/*").unwrap();
    assert_eq!(
        request("file:///tmp/example", Purpose::Image, h).unwrap_err(),
        E::HeadersNotApplicable
    );
}

#[test]
fn ai2_fragment_and_query_distinctions_are_not_rewritten() {
    for input in [
        "https://example.test/a",
        "https://example.test/a#",
        "https://example.test/a#f",
        "https://example.test/a?",
        "https://example.test/a?b=2&a=1&a=%31#f",
    ] {
        let r = request(input, Purpose::Document, HttpHeaders::new()).unwrap();
        assert_eq!(r.original_url().as_str(), input);
        assert_eq!(r.transport_target(), r.address().retrieval_key());
    }
}

#[test]
fn ai2_origin_context_clones_actual_opaque_identity() {
    let source = url("file:///private/source");
    let origin = source.url_origin();
    let independent = source.url_origin();
    let r = FetchRequest::try_new(
        1,
        ResourceAddress::new(url("https://target.test/a"), Purpose::Image),
        RequestMethod::Get,
        HttpHeaders::new(),
        Some(source.clone()),
        Some(origin.clone()),
    )
    .unwrap();
    assert_eq!(r.initiator_url(), Some(&source));
    assert_eq!(r.requesting_origin(), Some(&origin));
    assert_ne!(r.requesting_origin(), Some(&independent));
    assert_eq!(r.clone().requesting_origin(), Some(&origin));
}

#[test]
fn ai2_execution_routes_never_guess_unsupported_purpose() {
    for purpose in [
        Purpose::Document,
        Purpose::Hyperlink,
        Purpose::Stylesheet,
        Purpose::Image,
        Purpose::Font,
        Purpose::Media,
        Purpose::Script,
        Purpose::FormAction,
        Purpose::Unknown,
    ] {
        let r = request("https://example.test/a", purpose, HttpHeaders::new()).unwrap();
        for route in [Route::Html, Route::Css, Route::Image] {
            let allowed = matches!(
                (purpose, route),
                (Purpose::Document | Purpose::Hyperlink, Route::Html)
                    | (Purpose::Stylesheet, Route::Css)
                    | (Purpose::Image, Route::Image)
            );
            assert_eq!(r.validate_route(route).is_ok(), allowed);
            assert_eq!(r.legacy_issued_url(route).is_ok(), allowed);
        }
    }
}

#[test]
fn ai2_request_debug_is_literal_stable_and_redacted_when_nested() {
    let r = request(
        "https://private.test/SECRET?token=SECRET#SECRET",
        Purpose::Document,
        HttpHeaders::new(),
    )
    .unwrap();
    assert_eq!(
        format!("{r:?}"),
        "FetchRequest { request_id: 7, address: \"<redacted>\", purpose: Document, current_url: \"<redacted>\", method: Get, headers: [(\"accept\", \"<redacted>\")], initiator_url: None, requesting_origin: None, redirect_policy: Follow, body: \"absent\" }"
    );
    for out in [
        format!("{r:#?}"),
        format!("{:?}", Ok::<_, E>(Some(r.clone()))),
    ] {
        assert!(!out.contains("SECRET"));
        assert!(!out.contains("private.test"));
        assert!(out.len() < 2000);
    }
    let source = url("https://USER:PASS@private.test/SECRET?SECRET#SECRET");
    let contextual = FetchRequest::try_new(
        7,
        r.address().clone(),
        RequestMethod::Get,
        HttpHeaders::new(),
        Some(source.clone()),
        Some(source.url_origin()),
    )
    .unwrap();
    for out in [
        format!("{contextual:?}"),
        format!("{:#?}", Some(contextual)),
    ] {
        for secret in ["USER", "PASS", "private.test", "SECRET"] {
            assert!(!out.contains(secret));
        }
    }
}

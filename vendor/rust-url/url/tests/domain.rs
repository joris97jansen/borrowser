// Copyright The rust-url developers.
// Licensed under the Apache License, Version 2.0 or the MIT license.

//! WHATWG domain parsing and its strict ToASCII validation diagnostic.
#![no_std]

extern crate alloc;

use alloc::{format, vec::Vec};
use core::cell::RefCell;
use url::{Host, ParseError, SyntaxViolation, Url};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn parse(input: &str) -> (Result<Url, ParseError>, Vec<SyntaxViolation>) {
    let events = RefCell::new(Vec::new());
    let callback = |event| events.borrow_mut().push(event);
    let result = Url::options()
        .syntax_violation_callback(Some(&callback))
        .parse(input);
    (result, events.into_inner())
}

fn stable(url: &Url) {
    url.check_invariants().unwrap();
    let reparsed = Url::parse(url.as_str()).unwrap();
    reparsed.check_invariants().unwrap();
    assert_eq!(url, &reparsed);
    assert_eq!(url.host(), reparsed.host());
    assert_eq!(
        url.origin().ascii_serialization(),
        reparsed.origin().ascii_serialization()
    );
    assert_eq!(
        url.origin().unicode_serialization(),
        reparsed.origin().unicode_serialization()
    );
    assert_eq!(url.path(), reparsed.path());
    assert_eq!(url.query(), reparsed.query());
    assert_eq!(url.fragment(), reparsed.fragment());
}

#[test]
fn ascii_punycode_wpt_regressions() {
    // Exact identities from WPT c48d58747e1f211527fb695fd60548a997fae617,
    // url/resources/urltestdata.json, indices 310, 311, 313-316, 839, 921.
    // The diagnostic follows the independent strict domain-parser step.
    for (input, expected) in [
        ("http://a.b.c.xn--pokxncvks", "http://a.b.c.xn--pokxncvks/"),
        (
            "http://10.0.0.xn--pokxncvks",
            "http://10.0.0.xn--pokxncvks/",
        ),
        ("http://a.b.c.XN--pokxncvks", "http://a.b.c.xn--pokxncvks/"),
        ("http://a.b.c.Xn--pokxncvks", "http://a.b.c.xn--pokxncvks/"),
        (
            "http://10.0.0.XN--pokxncvks",
            "http://10.0.0.xn--pokxncvks/",
        ),
        (
            "http://10.0.0.xN--pokxncvks",
            "http://10.0.0.xn--pokxncvks/",
        ),
        ("file://xn--/p", "file://xn--/p"),
        ("https://xn--/", "https://xn--/"),
        ("https://%78%6e--/", "https://xn--/"),
        ("https://xn--8i7caa/", "https://xn--8i7caa/"),
    ] {
        let (result, events) = parse(input);
        let url = result.unwrap();
        assert_eq!(url.as_str(), expected);
        assert_eq!(events, [SyntaxViolation::DomainToAscii]);
        assert!(matches!(url.host(), Some(Host::Domain(_))));
        for _ in 0..3 {
            assert_eq!(parse(input), (Ok(url.clone()), events.clone()));
        }
        assert_eq!(Url::parse(input).unwrap(), url, "callback isolation");
        stable(&url);
    }
}

#[test]
fn domain_security_boundary() {
    for input in [
        "https://xn--.é/",
        "https://xn--8i7caa.é/",
        "https://xn--pokxncvks.é/",
        "https://xn--.%C3%A9/",
        "https://exa%23mple.test/",
        "https://%FF/",
        "https://%EF%BF%BD/",
        "https://a%00b/",
        "https://a%7Fb/",
        "https://a%25b/",
        "https://a\u{200d}b.test/",
        "https://aא.test/",
        "https://\u{0301}a.test/",
    ] {
        let (result, events) = parse(input);
        assert_eq!(result, Err(ParseError::IdnaError), "{input:?}");
        assert_eq!(events, [SyntaxViolation::DomainToAscii], "{input:?}");
        assert_eq!(Url::parse(input), result, "callback isolation");
    }
    assert_eq!(
        parse("https://xn--.123/").0,
        Err(ParseError::InvalidIpv4Address)
    );
    assert_eq!(
        parse("https://999.1/"),
        (Err(ParseError::InvalidIpv4Address), Vec::new())
    );
    assert_eq!(
        parse("https://[invalid]/"),
        (Err(ParseError::InvalidIpv6Address), Vec::new())
    );

    // This enumerates the Standard's forbidden domain bytes independently of
    // IDNA validity: other ASCII may be accepted with a strict diagnostic.
    for byte in 0u8..=127 {
        let host = format!("a%{byte:02X}b.test");
        let forbidden = byte <= b' ' || byte == 127 || b"#%/:<>?@[\\]^|".contains(&byte);
        assert_eq!(Host::parse(&host).is_err(), forbidden, "ASCII {byte}");
        let input = format!("https://{host}/");
        let (result, events) = parse(&input);
        assert_eq!(result.is_err(), forbidden, "ASCII {byte}");
        let decoded = format!("a{}b.test", char::from(byte));
        let strict_failure = idna::domain_to_ascii_strict(&decoded).is_err();
        assert_eq!(events.len(), usize::from(strict_failure), "ASCII {byte}");
        assert!(events
            .iter()
            .all(|event| *event == SyntaxViolation::DomainToAscii));
    }
    // The generic IDNA API remains a conversion/validation API.
    assert!(idna::domain_to_ascii("xn--").is_err());
    assert!(idna::domain_to_ascii("xn--8i7caa").is_err());
}

#[test]
fn strict_domain_validation_is_recoverable() {
    for host in ["a_b.test", "-a.test", "ab--cd.test", "a..test"] {
        assert!(idna::domain_to_ascii_strict(host).is_err());
        let (result, events) = parse(&format!("https://{host}/"));
        assert_eq!(result.unwrap().host_str(), Some(host));
        assert_eq!(events, [SyntaxViolation::DomainToAscii]);
    }
    let long_host = format!("{}.test", "a".repeat(64));
    let (result, events) = parse(&format!("https://{long_host}/"));
    assert_eq!(result.unwrap().host_str(), Some(long_host.as_str()));
    assert_eq!(events, [SyntaxViolation::DomainToAscii]);

    for (input, expected) in [
        ("https://faß.de/", "https://xn--fa-hia.de/"),
        ("https://%66%61%C3%9F.de/", "https://xn--fa-hia.de/"),
        ("https://ｗｗｗ.test/", "https://www.test/"),
        ("https://0x7f.1/", "https://127.0.0.1/"),
        ("https://[2001:0db8::1]/", "https://[2001:db8::1]/"),
        ("custom://XN--/", "custom://XN--/"),
    ] {
        let (result, events) = parse(input);
        let url = result.unwrap();
        assert_eq!(url.as_str(), expected);
        assert_eq!(events, []);
        stable(&url);
    }
}

#[test]
fn domain_unicode_serialization_preserves_identity() {
    for domain in ["xn--", "xn--8i7caa", "a.xn--pokxncvks"] {
        assert_eq!(url::quirks::domain_to_unicode(domain), domain);
        assert_eq!(url::quirks::domain_to_ascii(domain), domain);
        let url = Url::parse(&format!("https://{domain}/")).unwrap();
        assert_eq!(
            url.origin().ascii_serialization(),
            format!("https://{domain}")
        );
        assert_eq!(
            url.origin().unicode_serialization(),
            format!("https://{domain}")
        );
        stable(&url);
    }
    let url = Url::parse("https://faß.de/").unwrap();
    assert_eq!(url.origin().unicode_serialization(), "https://faß.de");
    assert_eq!(url::quirks::domain_to_unicode("xn--fa-hia.de"), "faß.de");
    assert_eq!(url::quirks::domain_to_unicode("xn--.é"), "");
    assert!(!Url::parse("file://xn--/p").unwrap().origin().is_tuple());
}

#[test]
fn domain_setters_preserve_failure_atomicity() {
    let mut url = Url::parse("https://original.test/path?query#fragment").unwrap();
    url.set_host(Some("XN--")).unwrap();
    assert_eq!(url.as_str(), "https://xn--/path?query#fragment");
    stable(&url);
    let original = url.clone();
    assert!(url.set_host(Some("xn--.é")).is_err());
    assert_eq!(url, original);
    url::quirks::set_host(&mut url, "xn--8i7caa:8443").unwrap();
    assert_eq!(url.host_str(), Some("xn--8i7caa"));
    assert_eq!(url.port(), Some(8443));
    stable(&url);
    url::quirks::set_hostname(&mut url, "XN--").unwrap();
    assert_eq!(url.host_str(), Some("xn--"));
    let original = url.clone();
    for host in ["xn--.é", "a%23b", "xn--:80"] {
        assert!(url::quirks::set_hostname(&mut url, host).is_err());
        assert_eq!(url, original);
    }
    let mut file = Url::parse("file://original.test/path").unwrap();
    url::quirks::set_host(&mut file, "XN--").unwrap();
    assert_eq!(file.as_str(), "file://xn--/path");
    stable(&file);
}

#[test]
fn domain_diagnostic_order_and_base_isolation() {
    use SyntaxViolation::{DomainToAscii, EmbeddedCredentials, PercentDecode, TabOrNewlineIgnored};
    let (result, events) = parse("https://user:pw@x\tn--/a%zz");
    assert!(result.is_ok());
    assert_eq!(
        events,
        [
            TabOrNewlineIgnored,
            EmbeddedCredentials,
            DomainToAscii,
            PercentDecode
        ]
    );
    assert_eq!(
        parse("https://xn--:bad/"),
        (Err(ParseError::InvalidPort), alloc::vec![DomainToAscii])
    );

    for base in ["https://xn--/dir/page", "file://xn--/dir/page"] {
        let base = Url::parse(base).unwrap();
        let original = base.clone();
        for (input, expected_events) in [("leaf", &[][..]), ("//xn--/leaf", &[DomainToAscii][..])] {
            let events = RefCell::new(Vec::new());
            let callback = |event| events.borrow_mut().push(event);
            let url = Url::options()
                .base_url(Some(&base))
                .syntax_violation_callback(Some(&callback))
                .parse(input)
                .unwrap();
            assert_eq!(*events.borrow(), expected_events);
            assert_eq!(base, original);
            assert_eq!(url, base.join(input).unwrap());
            stable(&url);
        }
    }
    assert_eq!(
        DomainToAscii.description(),
        "domain-to-ASCII validation error"
    );
}

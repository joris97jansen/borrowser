use addressing::{BrowserUrl, UrlDiagnostic as D, UrlFailureKind, UrlHost, UrlParseError as E};
use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

fn parsed(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).unwrap().url
}

#[test]
fn normalized_components_are_explicit() {
    let url = parsed("HTTPS://user:p%3Ass@BÜCHER.test:443/a/../b%2Fc?b=2&a=1&a=3#frag");
    assert_eq!(
        url.as_str(),
        "https://user:p%3Ass@xn--bcher-kva.test/b%2Fc?b=2&a=1&a=3#frag"
    );
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.username(), "user");
    assert_eq!(url.password(), Some("p%3Ass"));
    assert_eq!(url.host(), Some(UrlHost::Domain("xn--bcher-kva.test")));
    assert_eq!(url.port(), None);
    assert_eq!(url.path(), "/b%2Fc");
    assert_eq!(url.query(), Some("b=2&a=1&a=3"));
    assert_eq!(url.fragment(), Some("frag"));
    assert!(url.has_authority());
    assert!(!url.has_opaque_path());

    assert_eq!(
        parsed("HTTP://EXAMPLE.test:80").as_str(),
        "http://example.test/"
    );
    assert_eq!(parsed("https://example.test:0/").port(), Some(0));
    assert_eq!(parsed("https://example.test:65535/").port(), Some(65535));
    assert_eq!(
        parsed("http://0x7f.1/").host(),
        Some(UrlHost::Ipv4(Ipv4Addr::LOCALHOST))
    );
    assert_eq!(
        parsed("https://[0:0:0:0:0:0:0:1]/").host(),
        Some(UrlHost::Ipv6(Ipv6Addr::LOCALHOST))
    );
    assert_eq!(
        parsed("custom://EXAMPLE.test/").host(),
        Some(UrlHost::Opaque("EXAMPLE.test"))
    );
    for (input, authority, opaque) in [
        ("file:///tmp/a", true, false),
        ("custom:///x", true, false),
        ("custom:/x", false, false),
        ("about:blank", false, true),
    ] {
        let url = parsed(input);
        assert_eq!(url.host(), None);
        assert_eq!(url.has_authority(), authority);
        assert_eq!(url.has_opaque_path(), opaque);
    }
}

#[test]
fn absent_and_empty_components_and_credentials_follow_normalization() {
    for (suffix, query, fragment) in [
        ("", None, None),
        ("?", Some(""), None),
        ("#", None, Some("")),
        ("?#", Some(""), Some("")),
    ] {
        let url = parsed(&format!("https://h.test/{suffix}"));
        assert_eq!(url.query(), query);
        assert_eq!(url.fragment(), fragment);
    }
    for (input, expected, username, password) in [
        ("https://h.test/", "https://h.test/", "", None),
        ("https://u:@h.test/", "https://u@h.test/", "u", None),
        ("https://:p@h.test/", "https://:p@h.test/", "", Some("p")),
        ("https://@h.test/", "https://h.test/", "", None),
        ("https://:@h.test/", "https://h.test/", "", None),
        (
            "https://u%40x:p%3Ass@h.test/",
            "https://u%40x:p%3Ass@h.test/",
            "u%40x",
            Some("p%3Ass"),
        ),
    ] {
        let url = parsed(input);
        assert_eq!(url.as_str(), expected);
        assert_eq!(url.username(), username);
        assert_eq!(url.password(), password);
    }
}

#[test]
fn explicit_base_resolution_preserves_the_base_and_reference_semantics() {
    let base = parsed("https://u:p@example.test/docs/chapter/page.html?old#before");
    for (reference, expected) in [
        (
            "../images/logo.png",
            "https://u:p@example.test/docs/images/logo.png",
        ),
        (
            "/images/logo.png",
            "https://u:p@example.test/images/logo.png",
        ),
        ("//cdn.test:443/logo.png", "https://cdn.test/logo.png"),
        (
            "?x=1&x=2",
            "https://u:p@example.test/docs/chapter/page.html?x=1&x=2",
        ),
        (
            "#after",
            "https://u:p@example.test/docs/chapter/page.html?old#after",
        ),
        ("", "https://u:p@example.test/docs/chapter/page.html?old"),
        ("HTTP://OTHER.test:80/a", "http://other.test/a"),
        (
            "%2e%2e/images/a%20b.png",
            "https://u:p@example.test/docs/images/a%20b.png",
        ),
    ] {
        let first = base.resolve(reference).unwrap();
        assert_eq!(first.url.as_str(), expected);
        for _ in 0..3 {
            assert_eq!(base.resolve(reference).unwrap(), first);
        }
        assert_eq!(parsed(first.url.as_str()), first.url);
    }
    assert_eq!(
        base.as_str(),
        "https://u:p@example.test/docs/chapter/page.html?old#before"
    );
    assert_eq!(
        base.resolve("a%zz\\b").unwrap().diagnostics,
        vec![D::InvalidPercentEncoding, D::Backslash]
    );
    let opaque = parsed("about:blank");
    assert_eq!(opaque.resolve("#x").unwrap().url.as_str(), "about:blank#x");
    assert_eq!(
        opaque.resolve("image.png").unwrap_err().kind,
        UrlFailureKind::InvalidInput(E::RelativeAgainstOpaqueBase)
    );
    assert_eq!(
        opaque.resolve("https://h.test/").unwrap().url.as_str(),
        "https://h.test/"
    );
    let original_opaque = opaque.clone();
    let empty_error = opaque.resolve("").unwrap_err();
    assert_eq!(
        empty_error.kind,
        UrlFailureKind::InvalidInput(E::RelativeAgainstOpaqueBase)
    );
    assert!(empty_error.diagnostics.is_empty());
    assert_eq!(empty_error.base.as_ref(), Some(&opaque));
    assert_eq!(opaque, original_opaque);
    for _ in 0..3 {
        assert_eq!(opaque.resolve("").unwrap_err(), empty_error);
        assert_eq!(opaque, original_opaque);
    }
}

#[test]
fn url_equality_and_hashing_use_complete_serialization_not_origin_or_fetch_identity() {
    for (left, right, equal) in [
        ("HTTP://H.test:80", "http://h.test/", true),
        ("https://h.test:443/a/../b", "https://h.test/b", true),
        ("https://u:@h.test/", "https://u@h.test/", true),
        ("https://u:p@h.test/", "https://u:q@h.test/", false),
        ("https://u:p@h.test/", "https://v:p@h.test/", false),
        ("https://h.test/a", "https://h.test/b", false),
        ("https://h.test/#one", "https://h.test/#two", false),
        ("https://h.test/?a=1&b=2", "https://h.test/?b=2&a=1", false),
        ("https://h.test/", "https://h.test/?", false),
        ("https://h.test/", "https://h.test/#", false),
        ("https://h.test/%2f", "https://h.test/%2F", false),
        ("custom:/x", "custom:///x", false),
    ] {
        let a = parsed(left);
        let b = parsed(right);
        assert_eq!(a == b, equal, "{left} versus {right}");
        assert_eq!(HashSet::from([a, b]).len(), if equal { 1 } else { 2 });
    }
    let recovered = BrowserUrl::parse(" https://h.test/").unwrap();
    assert!(!recovered.diagnostics.is_empty());
    assert_eq!(recovered.url, parsed("https://h.test/"));
}

#[test]
fn recoverable_events_are_ordered_repeated_and_separate_from_values() {
    let input = " http:////u:p@exa\tmple.test/a\\b\\c%zz%yy#x\0y";
    let expected = vec![
        D::BoundaryControlOrSpaceIgnored,
        D::TabOrNewlineIgnored,
        D::ExpectedAuthoritySlashes,
        D::EmbeddedCredentials,
        D::Backslash,
        D::Backslash,
        D::InvalidPercentEncoding,
        D::InvalidPercentEncoding,
        D::NullInFragment,
    ];
    let first = BrowserUrl::parse(input).unwrap();
    assert_eq!(
        first.url.as_str(),
        "http://u:p@example.test/a/b/c%zz%yy#x%00y"
    );
    assert_eq!(first.diagnostics, expected);
    for _ in 0..3 {
        assert_eq!(BrowserUrl::parse(input).unwrap(), first);
    }
    assert_eq!(parsed(first.url.as_str()), first.url);
    for (input, events) in [
        (
            "https://u@@h.test/",
            vec![D::EmbeddedCredentials, D::UnencodedAtSign],
        ),
        ("file:/tmp/a", vec![D::ExpectedFileSlashes]),
        ("https://h.test/<", vec![D::NonUrlCodePoint]),
        ("file://host/C:/", vec![D::FileHostWithDriveLetter]),
    ] {
        assert_eq!(BrowserUrl::parse(input).unwrap().diagnostics, events);
    }
    // Known omissions, not evidence of validity: no invented callbacks.
    for input in ["http://0x7f.1/", "custom://%/"] {
        assert!(BrowserUrl::parse(input).unwrap().diagnostics.is_empty());
    }
}

#[test]
fn fatal_errors_retain_prior_events_without_raw_input() {
    for (input, expected) in [
        ("", E::RelativeWithoutBase),
        ("../x", E::RelativeWithoutBase),
        ("https://:443/", E::EmptyHost),
        ("https://h.test:65536/", E::InvalidPort),
        ("https://h.test:abc/", E::InvalidPort),
        ("https://256.0.0.1/", E::InvalidIpv4Address),
        ("https://[::1/", E::InvalidIpv6Address),
        ("https://exa mple.test/", E::InvalidDomain),
        ("custom://a b/", E::InvalidDomainCharacter),
    ] {
        let first = BrowserUrl::parse(input).unwrap_err();
        assert_eq!(first.kind, UrlFailureKind::InvalidInput(expected));
        assert_eq!(BrowserUrl::parse(input).unwrap_err(), first);
    }
    let error =
        BrowserUrl::parse(" https://secret-user:secret-password@h.test:70000/").unwrap_err();
    assert_eq!(error.kind, UrlFailureKind::InvalidInput(E::InvalidPort));
    assert_eq!(
        error.diagnostics,
        vec![D::BoundaryControlOrSpaceIgnored, D::EmbeddedCredentials]
    );
    assert!(!format!("{error:?} {error}").contains("secret"));
    assert_eq!(error.base, None);
    let base = parsed("https://base-secret:password-secret@h.test/path");
    let error = base.resolve("https://:443/").unwrap_err();
    assert_eq!(error.base.as_ref(), Some(&base));
    assert_eq!(error.kind, UrlFailureKind::InvalidInput(E::EmptyHost));
    assert!(!format!("{error:?} {error:#?} {error}").contains("secret"));

    let original_base = base.clone();
    let reference = "//user:password@authority.test:70000/path";
    let error = base.resolve(reference).unwrap_err();
    assert_eq!(error.kind, UrlFailureKind::InvalidInput(E::InvalidPort));
    assert_eq!(error.diagnostics, vec![D::EmbeddedCredentials]);
    assert_eq!(error.base.as_ref(), Some(&base));
    assert_eq!(base, original_base);
    for _ in 0..3 {
        assert_eq!(base.resolve(reference).unwrap_err(), error);
        assert_eq!(base, original_base);
    }
}

#[test]
fn debug_is_deterministic_and_redacts_contents_without_mutating_identity() {
    let outcome = BrowserUrl::parse("https://secret-user:p%3Ass@h.test:8443/a?x#f").unwrap();
    let expected = "BrowserUrl { scheme: \"https\", username: \"<redacted>\", password: Some(\"<redacted>\"), host_kind: Some(\"domain\"), port: Some(\"<redacted>\"), path: \"<redacted>\", query: Some(\"<redacted>\"), fragment: Some(\"<redacted>\"), authority: true, opaque_path: false }";
    assert_eq!(format!("{:?}", outcome.url), expected);
    for input in [
        "https://secret-user@h.test/",
        "https://:secret-password@h.test/",
        "https://secret%40user:secret%3Apassword@h.test/",
    ] {
        let parsed = BrowserUrl::parse(input).unwrap();
        assert!(!format!("{parsed:?} {parsed:#?}").contains("secret"));
        assert_eq!(parsed.url.as_str(), input);
    }
    assert_eq!(
        outcome.url.as_str(),
        "https://secret-user:p%3Ass@h.test:8443/a?x#f"
    );
    assert_ne!(
        outcome.url,
        parsed("https://secret-user:other@h.test:8443/a?x#f")
    );
}

#[test]
fn parsing_does_not_imply_operation_support_or_origin_computation() {
    for input in [
        "file:///tmp/a",
        "file://server/share/a",
        "ftp://h.test/a",
        "javascript:1+1",
        "data:text/plain,a",
        "about:blank",
        "custom:payload",
        "blob:https://h.test/id",
    ] {
        let url = parsed(input);
        assert_eq!(url.as_str(), input);
        assert_eq!(parsed(url.as_str()), url);
    }
}

#[test]
fn maintained_dependency_satisfies_previous_deviation_cases() {
    // Literal expectations retained from the independent URL Standard review.
    // Historical url 2.5.7 observations remain in the AH2 contract.
    for (input, expected) in [
        ("file://example.net/C:/", "file://example.net/C:/"),
        ("file:///w|/m", "file:///w:/m"),
        ("file://host//w|/m", "file://host//w|/m"),
        ("file:////w|/m", "file:////w|/m"),
        ("file://spider///", "file://spider///"),
        ("file:////foo", "file:////foo"),
        ("http://example.test/a^b", "http://example.test/a%5Eb"),
        ("non-special:opaque  ?hi", "non-special:opaque %20?hi"),
        ("https://xn--/", "https://xn--/"),
    ] {
        let first = parsed(input);
        assert_eq!(first.as_str(), expected, "{input:?}");
        assert_eq!(parsed(first.as_str()), first, "{input:?}: fixed point");
    }
    assert_eq!(
        parsed("file://example.net/C:/").host(),
        Some(UrlHost::Domain("example.net"))
    );
}

#[test]
fn domain_to_ascii_validation_is_recoverable_and_ordered() {
    let accepted = BrowserUrl::parse("https://xn--/").unwrap();
    assert_eq!(accepted.url.as_str(), "https://xn--/");
    assert_eq!(accepted.diagnostics, vec![D::DomainToAscii]);
    assert!(
        BrowserUrl::parse("https://bücher.test/")
            .unwrap()
            .diagnostics
            .is_empty()
    );

    let input = " https://u:p@xn--/a%zz";
    let first = BrowserUrl::parse(input).unwrap();
    assert_eq!(first.url.as_str(), "https://u:p@xn--/a%zz");
    assert_eq!(
        first.diagnostics,
        vec![
            D::BoundaryControlOrSpaceIgnored,
            D::EmbeddedCredentials,
            D::DomainToAscii,
            D::InvalidPercentEncoding,
        ]
    );
    for _ in 0..3 {
        assert_eq!(BrowserUrl::parse(input).unwrap(), first);
    }

    let base = BrowserUrl::parse("https://u:p@xn--/dir/page").unwrap();
    assert_eq!(
        base.diagnostics,
        vec![D::EmbeddedCredentials, D::DomainToAscii]
    );
    let original = base.url.clone();
    let resolved = base.url.resolve("child").unwrap();
    assert_eq!(resolved.url.as_str(), "https://u:p@xn--/dir/child");
    assert!(
        resolved.diagnostics.is_empty(),
        "base callbacks must not be replayed"
    );
    assert_eq!(
        base.url.resolve("//xn--/new").unwrap().diagnostics,
        vec![D::DomainToAscii]
    );
    assert_eq!(base.url, original);
}

#[test]
fn domain_to_ascii_validation_does_not_replace_fatal_errors() {
    let invalid = BrowserUrl::parse("https://exa mple.test/").unwrap_err();
    assert_eq!(invalid.kind, UrlFailureKind::InvalidInput(E::InvalidDomain));
    assert_eq!(invalid.diagnostics, vec![D::DomainToAscii]);

    let base = parsed("https://base-user:base-password@h.test/dir/page");
    let original = base.clone();
    for _ in 0..3 {
        let error = base
            .resolve("https://input-user:input-password@xn--:70000/")
            .unwrap_err();
        assert_eq!(error.kind, UrlFailureKind::InvalidInput(E::InvalidPort));
        assert_eq!(
            error.diagnostics,
            vec![D::EmbeddedCredentials, D::DomainToAscii]
        );
        assert_eq!(error.base.as_ref(), Some(&base));
        let debug = format!("{error:?} {error:#?} {error}");
        assert!(!debug.contains("base-user") && !debug.contains("base-password"));
        assert!(!debug.contains("input-user") && !debug.contains("input-password"));
        assert_eq!(base, original);
    }
}

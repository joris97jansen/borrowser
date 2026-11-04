use addressing::{BrowserUrl, UrlDiagnostic as D, UrlFailureKind, UrlParseError};

#[test]
fn unavailable_base_preserves_ambiguity_and_known_base_resolution() {
    let base = BrowserUrl::parse("https://example.test/css/main.css")
        .unwrap()
        .url;
    assert_eq!(
        base.resolve("https:icons/logo.svg").unwrap().url.as_str(),
        "https://example.test/css/icons/logo.svg"
    );
    assert_eq!(base.resolve("https:").unwrap().url, base);
    assert!(BrowserUrl::parse("https:icons/logo.svg").is_ok());
    assert!(BrowserUrl::parse("https:").is_err());
    for reference in [
        "https:icons/logo.svg",
        "https:",
        "icons/logo.svg",
        "file:icons/logo.svg",
        "file:/icons/logo.svg",
        "file:",
        " https:",
        "ht\ttps:",
    ] {
        assert_eq!(
            BrowserUrl::parse_if_base_independent(reference).unwrap(),
            None
        );
    }
    let file = BrowserUrl::parse("file://server/C:/css/main.css")
        .unwrap()
        .url;
    assert_eq!(
        file.resolve("file:icons/logo.svg").unwrap().url.as_str(),
        "file://server/C:/css/icons/logo.svg"
    );
    assert_eq!(
        file.resolve("file:/icons/logo.svg").unwrap().url.as_str(),
        "file://server/C:/icons/logo.svg"
    );
}

#[test]
fn independent_success_failure_and_diagnostics_are_production_results() {
    for reference in [
        "https://cdn.test/a",
        "file:///icons/logo.svg",
        "data:payload",
        "custom:/path",
        " https://u:p@h.test/a%zz%yy",
        "https://[::1",
        " https://u:p@[::1",
        " ht\ttps://h.test/a%zz%zz",
    ] {
        assert_eq!(
            BrowserUrl::parse_if_base_independent(reference),
            BrowserUrl::parse(reference).map(Some)
        );
    }
    let result = BrowserUrl::parse_if_base_independent(" https://u:p@h.test/a%zz%yy")
        .unwrap()
        .unwrap();
    assert_eq!(
        result.diagnostics,
        [
            D::BoundaryControlOrSpaceIgnored,
            D::EmbeddedCredentials,
            D::InvalidPercentEncoding,
            D::InvalidPercentEncoding
        ]
    );
    let failure = BrowserUrl::parse_if_base_independent("https://[::1").unwrap_err();
    assert_eq!(
        failure.kind,
        UrlFailureKind::InvalidInput(UrlParseError::InvalidIpv6Address)
    );
    assert!(failure.base.is_none());
    for _ in 0..3 {
        assert_eq!(
            BrowserUrl::parse_if_base_independent("https://[::1").unwrap_err(),
            failure
        );
    }
}

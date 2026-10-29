use addressing::{
    BrowserUrl, UrlDiagnostic as D, UrlFailureKind, UrlParseError as E, format_resolution_debug,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn parsed(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).unwrap().url
}

fn identity_hash(url: &BrowserUrl) -> u64 {
    let mut state = DefaultHasher::new();
    url.hash(&mut state);
    state.finish()
}

#[test]
fn successful_resolution_snapshot_preserves_exact_semantics_separately() {
    let base_outcome = BrowserUrl::parse(
        "https://base-user:base-password@private.test:8443/private/base?base-token=one#base-fragment",
    )
    .unwrap();
    assert_eq!(base_outcome.diagnostics, vec![D::EmbeddedCredentials]);
    let base = base_outcome.url;
    let original = base.clone();
    let original_hash = identity_hash(&base);
    let reference = "../resolved-secret?access_token=query-secret#fragment-secret";
    let result = base.resolve(reference);
    let before = result.clone();
    let expected = r#"UrlResolution {
    input: "<redacted>",
    base: Some(BrowserUrl { scheme: "https", username: "<redacted>", password: Some("<redacted>"), host_kind: Some("domain"), port: Some("<redacted>"), path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: true, opaque_path: false }),
    resolved: Some(BrowserUrl { scheme: "https", username: "<redacted>", password: Some("<redacted>"), host_kind: Some("domain"), port: Some("<redacted>"), path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: true, opaque_path: false }),
    diagnostics: [],
    failure: None,
    failure_base_matches: None,
}"#;
    for _ in 0..3 {
        assert_eq!(
            format_resolution_debug(reference, Some(&base), &result),
            expected
        );
        assert_eq!(result, before);
        assert_eq!(base.resolve(reference), result);
        assert_eq!(base, original);
        assert_eq!(identity_hash(&base), original_hash);
    }
    // Public, synthetic fixtures can assert exact conformance independently of
    // the production diagnostic projection. No raw-mode formatter is needed.
    let resolved = &result.as_ref().unwrap().url;
    assert_eq!(
        resolved.as_str(),
        "https://base-user:base-password@private.test:8443/resolved-secret?access_token=query-secret#fragment-secret"
    );
    assert_eq!(resolved.path(), "/resolved-secret");
    assert_eq!(resolved.query(), Some("access_token=query-secret"));
    assert_eq!(resolved.fragment(), Some("fragment-secret"));
    assert_eq!(parsed(resolved.as_str()), *resolved);
    assert_eq!(identity_hash(resolved), identity_hash(&before.unwrap().url));
}

#[test]
fn failed_resolution_snapshot_retains_prior_events_and_fatal_category() {
    let base = parsed("https://private.test/private-base");
    let reference = " https://input-user:input-password@xn--:70000/private?token=private#private";
    let result = base.resolve(reference);
    let before = result.clone();
    let expected = r#"UrlResolution {
    input: "<redacted>",
    base: Some(BrowserUrl { scheme: "https", username: "", password: None, host_kind: Some("domain"), port: None, path: "<redacted>", query: None, fragment: None, authority: true, opaque_path: false }),
    resolved: None,
    diagnostics: [BoundaryControlOrSpaceIgnored, EmbeddedCredentials, DomainToAscii],
    failure: Some(InvalidInput(InvalidPort)),
    failure_base_matches: Some(true),
}"#;
    for _ in 0..3 {
        assert_eq!(
            format_resolution_debug(reference, Some(&base), &result),
            expected
        );
        assert_eq!(result, before);
        assert_eq!(base.resolve(reference), result);
    }
    let error = result.unwrap_err();
    assert_eq!(error.kind, UrlFailureKind::InvalidInput(E::InvalidPort));
    assert_eq!(error.base.as_ref(), Some(&base));
    assert_eq!(
        error.to_string(),
        "URL operation failed: InvalidInput(InvalidPort)"
    );
}

#[test]
fn empty_reference_snapshot_distinguishes_empty_from_withheld_input() {
    let base = parsed("https://private.test/private#private");
    let result = base.resolve("");
    assert_eq!(
        format_resolution_debug("", Some(&base), &result),
        r#"UrlResolution {
    input: "",
    base: Some(BrowserUrl { scheme: "https", username: "", password: None, host_kind: Some("domain"), port: None, path: "<redacted>", query: None, fragment: Some("<redacted>"), authority: true, opaque_path: false }),
    resolved: Some(BrowserUrl { scheme: "https", username: "", password: None, host_kind: Some("domain"), port: None, path: "<redacted>", query: None, fragment: None, authority: true, opaque_path: false }),
    diagnostics: [],
    failure: None,
    failure_base_matches: None,
}"#
    );
    assert_eq!(result.unwrap().url.as_str(), "https://private.test/private");
}

#[test]
fn missing_base_snapshot_and_failure_consistency_use_actual_url_equality() {
    let reference = "../private?token=private#private";
    let result = BrowserUrl::parse(reference);
    assert_eq!(
        format_resolution_debug(reference, None, &result),
        r#"UrlResolution {
    input: "<redacted>",
    base: None,
    resolved: None,
    diagnostics: [],
    failure: Some(InvalidInput(RelativeWithoutBase)),
    failure_base_matches: Some(true),
}"#
    );
    let base = parsed("https://one.test/private");
    let other = parsed("https://two.test/different");
    assert_eq!(format!("{base:?}"), format!("{other:?}"));
    assert_ne!(base, other);
    let failed = base.resolve("https://:443/");
    assert!(
        format_resolution_debug("https://:443/", Some(&other), &failed)
            .contains("failure_base_matches: Some(false)")
    );
    assert!(
        format_resolution_debug("https://:443/", None, &failed)
            .contains("failure_base_matches: Some(false)")
    );
    assert!(
        format_resolution_debug(reference, Some(&base), &result)
            .contains("failure_base_matches: Some(false)")
    );
    // Success has no retained base with which to authenticate supplied context.
    assert!(
        format_resolution_debug("different", Some(&other), &base.resolve("child"))
            .contains("failure_base_matches: None")
    );
}

#[test]
fn opaque_resolution_snapshot_hides_payload_and_preserves_failure_distinction() {
    let base = parsed("data:private-payload?token=private#private");
    let result = base.resolve("#private-next");
    assert_eq!(
        format_resolution_debug("#private-next", Some(&base), &result),
        r#"UrlResolution {
    input: "<redacted>",
    base: Some(BrowserUrl { scheme: "data", username: "", password: None, host_kind: None, port: None, path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: false, opaque_path: true }),
    resolved: Some(BrowserUrl { scheme: "data", username: "", password: None, host_kind: None, port: None, path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: false, opaque_path: true }),
    diagnostics: [],
    failure: None,
    failure_base_matches: None,
}"#
    );
    let failed = base.resolve("private-child");
    assert_eq!(
        failed.as_ref().unwrap_err().kind,
        UrlFailureKind::InvalidInput(E::RelativeAgainstOpaqueBase)
    );
    assert!(
        format_resolution_debug("private-child", Some(&base), &failed)
            .contains("failure: Some(InvalidInput(RelativeAgainstOpaqueBase))")
    );
}

#[test]
fn component_presence_and_nested_pretty_debug_have_explicit_snapshots() {
    let outcome = BrowserUrl::parse("https://private.test/?#").unwrap();
    assert_eq!(
        format!("{outcome:#?}"),
        r#"UrlParseOutcome {
    url: BrowserUrl {
        scheme: "https",
        username: "",
        password: None,
        host_kind: Some(
            "domain",
        ),
        port: None,
        path: "<redacted>",
        query: Some(
            "",
        ),
        fragment: Some(
            "",
        ),
        authority: true,
        opaque_path: false,
    },
    diagnostics: [],
}"#
    );
    let failed = outcome.url.resolve("https://:443/").unwrap_err();
    assert_eq!(
        format!("{failed:#?}"),
        r#"UrlFailure {
    kind: InvalidInput(
        EmptyHost,
    ),
    diagnostics: [],
    base: Some(
        BrowserUrl {
            scheme: "https",
            username: "",
            password: None,
            host_kind: Some(
                "domain",
            ),
            port: None,
            path: "<redacted>",
            query: Some(
                "",
            ),
            fragment: Some(
                "",
            ),
            authority: true,
            opaque_path: false,
        },
    ),
}"#
    );
    let empty_path = parsed("custom-private:");
    assert_eq!(
        format!("{empty_path:?}"),
        r#"BrowserUrl { scheme: "<redacted>", username: "", password: None, host_kind: None, port: None, path: "", query: None, fragment: None, authority: false, opaque_path: true }"#
    );
}

#[test]
fn secret_substitution_preserves_structural_debug_without_collapsing_url_identity() {
    for (left, right) in [
        (
            "https://u:p@one.test:1234/short?q=one#one",
            "https://long-user:long-password@longer-host.test:9876/longer-path?different=long-value#long-fragment",
        ),
        (
            "https://192.0.2.1:1234/private",
            "https://198.51.100.25:9876/different",
        ),
        (
            "https://[2001:db8::1]:1234/private",
            "https://[2001:db8:1::abcd]:9876/different",
        ),
        (
            "secret-one://private-one:1234/private?q=one#one",
            "secret-longer://private-longer:9876/different?different=longer#longer",
        ),
        ("data:one-secret", "data:a-much-longer-secret"),
        (
            "blob:https://one.test/private-token",
            "blob:https://longer.test/different-token",
        ),
        (
            "file:///Users/one/private",
            "file:///home/another/private-file",
        ),
    ] {
        let left_result = BrowserUrl::parse(left);
        let right_result = BrowserUrl::parse(right);
        assert_ne!(
            left_result.as_ref().unwrap().url,
            right_result.as_ref().unwrap().url
        );
        assert_eq!(
            left_result.as_ref().unwrap().diagnostics,
            right_result.as_ref().unwrap().diagnostics
        );
        assert_eq!(format!("{left_result:?}"), format!("{right_result:?}"));
        assert_eq!(format!("{left_result:#?}"), format!("{right_result:#?}"));
        assert_eq!(
            format_resolution_debug(left, None, &left_result),
            format_resolution_debug(right, None, &right_result)
        );
    }
}

#[test]
fn malformed_and_encoded_inputs_cannot_escape_redaction_or_outlive_their_owner() {
    let base = parsed(
        "https://base-private:password-private@host-private.test:4321/path-private?query-private#fragment-private",
    );
    for input in [
        "https://user-private:password-private@host-private.test:bad/path-private?token-private#fragment-private",
        "https://user%40private:password%3Aprivate@host-private.test/path-private?token-private#fragment-private",
        "https://host private.test/path-private?token-private#fragment-private",
        "https://[private-ip]/path-private?token-private#fragment-private",
        "//user-private:password-private@host-private.test:70000/path-private",
        "data:payload-private",
        "custom-private://host-private.test/path-private",
        "https://host-private.test/é-private\npath?token-private#fragment-private",
    ] {
        let (result, text) = {
            let owned_reference = input.to_owned();
            let result = base.resolve(&owned_reference);
            let text = format_resolution_debug(&owned_reference, Some(&base), &result);
            (result, text)
        };
        let nested = format!("{result:?} {result:#?} {base:?} {base:#?}");
        assert!(!text.contains("private"));
        assert!(!nested.contains("private"));
        assert!(!text.contains("4321"));
        assert!(!nested.contains("4321"));
        if let Err(error) = &result {
            assert!(!error.to_string().contains("private"));
        }
        assert_eq!(base.resolve(input), result);
        assert_eq!(format_resolution_debug(input, Some(&base), &result), text);
    }
}

#[test]
fn recoverable_order_and_repetitions_are_not_replayed_from_base_construction() {
    let base = BrowserUrl::parse(" https://u:p@xn--/private").unwrap();
    assert_eq!(
        base.diagnostics,
        vec![
            D::BoundaryControlOrSpaceIgnored,
            D::EmbeddedCredentials,
            D::DomainToAscii
        ]
    );
    let result = base.url.resolve("child%zz%yy");
    assert_eq!(
        result.as_ref().unwrap().diagnostics,
        vec![D::InvalidPercentEncoding, D::InvalidPercentEncoding]
    );
    let text = format_resolution_debug("child%zz%yy", Some(&base.url), &result);
    assert!(text.contains("diagnostics: [InvalidPercentEncoding, InvalidPercentEncoding]"));
    assert!(!text.contains("EmbeddedCredentials"));
    assert!(!text.contains("DomainToAscii"));
    assert!(!text.contains("BoundaryControlOrSpaceIgnored"));
    assert_eq!(
        format!("{:?}", result.as_ref().unwrap().diagnostics),
        "[InvalidPercentEncoding, InvalidPercentEncoding]"
    );
}

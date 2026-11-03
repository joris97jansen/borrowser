use addressing::{BrowserUrl, ResourceAddress, ResourceKind};

#[test]
fn script_debug_uses_fixed_kind_and_existing_redaction() {
    let address = ResourceAddress::new(
        BrowserUrl::parse("https://private.test/private.js?secret#private")
            .unwrap()
            .url,
        ResourceKind::Script,
    );
    assert_eq!(
        format!("{address:?}"),
        r#"ResourceAddress { url: BrowserUrl { scheme: "https", username: "", password: None, host_kind: Some("domain"), port: None, path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: true, opaque_path: false }, kind: Script }"#,
    );
    assert!(!format!("{address:#?}").contains("private"));
}

#[test]
fn resource_debug_is_deterministic_redacted_and_separate_from_semantics() {
    let address = ResourceAddress::new(
        BrowserUrl::parse(
            "https://SECRET_USER:SECRET_PASSWORD@private.test:8443/private?token=secret#hidden",
        )
        .unwrap()
        .url,
        ResourceKind::Stylesheet,
    );
    let key = address.retrieval_key();
    assert_eq!(
        format!("{address:?}"),
        r#"ResourceAddress { url: BrowserUrl { scheme: "https", username: "<redacted>", password: Some("<redacted>"), host_kind: Some("domain"), port: Some("<redacted>"), path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: true, opaque_path: false }, kind: Stylesheet }"#
    );
    assert_eq!(
        format!("{key:?}"),
        r#"ResourceIdentityKey { url: "<redacted>" }"#
    );
    assert_eq!(
        format!("{key:#?}"),
        "ResourceIdentityKey {\n    url: \"<redacted>\",\n}"
    );
    assert_eq!(
        format!("{:?}", Some(&key)),
        r#"Some(ResourceIdentityKey { url: "<redacted>" })"#
    );
    for _ in 0..3 {
        for output in [
            format!("{address:#?}"),
            format!("{:?}", Ok::<_, ()>(&address)),
            format!("{key:?}"),
        ] {
            for secret in [
                "SECRET_USER",
                "SECRET_PASSWORD",
                "private.test",
                "8443",
                "/private",
                "token",
                "secret",
                "hidden",
            ] {
                assert!(!output.contains(secret));
            }
        }
    }
    assert_eq!(
        key.as_str(),
        "https://SECRET_USER:SECRET_PASSWORD@private.test:8443/private?token=secret"
    );
    let opaque = ResourceAddress::new(
        BrowserUrl::parse("private-scheme:private-payload#private-fragment")
            .unwrap()
            .url,
        ResourceKind::Unknown,
    );
    assert!(!format!("{opaque:?}").contains("private"));
}

use addressing::BrowserUrl;

#[test]
fn origin_debug_is_structural_in_ordinary_pretty_and_nested_output() {
    let tuple = BrowserUrl::parse(
        "https://private-user:private-password@private.test:8443/private?token=secret#hidden",
    )
    .unwrap()
    .url
    .url_origin();
    assert_eq!(
        format!("{tuple:?}"),
        r#"Origin { kind: "tuple", scheme: "https", host_kind: "domain", port: Some("<redacted>") }"#
    );
    assert_eq!(
        format!("{tuple:#?}"),
        "Origin {\n    kind: \"tuple\",\n    scheme: \"https\",\n    host_kind: \"domain\",\n    port: Some(\n        \"<redacted>\",\n    ),\n}"
    );
    assert_eq!(format!("{:?}", Some(&tuple)), format!("Some({tuple:?})"));
    for _ in 0..3 {
        for input in [
            "file:///private/path",
            "data:text/plain,secret",
            "custom-secret:payload",
            "blob:blob:https://private.test/id",
        ] {
            let a = BrowserUrl::parse(input).unwrap().url.url_origin();
            assert_eq!(format!("{a:?}"), r#"Origin { kind: "opaque" }"#);
            assert_eq!(format!("{a:#?}"), "Origin {\n    kind: \"opaque\",\n}");
            assert_eq!(
                format!("{:?}", Ok::<_, ()>(a)),
                "Ok(Origin { kind: \"opaque\" })"
            );
        }
    }
    for (input, expected) in [("http://127.1", "ipv4"), ("https://[::1]", "ipv6")] {
        let a = BrowserUrl::parse(input).unwrap().url.url_origin();
        assert_eq!(
            format!("{a:?}"),
            format!(
                "Origin {{ kind: \"tuple\", scheme: \"{}\", host_kind: \"{expected}\", port: None }}",
                input.split(':').next().unwrap()
            )
        );
    }
}

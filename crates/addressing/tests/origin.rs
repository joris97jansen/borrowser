use addressing::{BrowserUrl, Origin};

fn origin(input: &str) -> Origin {
    BrowserUrl::parse(input).unwrap().url.url_origin()
}

#[test]
fn tuple_comparison_uses_normalized_authority_only() {
    let a = origin("https://user:secret@EXAMPLE.test:443/a?x#f");
    for input in [
        "https://example.test/",
        "https://other:password@example.test/b?q#other",
    ] {
        let b = origin(input);
        assert!(a.is_same_origin(&b));
        assert!(b.is_same_origin(&a));
        assert_eq!(a, b);
    }
    for input in [
        "http://example.test/",
        "https://other.test/",
        "https://example.test:444/",
        "https://example.test:0/",
        "https://example.test./",
    ] {
        let b = origin(input);
        assert!(!a.is_same_origin(&b));
        assert!(!b.is_same_origin(&a));
        assert_ne!(a, b);
    }
    assert_eq!(a.serialize(), "https://example.test");
    assert_eq!(
        origin("https://example.test:0/").serialize(),
        "https://example.test:0"
    );
}

#[test]
fn all_tuple_schemes_have_default_port_equivalence() {
    for (scheme, port) in [
        ("http", 80),
        ("https", 443),
        ("ftp", 21),
        ("ws", 80),
        ("wss", 443),
    ] {
        let a = origin(&format!("{scheme}://host.test/"));
        let b = origin(&format!("{scheme}://host.test:{port}/other"));
        assert_eq!(a, b);
        assert!(!a.is_opaque());
        assert_eq!(a.serialize(), format!("{scheme}://host.test"));
        assert_eq!(
            origin(&format!("{scheme}://host.test:1234/")).serialize(),
            format!("{scheme}://host.test:1234")
        );
    }
}

#[test]
fn typed_hosts_preserve_canonical_domain_and_ip_semantics() {
    for (a, b, expected) in [
        (
            "https://BÜCHER.example/",
            "https://xn--bcher-kva.example/",
            "https://xn--bcher-kva.example",
        ),
        ("http://127.1/", "http://127.0.0.1/", "http://127.0.0.1"),
        (
            "http://0x7f000001/",
            "http://127.0.0.1/",
            "http://127.0.0.1",
        ),
        (
            "https://[0:0:0:0:0:0:0:1]:443/",
            "https://[::1]/",
            "https://[::1]",
        ),
        (
            "https://[2001:db8::1]:8443/",
            "https://[2001:0db8:0:0:0:0:0:1]:8443/",
            "https://[2001:db8::1]:8443",
        ),
    ] {
        let retained = origin(a);
        assert_eq!(retained, origin(b));
        assert_eq!(retained.serialize(), expected);
        assert_eq!(retained.clone().serialize(), expected);
    }
}

#[test]
fn opaque_identity_is_fresh_but_cloning_preserves_it() {
    for input in [
        "file:///tmp/a",
        "file://server/share/a",
        "about:blank",
        "about:srcdoc",
        "data:text/plain,x",
        "javascript:1",
        "custom://host/path",
        "custom:payload",
    ] {
        let url = BrowserUrl::parse(input).unwrap().url;
        let a = url.url_origin();
        let shared = a.clone();
        let b = url.url_origin();
        drop(url);
        assert!(a.is_opaque());
        assert_eq!(a, a);
        assert_eq!(a, shared);
        assert!(shared.is_same_origin(&a));
        assert_ne!(a, b);
        assert!(!b.is_same_origin(&a));
        assert_eq!(a.serialize(), "null");
        assert_eq!(a.serialize(), b.serialize());
        assert_ne!(a, origin("https://host/"));
        assert_ne!(origin("https://host/"), a);
    }
    assert_ne!(origin("file:///tmp/a"), origin("file:///tmp/b"));
    assert_ne!(origin("file:///tmp/a"), origin("file:///tmp/sub/b"));
}

#[test]
fn blob_no_entry_fallback_is_limited_to_http_https_and_file() {
    for (input, expected) in [
        (
            "blob:https://user:secret@EXAMPLE.test:443/id",
            "https://example.test",
        ),
        ("blob:http://[::1]:8080/id", "http://[::1]:8080"),
    ] {
        let a = origin(input);
        assert!(!a.is_opaque());
        assert_eq!(a.serialize(), expected);
        assert_eq!(a, origin(expected));
    }
    for input in [
        "blob:file:///tmp/id",
        "blob:",
        "blob:null/id",
        "blob:http://[",
        "blob:blob:https://example.test/id",
        "blob:ftp://host/id",
        "blob:ws://host/id",
        "blob:wss://host/id",
        "blob:about:blank",
        "blob:http%3a//host/id",
    ] {
        let a = origin(input);
        assert!(a.is_opaque(), "{input}");
        assert_eq!(a.serialize(), "null");
        assert_ne!(a, origin(input));
    }
}

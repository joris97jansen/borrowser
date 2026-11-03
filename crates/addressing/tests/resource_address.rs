use addressing::{
    BrowserUrl, ResourceAddress, ResourceCapability as C, ResourceKind as K, ResourceOperation as O,
};
use std::collections::{HashMap, HashSet};

fn address(input: &str, kind: K) -> ResourceAddress {
    ResourceAddress::new(BrowserUrl::parse(input).unwrap().url, kind)
}

#[test]
fn normalized_and_explicit_relative_references_have_equal_keys() {
    let base = BrowserUrl::parse("https://example.test/dir/page")
        .unwrap()
        .url;
    let absolute = address(
        "HTTPS://EXAMPLE.test:443/assets/site.css#one",
        K::Stylesheet,
    );
    for reference in [
        "../assets/site.css#two",
        "/assets/./site.css#",
        "https://example.test/assets/site.css",
    ] {
        let resolved = ResourceAddress::new(base.resolve(reference).unwrap().url, K::Stylesheet);
        assert_ne!(absolute.url(), resolved.url());
        assert_eq!(absolute.retrieval_key(), resolved.retrieval_key());
        let map = HashMap::from([(absolute.retrieval_key(), 42)]);
        assert_eq!(map.get(&resolved.retrieval_key()), Some(&42));
        assert_eq!(
            HashSet::from([absolute.retrieval_key(), resolved.retrieval_key()]).len(),
            1
        );
    }
    assert_eq!(base.as_str(), "https://example.test/dir/page");
}

#[test]
fn every_kind_preserves_complete_fragments_and_shares_retrieval_identity() {
    let key = address("https://h.test/resource?q=1", K::Unknown).retrieval_key();
    for kind in [
        K::Document,
        K::Stylesheet,
        K::Image,
        K::Font,
        K::Media,
        K::FormAction,
        K::Hyperlink,
        K::Unknown,
    ] {
        for (suffix, fragment) in [("", None), ("#", Some("")), ("#part", Some("part"))] {
            let a = address(&format!("https://h.test/resource?q=1{suffix}"), kind);
            assert_eq!(a.kind(), kind);
            assert_eq!(a.url().fragment(), fragment);
            assert_eq!(a.retrieval_key(), key);
        }
    }
}

#[test]
fn queries_credentials_and_component_spelling_remain_identity_inputs() {
    for (a, b) in [
        ("https://h.test/", "https://h.test/?"),
        ("https://h.test/?a=1&b=2", "https://h.test/?b=2&a=1"),
        ("https://h.test/?a=1&a=2", "https://h.test/?a=2&a=1"),
        ("https://h.test/?a=1", "https://h.test/?a=2"),
        ("https://u:p@h.test/", "https://u:q@h.test/"),
        ("https://u:p@h.test/", "https://v:p@h.test/"),
        ("https://h.test/%2f", "https://h.test/%2F"),
        ("https://h.test/%23part", "https://h.test/#part"),
        ("custom:/x", "custom:///x"),
    ] {
        let a = address(a, K::Unknown).retrieval_key();
        let b = address(b, K::Unknown).retrieval_key();
        assert_ne!(a, b);
        assert_eq!(HashSet::from([a, b]).len(), 2);
    }
    let a = address("custom:opaque  ?a=1&a=2#part", K::Unknown);
    assert_eq!(a.retrieval_key().as_str(), "custom:opaque %20?a=1&a=2");
    assert_eq!(a.url().fragment(), Some("part"));
}

#[test]
fn capability_is_separate_from_parsing_and_operation_aware() {
    for (input, retrieval, navigation) in [
        ("https://h.test/", C::Supported, C::Supported),
        ("http://h.test/", C::Supported, C::Supported),
        ("file:///tmp/a", C::Supported, C::Supported),
        ("about:blank", C::Unsupported, C::Reserved),
        ("data:text/plain,x", C::Deferred, C::Deferred),
        ("blob:https://h.test/id", C::Deferred, C::Deferred),
        ("ws://h.test/", C::Reserved, C::Reserved),
        ("wss://h.test/", C::Reserved, C::Reserved),
        ("ftp://h.test/", C::Unsupported, C::Unsupported),
        ("javascript:void(0)", C::Unsupported, C::Unsupported),
        ("custom:payload", C::Unsupported, C::Unsupported),
    ] {
        let a = address(input, K::Unknown);
        assert_eq!(a.capability(O::Retrieval), retrieval);
        assert_eq!(a.capability(O::Navigation), navigation);
        assert_eq!(a.url().as_str(), input);
        assert!(!a.retrieval_key().as_str().is_empty());
    }
    assert!(BrowserUrl::parse("http://[").is_err());
}

#[test]
fn retrieval_identity_does_not_create_or_replace_origin_identity() {
    let url = BrowserUrl::parse("file:///tmp/page#one").unwrap().url;
    let retained = url.url_origin();
    let shared = retained.clone();
    let independent = url.url_origin();
    let a = ResourceAddress::new(url.clone(), K::Document);
    let b = ResourceAddress::new(url, K::Image);
    assert_eq!(a.retrieval_key(), b.retrieval_key());
    assert_eq!(retained, shared);
    assert_ne!(retained, independent);
    assert_eq!(retained.serialize(), independent.serialize());
}

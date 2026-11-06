use super::support::request;
use crate::{
    FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod, fetch::build_http_request,
};
use core_types::ResourceKind;
use std::sync::{Arc, atomic::AtomicBool};

#[test]
fn ai2_pinned_ureq_repeated_fields_have_name_dependent_behavior() {
    let repeated = ureq::get("http://example.invalid/")
        .set("X-Forwarded-For", "one")
        .set("X-Forwarded-For", "two");
    assert_eq!(repeated.all("x-forwarded-for"), ["one", "two"]);
    for name in ["accept", "Accept", "accept-language", "Accept-Language"] {
        let r = ureq::get("http://example.invalid/")
            .set(name, "one")
            .set(name, "two");
        assert_eq!(r.all(name), ["two"]);
    }
    let mixed = ureq::get("http://example.invalid/")
        .set("Accept", "one")
        .set("accept", "two");
    assert_eq!(mixed.all("accept"), ["one", "two"]); // not an adapter workaround
    assert_eq!(
        ureq::get("http://example.invalid/")
            .set("accept", " one\t")
            .all("accept"),
        ["one"]
    );
}

#[test]
fn ai2_adapter_preserves_admitted_fields_and_complete_correlation_url_without_io() {
    let original = request(
        "https://example.invalid/a?x=1&x=2#fragment",
        ResourceKind::Image,
    );
    let mut fields = HttpHeaders::new();
    fields.append("Accept-Language", b"en").unwrap();
    fields.append("Accept", b"image/png").unwrap();
    let r = FetchRequest::try_new(
        1,
        original.address().clone(),
        RequestMethod::Get,
        fields,
        None,
        None,
    )
    .unwrap();
    let built = build_http_request(&ureq::AgentBuilder::new().build(), &r, ResourceKind::Image);
    assert_eq!(built.method(), "GET");
    assert_eq!(built.url(), "https://example.invalid/a?x=1&x=2#fragment");
    assert_eq!(built.header_names(), ["accept-language", "accept"]);
    assert_eq!(built.all("accept-language"), ["en"]);
    assert_eq!(built.all("accept"), ["image/png"]);
    assert_eq!(
        r.transport_target().as_str(),
        "https://example.invalid/a?x=1&x=2"
    );
}

#[test]
fn ai2_backend_defaults_are_separate_from_fetch_fields_without_network_io() {
    struct Intercept;
    impl ureq::Middleware for Intercept {
        fn handle(
            &self,
            r: ureq::Request,
            _: ureq::MiddlewareNext<'_>,
        ) -> Result<ureq::Response, ureq::Error> {
            assert_eq!(r.all("accept"), ["*/*"]);
            assert_eq!(r.all("accept-encoding"), ["gzip"]);
            assert!(!r.has("user-agent")); // generated later by the wire serializer
            ureq::Response::new(200, "OK", "")
        }
    }
    let agent = ureq::AgentBuilder::new()
        .user_agent("Borrowser/test")
        .middleware(Intercept)
        .build();
    assert!(format!("{agent:?}").contains("Borrowser/test"));
    let raw = agent.get("http://example.invalid/");
    assert!(!raw.has("accept"));
    assert!(!raw.has("accept-encoding"));
    let r = request("http://example.invalid/", ResourceKind::Html);
    // Test-only interceptor returns a synthetic response without calling next.
    build_http_request(&agent, &r, ResourceKind::Html)
        .call()
        .unwrap();
}

#[test]
fn ai2_mismatched_route_returns_before_worker_or_callback() {
    let (tx, rx) = std::sync::mpsc::channel();
    let result = crate::fetch_stream(
        request("https://example.invalid/", ResourceKind::Css),
        ResourceKind::Html,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |_| {
            tx.send(()).unwrap();
        }),
    );
    assert_eq!(
        result,
        Err(RequestAdmissionError::IncompatibleExecutionRoute)
    );
    assert_eq!(rx.recv(), Err(std::sync::mpsc::RecvError));
}

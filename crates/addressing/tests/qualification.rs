//! Maintained-backend qualification against independent URL expectations.
//!
//! The 2026-10-26 adoption decision permits known dependency limitations; it
//! does not change the independently authored WPT expectations. Exact observed
//! differences are asserted separately. New AND resolved differences fail the
//! inventory so dependency upgrades require review. No cases are skipped.
//! All credential-bearing inputs here are synthetic test data. Backend Debug
//! output in this test is evidence only, not the proposed safe engine Debug API.

use addressing::{BrowserUrl, UrlHost};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Debug;
use url::{Host, ParseError, SyntaxViolation, Url};

const CORPUS: &[u8] = include_bytes!("fixtures/wpt/urltestdata.json");
const LICENSE: &[u8] = include_bytes!("fixtures/wpt/LICENSE.md");

// WPT accessor spelling only: this observes the production URL without
// reparsing its serialization or borrowing a backend URL. Origin is out of AH3.
fn browser_url_components(url: &BrowserUrl) -> Value {
    let hostname = match url.host() {
        None => String::new(),
        Some(UrlHost::Domain(host) | UrlHost::Opaque(host)) => host.to_owned(),
        Some(UrlHost::Ipv4(host)) => host.to_string(),
        Some(UrlHost::Ipv6(host)) => format!("[{host}]"),
    };
    let host = match url.port() {
        Some(port) => format!("{hostname}:{port}"),
        None => hostname.clone(),
    };
    json!({
        "href": url.as_str(),
        "protocol": format!("{}:", url.scheme()),
        "username": url.username(),
        "password": url.password().unwrap_or_default(),
        "host": host,
        "hostname": hostname,
        "port": url.port().map(|port| port.to_string()).unwrap_or_default(),
        "pathname": url.path(),
        "search": url.query().filter(|value| !value.is_empty())
            .map(|value| format!("?{value}")).unwrap_or_default(),
        "hash": url.fragment().filter(|value| !value.is_empty())
            .map(|value| format!("#{value}")).unwrap_or_default(),
    })
}

#[test]
fn pinned_wpt_relative_discovery() {
    // All inputs below are the pinned, public, synthetic WPT fixtures. This
    // evidence output is not a safe logging API for arbitrary user input.
    let selection: Value =
        serde_json::from_str(include_str!("fixtures/wpt/resolution-selection.json")).unwrap();
    assert_eq!(
        selection["upstream_revision"],
        "c48d58747e1f211527fb695fd60548a997fae617"
    );
    assert_eq!(selection["upstream_path"], "url/resources/urltestdata.json");
    assert_eq!(
        selection["standard_revision"],
        "845bafca22fb0c5a42c4bf384a9bdea525a2f10d"
    );
    assert_eq!(selection["sha256"], format!("{:x}", Sha256::digest(CORPUS)));
    assert_eq!(
        selection["license_sha256"],
        format!("{:x}", Sha256::digest(LICENSE))
    );
    let corpus: Vec<Value> = serde_json::from_slice(CORPUS).unwrap();
    let mut selected_indices = BTreeSet::new();
    let mut selected_ids = BTreeSet::new();
    for case in selection["cases"].as_array().unwrap() {
        let index = case["index"].as_u64().unwrap() as usize;
        assert!(selected_indices.insert(index));
        assert!(selected_ids.insert(case["id"].as_str().unwrap()));
        assert!(!case["rationale"].as_str().unwrap().is_empty());
        assert_eq!(case["input"], corpus[index]["input"]);
        assert_eq!(case["base"], corpus[index]["base"]);
        assert!(case["base"].is_string());
    }
    assert_eq!(selected_indices.len(), 32);
    let objects = corpus.iter().filter(|source| source.is_object()).count();
    let relative_to = corpus
        .iter()
        .filter(|source| source.get("relativeTo").is_some())
        .count();
    assert_eq!(
        objects,
        selection["population"]["corpus_objects"].as_u64().unwrap() as usize
    );
    assert_eq!(
        relative_to,
        selection["population"]["null_base_relative_to"]
            .as_u64()
            .unwrap() as usize
    );
    let mut evaluated = 0;
    let mut expected_failures = 0;
    let mut discrepancies = Vec::new();
    for (index, source) in corpus.iter().enumerate() {
        let Some(base_input) = source.get("base").and_then(Value::as_str) else {
            continue;
        };
        evaluated += 1;
        let input = source["input"].as_str().unwrap();
        let mut differences = serde_json::Map::new();
        let base = BrowserUrl::parse(base_input)
            .expect("WPT base must parse")
            .url;
        let original = base.clone();
        let result = base.resolve(input);
        // Attribution only: the normative oracle below is WPT, not this second
        // call to the dependency. Verify wrapper/backend parity before blaming
        // upstream for a production mismatch, including base parsing effects.
        let backend_base = Url::parse(base_input).unwrap();
        assert_eq!(
            base.as_str(),
            backend_base.as_str(),
            "WPT {index}: base adapter"
        );
        let backend = Url::options().base_url(Some(&backend_base)).parse(input);
        match (&result, &backend) {
            (Ok(outcome), Ok(backend)) => assert_eq!(
                browser_url_components(&outcome.url),
                wpt_components(backend),
                "WPT {index}: adapter"
            ),
            (Err(_), Err(_)) => {}
            _ => panic!("WPT {index}: Borrowser/backend outcome differs"),
        }
        for _ in 0..3 {
            assert_eq!(base.resolve(input), result, "WPT {index}: repeatability");
            assert_eq!(base, original, "WPT {index}: immutable base");
        }
        match (&result, source.get("failure") == Some(&Value::Bool(true))) {
            (Ok(outcome), false) => {
                for (component, actual) in browser_url_components(&outcome.url).as_object().unwrap()
                {
                    let expected = source.get(component).expect("concrete WPT component");
                    if actual != expected {
                        differences.insert(
                            component.clone(),
                            json!({"expected":expected,"actual":actual}),
                        );
                    }
                }
            }
            (Err(_), true) => {
                expected_failures += 1;
            }
            (Ok(outcome), true) => {
                differences.insert(
                    "parse".into(),
                    json!({"expected":"failure","actual":outcome.url.as_str()}),
                );
            }
            (Err(error), false) => {
                differences.insert(
                    "parse".into(),
                    json!({"expected":source["href"],"actual":format!("{:?}",error.kind)}),
                );
            }
        }
        if !differences.is_empty() {
            discrepancies.push(json!({"index":index,"input":input,"base":base_input,"parsed_base":base.as_str(),"differences":differences}));
        }
    }
    let selected_discrepancies = discrepancies
        .iter()
        .filter(|case| selected_indices.contains(&(case["index"].as_u64().unwrap() as usize)))
        .count();
    eprintln!(
        "AH3 FOCUSED: {} selected; {} strict matches; {selected_discrepancies} discrepancies",
        selected_indices.len(),
        selected_indices.len() - selected_discrepancies
    );
    eprintln!(
        "AH3 DISCOVERY: {evaluated} evaluated; {} strict matches; {} discrepancies",
        evaluated - discrepancies.len(),
        discrepancies.len()
    );
    eprintln!(
        "AH3 FORMS: {} concrete successes; {expected_failures} expected failures matched; {relative_to} null-base relativeTo annotations outside explicit-base population",
        evaluated - expected_failures
    );
    eprintln!("{}", serde_json::to_string_pretty(&discrepancies).unwrap());
    assert_eq!(evaluated, 336);
    assert_eq!(expected_failures, 62);
    assert!(
        discrepancies.is_empty(),
        "Review AH3 discovery before accepting any deviations"
    );
}

#[test]
fn pinned_wpt_absolute_discovery() {
    let selection: Value =
        serde_json::from_str(include_str!("fixtures/wpt/selection.json")).unwrap();
    assert_eq!(
        selection["upstream_revision"],
        "c48d58747e1f211527fb695fd60548a997fae617"
    );
    assert_eq!(selection["sha256"], format!("{:x}", Sha256::digest(CORPUS)));
    assert_eq!(
        selection["license_sha256"],
        format!("{:x}", Sha256::digest(LICENSE))
    );
    let corpus: Vec<Value> = serde_json::from_slice(CORPUS).unwrap();
    let mut evaluated = 0;
    for (index, source) in corpus
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_object() && s["base"].is_null())
    {
        evaluated += 1;
        let input = source["input"].as_str().unwrap();
        let result = BrowserUrl::parse(input);
        for _ in 0..3 {
            assert_eq!(
                BrowserUrl::parse(input),
                result,
                "WPT {index}: repeatability"
            );
        }
        if source.get("failure") == Some(&Value::Bool(true)) {
            assert!(result.is_err(), "WPT {index}: expected fatal result");
        } else {
            let outcome = result.unwrap_or_else(|error| panic!("WPT {index}: {error:?}"));
            for (component, actual) in browser_url_components(&outcome.url).as_object().unwrap() {
                assert_eq!(actual, &source[component], "WPT {index}: {component}");
            }
        }
    }
    assert_eq!(evaluated, 560);
    eprintln!("AH3 NULL-BASE: {evaluated} evaluated; {evaluated} strict matches; 0 discrepancies");
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    upstream_revision: String,
    upstream_path: String,
    sha256: String,
    license_sha256: String,
    standard_revision: String,
    cases: Vec<SelectedCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedCase {
    id: String,
    index: usize,
    input: String,
    rationale: String,
}

fn parse(input: &str) -> (Result<Url, ParseError>, Vec<SyntaxViolation>) {
    let diagnostics = RefCell::new(Vec::new());
    let callback = |event| diagnostics.borrow_mut().push(event);
    let result = Url::options()
        .syntax_violation_callback(Some(&callback))
        .parse(input);
    (result, diagnostics.into_inner())
}

fn compare<T: PartialEq + Debug>(
    failures: &mut Vec<String>,
    id: &str,
    input: &str,
    invariant: &str,
    actual: T,
    expected: T,
) {
    if actual != expected {
        failures.push(format!(
            "{id}: input={input:?}; {invariant}; expected={expected:?}; actual={actual:?}"
        ));
    }
}

fn finish(label: &str, cases: usize, failures: Vec<String>) {
    for failure in &failures {
        eprintln!("FAIL {failure}");
    }
    eprintln!(
        "{label}: {cases} cases; {} assertion failures",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{label}: mandatory qualification failed"
    );
}

// This projects observed backend components into WPT's URL-API spelling. It
// does not parse or normalize expected values, compute origins, or use a base.
fn wpt_components(url: &Url) -> Value {
    let hostname = url.host_str().unwrap_or_default();
    let host = match url.port() {
        Some(port) => format!("{hostname}:{port}"),
        None => hostname.to_owned(),
    };
    let search = match url.query() {
        Some(query) if !query.is_empty() => format!("?{query}"),
        _ => String::new(),
    };
    let hash = match url.fragment() {
        Some(fragment) if !fragment.is_empty() => format!("#{fragment}"),
        _ => String::new(),
    };
    json!({
        "href": url.as_str(),
        "protocol": format!("{}:", url.scheme()),
        "username": url.username(),
        "password": url.password().unwrap_or_default(),
        "host": host,
        "hostname": hostname,
        "port": url.port().map(|port| port.to_string()).unwrap_or_default(),
        "pathname": url.path(),
        "search": search,
        "hash": hash,
    })
}

// WPT's search/hash/hostname strings collapse absent/empty distinctions. Keep
// those distinctions, path form, and backend IP types in stability checks too.
fn retained_components(url: &Url) -> Value {
    let host_kind = match url.host() {
        None => "none",
        Some(Host::Domain(_)) => "domain-or-opaque",
        Some(Host::Ipv4(_)) => "ipv4",
        Some(Host::Ipv6(_)) => "ipv6",
    };
    json!({
        "url_api": wpt_components(url),
        "authority": url.has_authority(),
        "host_kind": host_kind,
        "host": url.host_str(),
        "password": url.password(),
        "port": url.port(),
        "query": url.query(),
        "fragment": url.fragment(),
        "opaque_path": url.cannot_be_a_base(),
    })
}

fn check_stability(
    failures: &mut Vec<String>,
    id: &str,
    input: &str,
    result: &Result<Url, ParseError>,
    diagnostics: &[SyntaxViolation],
) {
    for _ in 0..3 {
        let (repeated, repeated_diagnostics) = parse(input);
        compare(
            failures,
            id,
            input,
            "repeated parse result",
            &repeated,
            result,
        );
        compare(
            failures,
            id,
            input,
            "repeated callback sequence",
            repeated_diagnostics.as_slice(),
            diagnostics,
        );
    }
    let Ok(url) = result else { return };
    let (reparsed, _) = parse(url.as_str());
    match reparsed {
        Ok(reparsed) => {
            compare(
                failures,
                id,
                input,
                "parse-serialize-parse serialization",
                reparsed.as_str(),
                url.as_str(),
            );
            compare(
                failures,
                id,
                input,
                "parse-serialize-parse components",
                retained_components(&reparsed),
                retained_components(url),
            );
            compare(failures, id, input, "round-trip equality", &reparsed, url);
        }
        Err(error) => failures.push(format!(
            "{id}: input={input:?}; normalized URL must reparse; error={error:?}"
        )),
    }
}

#[test]
fn pinned_wpt_absolute_semantics_and_published_dependency_deviations() {
    let selection: Selection =
        serde_json::from_str(include_str!("fixtures/wpt/selection.json")).unwrap();
    assert_eq!(
        selection.upstream_revision,
        "c48d58747e1f211527fb695fd60548a997fae617"
    );
    assert_eq!(selection.upstream_path, "url/resources/urltestdata.json");
    assert_eq!(
        selection.standard_revision,
        "845bafca22fb0c5a42c4bf384a9bdea525a2f10d"
    );
    assert_eq!(format!("{:x}", Sha256::digest(CORPUS)), selection.sha256);
    assert_eq!(
        format!("{:x}", Sha256::digest(LICENSE)),
        selection.license_sha256
    );
    let corpus: Vec<Value> = serde_json::from_slice(CORPUS).unwrap();
    assert!(!selection.cases.is_empty());
    let mut ids = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut failures = Vec::new();
    let mut failed_cases = 0;
    let mut deviations = serde_json::Map::new();
    for case in &selection.cases {
        assert!(ids.insert(&case.id), "duplicate case ID");
        assert!(indices.insert(case.index), "duplicate source index");
        assert!(!case.rationale.is_empty());
        let source = &corpus[case.index];
        assert_eq!(source.get("base"), Some(&Value::Null), "base must be null");
        assert_eq!(source["input"].as_str(), Some(case.input.as_str()));
        let mut differences = serde_json::Map::new();
        let (result, diagnostics) = parse(&case.input);
        let expected_failure = source.get("failure") == Some(&Value::Bool(true));
        match (&result, expected_failure) {
            (Ok(url), false) => {
                for (component, actual) in wpt_components(url).as_object().unwrap() {
                    let expected = source.get(component).expect("required WPT component");
                    if actual != expected {
                        differences.insert(
                            component.clone(),
                            json!({"expected": expected, "actual": actual}),
                        );
                    }
                }
            }
            (Err(_), true) => {}
            (Ok(url), true) => {
                differences.insert(
                    "parse".into(),
                    json!({"expected": "failure", "actual": url.as_str()}),
                );
            }
            (Err(error), false) => {
                differences.insert(
                    "parse".into(),
                    json!({"expected": source["href"], "actual": format!("{error:?}")}),
                );
            }
        }
        check_stability(&mut failures, &case.id, &case.input, &result, &diagnostics);
        if !differences.is_empty() {
            failed_cases += 1;
            deviations.insert(case.id.clone(), Value::Object(differences));
        }
    }
    eprintln!(
        "WPT standards results: {} matched; {failed_cases} deviated (not conformance passes)",
        selection.cases.len() - failed_cases
    );
    let recorded: Value =
        serde_json::from_str(include_str!("fixtures/published-url-deviations.json")).unwrap();
    assert_eq!(
        Value::Object(deviations),
        recorded,
        "Published dependency behavior changed; review independent expectations and limitation inventory"
    );
    finish(
        "pinned WPT determinism and stability",
        selection.cases.len(),
        failures,
    );
}

#[test]
fn native_absolute_normalization_and_stability() {
    // Independently authored expectations from the pinned URL Standard;
    // never generated by serializing the candidate.
    let cases = [
        (
            "http-case-port",
            "HTTP://EXAMPLE.test:80",
            "http://example.test/",
        ),
        (
            "https-path-query",
            "HTTPS://Example.test:443/a/../b?x=1&x=2#f",
            "https://example.test/b?x=1&x=2#f",
        ),
        (
            "idna",
            "https://bücher.test/",
            "https://xn--bcher-kva.test/",
        ),
        ("ipv4", "http://0x7f.1/", "http://127.0.0.1/"),
        (
            "ipv6",
            "https://[2001:0DB8:0:0:0:0:0:1]/",
            "https://[2001:db8::1]/",
        ),
        (
            "port-zero",
            "https://example.test:0/",
            "https://example.test:0/",
        ),
        (
            "port-max",
            "https://example.test:65535/",
            "https://example.test:65535/",
        ),
        (
            "port-empty",
            "https://example.test:/",
            "https://example.test/",
        ),
        (
            "encoded-dot",
            "https://example.test/a/%2e%2e/b",
            "https://example.test/b",
        ),
        (
            "encoded-slash",
            "https://example.test/a%2Fb",
            "https://example.test/a%2Fb",
        ),
        (
            "path-space",
            "https://example.test/a b",
            "https://example.test/a%20b",
        ),
        (
            "path-caret",
            "http://example.test/a^b",
            "http://example.test/a%5Eb",
        ),
        (
            "query-order",
            "https://example.test/?b=2&a=1&a=3",
            "https://example.test/?b=2&a=1&a=3",
        ),
        (
            "empty-query-fragment",
            "https://example.test/?#",
            "https://example.test/?#",
        ),
        ("file-local", "file:///tmp/a%20b", "file:///tmp/a%20b"),
        ("file-localhost", "file://localhost/tmp/a", "file:///tmp/a"),
        ("file-drive", "file:///C:/a/../b", "file:///C:/b"),
        (
            "file-remote",
            "file://server/share/a",
            "file://server/share/a",
        ),
        (
            "opaque-host",
            "custom://EXAMPLE.test/x",
            "custom://EXAMPLE.test/x",
        ),
        ("hostless-path", "custom:/x", "custom:/x"),
        ("empty-host-path", "custom:///x", "custom:///x"),
        ("mailto", "mailto:a@b.test", "mailto:a@b.test"),
        ("data", "data:text/plain,hello", "data:text/plain,hello"),
        ("about", "about:blank", "about:blank"),
        ("javascript", "javascript:1+1", "javascript:1+1"),
        (
            "blob",
            "blob:https://example.test/id",
            "blob:https://example.test/id",
        ),
    ];
    let mut failures = Vec::new();
    let mut failed_cases = 0;
    for (id, input, expected) in cases {
        let before = failures.len();
        let (result, diagnostics) = parse(input);
        match &result {
            Ok(url) => compare(
                &mut failures,
                id,
                input,
                "normalized serialization",
                url.as_str(),
                expected,
            ),
            Err(error) => failures.push(format!("{id}: expected={expected:?}; actual={error:?}")),
        }
        check_stability(&mut failures, id, input, &result, &diagnostics);
        if failures.len() != before {
            failed_cases += 1;
        }
    }
    eprintln!(
        "Native normalization: {} standards matches; {failed_cases} unexpected failures",
        cases.len() - failed_cases
    );
    finish("native normalization", cases.len(), failures);
}

#[test]
fn credential_components_follow_normalized_results() {
    let cases = [
        ("https://h.test/", "https://h.test/", "", None),
        ("https://u@h.test/", "https://u@h.test/", "u", None),
        ("https://u:@h.test/", "https://u@h.test/", "u", None),
        ("https://:p@h.test/", "https://:p@h.test/", "", Some("p")),
        ("https://@h.test/", "https://h.test/", "", None),
        ("https://:@h.test/", "https://h.test/", "", None),
        ("https://u:p@h.test/", "https://u:p@h.test/", "u", Some("p")),
        (
            "https://u%40x:p%3Ass@h.test/",
            "https://u%40x:p%3Ass@h.test/",
            "u%40x",
            Some("p%3Ass"),
        ),
        ("https://u@@h.test/", "https://u%40@h.test/", "u%40", None),
    ];
    let mut failures = Vec::new();
    for (input, href, username, password) in cases {
        let (result, diagnostics) = parse(input);
        let url = result.as_ref().expect("credential fixture must parse");
        compare(
            &mut failures,
            "credentials",
            input,
            "href",
            url.as_str(),
            href,
        );
        compare(
            &mut failures,
            "credentials",
            input,
            "username",
            url.username(),
            username,
        );
        compare(
            &mut failures,
            "credentials",
            input,
            "password",
            url.password(),
            password,
        );
        check_stability(&mut failures, "credentials", input, &result, &diagnostics);
    }
    finish("credential components", cases.len(), failures);
}

#[test]
fn optional_components_and_host_shapes_are_inspectable() {
    let mut failures = Vec::new();
    let optional = [
        ("https://h.test/", None, None),
        ("https://h.test/?", Some(""), None),
        ("https://h.test/#", None, Some("")),
        ("https://h.test/?#", Some(""), Some("")),
        ("https://h.test/?a=1&a=2#f", Some("a=1&a=2"), Some("f")),
    ];
    for (input, query, fragment) in optional {
        let url = Url::parse(input).unwrap();
        compare(
            &mut failures,
            "optional",
            input,
            "query",
            url.query(),
            query,
        );
        compare(
            &mut failures,
            "optional",
            input,
            "fragment",
            url.fragment(),
            fragment,
        );
    }
    let shapes = [
        (
            "https://example.test/",
            Some("example.test"),
            true,
            false,
            "domain-or-opaque",
        ),
        ("https://127.0.0.1/", Some("127.0.0.1"), true, false, "ipv4"),
        ("https://[::1]/", Some("[::1]"), true, false, "ipv6"),
        (
            "custom://EXAMPLE.test/x",
            Some("EXAMPLE.test"),
            true,
            false,
            "domain-or-opaque",
        ),
        ("custom:///x", None, true, false, "none"),
        ("custom:/x", None, false, false, "none"),
        ("file:///tmp/a", None, true, false, "none"),
        ("mailto:a@b.test", None, false, true, "none"),
    ];
    for (input, host, authority, opaque, kind) in shapes {
        let url = Url::parse(input).unwrap();
        compare(
            &mut failures,
            "host-shape",
            input,
            "host",
            url.host_str(),
            host,
        );
        compare(
            &mut failures,
            "host-shape",
            input,
            "authority",
            url.has_authority(),
            authority,
        );
        compare(
            &mut failures,
            "host-shape",
            input,
            "opaque path",
            url.cannot_be_a_base(),
            opaque,
        );
        compare(
            &mut failures,
            "host-shape",
            input,
            "backend host kind",
            &retained_components(&url)["host_kind"],
            &json!(kind),
        );
    }
    finish(
        "optional components and host shapes",
        optional.len() + shapes.len(),
        failures,
    );
}

#[test]
fn normalized_url_equality_is_complete() {
    let cases = [
        ("HTTP://H.test:80", "http://h.test/", true),
        ("https://h.test:443/a/../b", "https://h.test/b", true),
        ("https://u:@h.test/", "https://u@h.test/", true),
        ("https://:@h.test/", "https://h.test/", true),
        ("https://u:p@h.test/", "https://u:q@h.test/", false),
        ("https://u:p@h.test/", "https://v:p@h.test/", false),
        ("https://h.test/?a=1&b=2", "https://h.test/?b=2&a=1", false),
        ("https://h.test/#one", "https://h.test/#two", false),
        ("https://h.test/", "https://h.test/?", false),
        ("https://h.test/", "https://h.test/#", false),
        ("https://h.test/%2f", "https://h.test/%2F", false),
        ("custom:/x", "custom:///x", false),
    ];
    let mut failures = Vec::new();
    for (left, right, equal) in cases {
        let a = Url::parse(left).unwrap();
        let b = Url::parse(right).unwrap();
        compare(&mut failures, "equality", left, right, a == b, equal);
    }
    finish("equality", cases.len(), failures);
}

#[test]
fn fatal_categories_and_prior_callbacks_are_observable() {
    let cases = [
        ("", ParseError::RelativeUrlWithoutBase),
        ("../x", ParseError::RelativeUrlWithoutBase),
        ("//example.test/x", ParseError::RelativeUrlWithoutBase),
        ("https://:443/", ParseError::EmptyHost),
        ("https://example.test:65536/", ParseError::InvalidPort),
        ("https://example.test:abc/", ParseError::InvalidPort),
        ("https://256.0.0.1/", ParseError::InvalidIpv4Address),
        ("https://[::1/", ParseError::InvalidIpv6Address),
        ("https://exa mple.test/", ParseError::IdnaError),
        ("custom://a b/", ParseError::InvalidDomainCharacter),
    ];
    let mut failures = Vec::new();
    for (input, expected) in cases {
        let (result, diagnostics) = parse(input);
        compare(
            &mut failures,
            "fatal",
            input,
            "fatal category",
            result.as_ref().err().copied(),
            Some(expected),
        );
        check_stability(&mut failures, "fatal", input, &result, &diagnostics);
    }
    let input = " https://u:p@h.test:70000/";
    let (result, diagnostics) = parse(input);
    compare(
        &mut failures,
        "fatal-after-recovery",
        input,
        "fatal category",
        result.err(),
        Some(ParseError::InvalidPort),
    );
    compare(
        &mut failures,
        "fatal-after-recovery",
        input,
        "prior events",
        diagnostics,
        vec![
            SyntaxViolation::C0SpaceIgnored,
            SyntaxViolation::EmbeddedCredentials,
        ],
    );
    finish("fatal categories", cases.len() + 1, failures);
}

#[test]
fn recoverable_callbacks_preserve_observed_sequence_and_duplicates() {
    use SyntaxViolation::*;
    let cases: &[(&str, &str, &[SyntaxViolation])] = &[
        (
            " http:////u:p@exa\tmple.test/a\\b\\c%zz%yy#x\0y",
            "http://u:p@example.test/a/b/c%zz%yy#x%00y",
            &[
                C0SpaceIgnored,
                TabOrNewlineIgnored,
                ExpectedDoubleSlash,
                EmbeddedCredentials,
                Backslash,
                Backslash,
                PercentDecode,
                PercentDecode,
                NullInFragment,
            ],
        ),
        (
            "https://u@@h.test/",
            "https://u%40@h.test/",
            &[EmbeddedCredentials, UnencodedAtSign],
        ),
        (
            "https://u:@h.test/",
            "https://u@h.test/",
            &[EmbeddedCredentials],
        ),
        (
            "https://:@h.test/",
            "https://h.test/",
            &[EmbeddedCredentials],
        ),
        ("file:/tmp/a", "file:///tmp/a", &[ExpectedFileDoubleSlash]),
        ("https://h.test/<", "https://h.test/%3C", &[NonUrlCodePoint]),
    ];
    let mut failures = Vec::new();
    for &(input, href, expected_events) in cases {
        let (result, diagnostics) = parse(input);
        let url = result.as_ref().expect("recoverable fixture must parse");
        compare(
            &mut failures,
            "recoverable",
            input,
            "href",
            url.as_str(),
            href,
        );
        compare(
            &mut failures,
            "recoverable",
            input,
            "callback sequence",
            diagnostics.as_slice(),
            expected_events,
        );
        check_stability(&mut failures, "recoverable", input, &result, &diagnostics);
    }
    finish("recoverable callbacks", cases.len(), failures);
}

#[test]
fn former_file_round_trip_counterexample_preserves_path_and_host() {
    // Preserve the independent fixed-point invariant and original reproduction
    // from rust-url #1101. An empty first segment must remain before t:.
    let input = "file://.cRe!+aacRddddddddddddddtpe=//t:/a|et/!..";
    let first = Url::parse(input).unwrap();
    assert_eq!(
        first.as_str(),
        "file://.cre!+aacrddddddddddddddtpe=//t:/a|et/!.."
    );
    assert_eq!(first.host_str(), Some(".cre!+aacrddddddddddddddtpe="));
    for _ in 0..3 {
        let second = Url::parse(first.as_str()).unwrap();
        assert_eq!(first, second);
    }
}

#[test]
fn diagnostic_coverage_limitations_are_visible() {
    // Coverage observations, NOT normative expectations of diagnostic silence.
    // Nondecimal IPv4 and malformed opaque-host escapes have missing events.
    // The valid domain escape is a control, not a missing validation error.
    // No fake events or positions are synthesized for the omissions.
    for input in ["http://0x7f.1/", "https://exam%70le.test/", "custom://%/"] {
        let (result, diagnostics) = parse(input);
        eprintln!("DIAGNOSTIC COVERAGE input={input:?}; events={diagnostics:?}");
        assert!(result.is_ok());
    }
}

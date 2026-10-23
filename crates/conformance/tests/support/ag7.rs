//! Qualification of the named initial corpus, never production registration.
#![allow(
    dead_code,
    reason = "Shared by independently compiled qualification test targets"
)]
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const RGB: [&str; 2] = ["wpt/rgb-alias-p6", "wpt/rgb-comments-p6"];
pub const LOCAL: &str = "wpt-style/background-color-mismatch";
pub const EXCLUDED: [&str; 2] = [
    "wpt/unsupported/first-child-js",
    "wpt/unsupported/testdriver",
];

fn present(rgba: [u8; 4]) -> Value {
    json!({"state":"observed","value":{"state":"present","value":{"kind":"rgba8","value":rgba}}})
}
fn case<'a>(report: &'a Value, id: &str) -> &'a Value {
    let matches: Vec<_> = report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["test_id"] == id)
        .collect();
    assert_eq!(matches.len(), 1, "exact case {id}");
    matches[0]
}

pub fn assert_exact_difference_set(report: &Value) {
    let mut actual = BTreeSet::new();
    for c in report["cases"].as_array().unwrap() {
        let id = c["test_id"].as_str().unwrap();
        for (role, doc) in [("test", c), ("reference", &c["reference_document"])] {
            if doc.is_null() {
                continue;
            }
            for field in doc["fields"].as_array().unwrap() {
                if field["status"] == "different" {
                    assert!(actual.insert((
                        id.to_owned(),
                        role.to_owned(),
                        field["id"].as_str().unwrap().to_owned()
                    )));
                }
            }
        }
    }
    let expected: BTreeSet<_> = RGB
        .into_iter()
        .flat_map(|id| {
            ["test", "reference"].map(|role| {
                (
                    id.to_owned(),
                    role.to_owned(),
                    "style:#p6/background-color".to_owned(),
                )
            })
        })
        .collect();
    assert_eq!(
        actual, expected,
        "only the four confirmed initial RGB deficiencies are accepted"
    );
    assert_eq!(report["summary"]["different_fields"], 4);
}

pub fn assert_initial_cases(report: &Value) {
    assert_eq!(report["schema"], "borrowser.cross-engine-phases.v2");
    for id in RGB.into_iter().chain([LOCAL]) {
        let c = case(report, id);
        let rgb = id != LOCAL;
        let target = if rgb { "p6" } else { "probe" };
        assert_eq!(
            c["status"],
            if rgb {
                "different"
            } else {
                "equal_for_requested_scope"
            }
        );
        if rgb {
            assert_eq!(c["borrowser_expectation"]["status"], "XFAIL");
            assert_eq!(c["borrowser_expectation"]["metadata"], "known_failure");
            assert_eq!(
                c["borrowser_expectation"]["expected"]["styles"][0]["value"],
                present([10, 150, 50, 255])
            );
            assert_eq!(c["upstream_assertion"]["status"], "unsupported");
            assert_eq!(c["upstream_assertion"]["form"], "visual_reftest");
            assert_eq!(
                c["upstream_assertion"]["coverage"],
                "scoped computed style only"
            );
            assert_eq!(
                c["provenance"]["revision"],
                "2c705104a295c48053eeddf7fe0170d790a4e853"
            );
        } else {
            assert!(c["borrowser_expectation"].is_null());
            assert!(c["upstream_assertion"].is_null());
        }
        for (role, doc) in [("test", c), ("reference", &c["reference_document"])] {
            assert_eq!(doc["descriptor"]["case_id"], id);
            assert_eq!(doc["descriptor"]["role"], role);
            assert_eq!(doc["fields"].as_array().unwrap().len(), 1);
            let field = &doc["fields"][0];
            assert_eq!(field["id"], format!("style:#{target}/background-color"));
            assert_eq!(field["status"], if rgb { "different" } else { "equal" });
            let b = if rgb {
                [0, 0, 0, 0]
            } else if role == "test" {
                [0, 128, 0, 255]
            } else {
                [255, 0, 0, 255]
            };
            let ch = if rgb { [10, 150, 50, 255] } else { b };
            assert_eq!(field["borrowser"], present(b));
            assert_eq!(field["chromium"], present(ch));
            for (engine, value) in [("borrowser", b), ("chromium", ch)] {
                assert_eq!(
                    doc["projections"][engine]["styles"],
                    json!([{"target":target,"property":"background-color","value":present(value)}])
                );
                assert_eq!(c["relationships"][engine][role], present(value));
                assert_eq!(
                    c["relationships"][engine]["kind"],
                    if rgb { "match" } else { "mismatch" }
                );
                assert_eq!(c["relationships"][engine]["status"], "satisfied");
                assert_eq!(
                    c["relationships"][engine]["scope"],
                    format!("style:#{target}/background-color")
                );
                assert_eq!(
                    c["relationships"][engine]["coverage"],
                    "present-target computed background-color only; no full visual verdict"
                );
            }
            let raw = &doc["captures"];
            assert_eq!(raw["borrowser"]["method"], "accepted-production-frame");
            assert_eq!(raw["borrowser"]["layout_viewport_width"], 640.0);
            assert_eq!(raw["borrowser"]["layout_artifact_source_width"], 640.0);
            assert!(raw["borrowser"]["canvas_color_rgb8"].is_null());
            assert!(raw["chromium"]["canvas_color_rgb8"].is_null());
            let native = &raw["chromium"]["inspection"]["styles"];
            assert_eq!(native.as_array().unwrap().len(), 1);
            assert_eq!(native[0]["target"], target);
            assert_eq!(
                native[0]["values"][1],
                format!("rgb({}, {}, {})", ch[0], ch[1], ch[2])
            );
            assert_eq!(
                raw["chromium"]["browser"]["capture_profile"],
                "ag7-static-references-v1"
            );
            assert_eq!(
                raw["chromium"]["browser"],
                c["captures"]["chromium"]["browser"]
            );
            let links = raw["chromium"]["metadata_links"].as_array().unwrap();
            if role == "test" {
                assert!(links.iter().any(|link| {
                    link["attributes"].as_array().unwrap().iter().any(|a| {
                        a["local_name"] == "rel"
                            && a["value"] == if rgb { "match" } else { "mismatch" }
                    })
                }));
            }
            if role == "reference" {
                assert!(doc.get("borrowser_expectation").is_none());
            }
        }
    }
    for id in EXCLUDED {
        let c = case(report, id);
        assert_eq!(c["status"], "incomplete");
        assert_eq!(c["coverage"]["classification"], "unsupported");
        assert_eq!(c["upstream_assertion"]["status"], "unsupported");
        for field in [
            "captures",
            "projections",
            "descriptor",
            "relationships",
            "reference_document",
            "borrowser_expectation",
        ] {
            assert!(c[field].is_null(), "{id}: {field}");
        }
        assert_eq!(c["fields"], json!([]));
    }
    assert_exact_difference_set(report);
}

// Fixed counts apply ONLY to these named initial cases, never to discovery.
pub fn assert_initial_summary(report: &Value) {
    let selected: Vec<_> = BASELINE
        .into_iter()
        .chain(RGB)
        .chain([LOCAL])
        .chain(EXCLUDED)
        .map(|id| case(report, id))
        .collect();
    assert_eq!(selected.len(), 11);
    assert_eq!(
        summary(&selected),
        json!({"equal_fields":36,"different_fields":4,"unsupported_fields":0,"skipped_cases":0,"expectation_failures":0,"expectation_unexpected_passes":0,"incomplete_cases":2,"executed_documents":12,"upstream_unsupported_cases":4,"runnable_upstream_incomplete_cases":2,"independent_expected_failures":2,"relationship_failures":0,"borrowser_relationships_satisfied":3,"chromium_relationships_satisfied":3})
    );
}

pub const BASELINE: [&str; 6] = [
    "canvas/root",
    "canvas/cascade",
    "dom/structure",
    "dom/template-namespaces",
    "style/cascade-inheritance",
    "layout/block-boxes",
];

// The production projection types are crate-private. Decode only their f64
// leaves here: font-size CssPx and BorderBox coordinates. This is deliberately
// not a recursive JSON-number conversion: colors, identities, states, variants,
// array order and every other field retain exact JSON equality.
fn decode_projection_numbers(mut projection: Value) -> Value {
    fn css_pixels(value: &mut Value) {
        let pixels: f64 = serde_json::from_value(value.clone()).expect("AG5 CSS-pixel number");
        *value = json!(pixels);
    }
    for record in projection["styles"].as_array_mut().expect("style records") {
        if record["property"] == "font-size"
            && record["value"]["state"] == "observed"
            && record["value"]["value"]["state"] == "present"
            && record["value"]["value"]["value"]["kind"] == "css_px"
        {
            css_pixels(&mut record["value"]["value"]["value"]["value"]);
        }
    }
    if projection["layout"]["state"] == "observed" {
        for record in projection["layout"]["value"]
            .as_array_mut()
            .expect("layout records")
        {
            if record["value"]["state"] == "present" {
                for rect in record["value"]["value"]
                    .as_array_mut()
                    .expect("border boxes")
                {
                    for edge in ["left", "top", "right", "bottom"] {
                        css_pixels(&mut rect[edge]);
                    }
                }
            }
        }
    }
    projection
}

// Read authored semantic expectations before execution (also before the isolated
// source checkout is removed). Never bless observed values or register new IDs.
pub fn declarations(root: &Path) -> BTreeMap<String, Value> {
    fn walk(root: &Path, out: &mut BTreeMap<String, Value>) {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.to_string_lossy().ends_with(".fixture.json") {
                let mut m: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                let expected = match m["expected"]["kind"].as_str() {
                    Some("projection") => decode_projection_numbers(
                        serde_json::from_slice(
                            &std::fs::read(
                                path.parent()
                                    .unwrap()
                                    .join(m["expected"]["path"].as_str().unwrap()),
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                    ),
                    Some("canvas") => json!({"dom":{"state":"not_requested"},"styles":[],
                        "layout":{"state":"not_requested"},"canvas":{"state":"observed","value":m["expected"]["rgb8"]}}),
                    None => Value::Null,
                    other => panic!("unsupported test expectation {other:?}"),
                };
                m["authored_expected"] = expected;
                assert!(
                    out.insert(m["id"].as_str().unwrap().to_owned(), m)
                        .is_none()
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, &mut out);
    out
}

// This computes aggregate accounting only, not a semantic acceptance oracle.
// Semantics are checked against authored expectations, declared relationships,
// and the exact four independently confirmed deficiencies above.
fn summary(cases: &[&Value]) -> Value {
    let mut s = json!({"equal_fields":0,"different_fields":0,"unsupported_fields":0,"skipped_cases":0,
        "expectation_failures":0,"expectation_unexpected_passes":0,"incomplete_cases":0,"executed_documents":0,
        "upstream_unsupported_cases":0,"runnable_upstream_incomplete_cases":0,"independent_expected_failures":0,
        "relationship_failures":0,"borrowser_relationships_satisfied":0,"chromium_relationships_satisfied":0});
    let mut add = |key: &str, count: u64| {
        s[key] = json!(s[key].as_u64().unwrap() + count);
    };
    for c in cases {
        for doc in [*c, &c["reference_document"]] {
            if doc.is_null() {
                continue;
            }
            add(
                "executed_documents",
                u64::from(!doc["descriptor"].is_null()),
            );
            for f in doc["fields"].as_array().unwrap() {
                add(
                    match f["status"].as_str().unwrap() {
                        "equal" => "equal_fields",
                        "different" => "different_fields",
                        "unsupported" => "unsupported_fields",
                        other => panic!("unrecognized field status {other}"),
                    },
                    1,
                );
            }
        }
        add("incomplete_cases", u64::from(c["status"] == "incomplete"));
        add("skipped_cases", u64::from(!c["skipped"].is_null()));
        add(
            "upstream_unsupported_cases",
            u64::from(!c["upstream_assertion"].is_null()),
        );
        add(
            "runnable_upstream_incomplete_cases",
            u64::from(!c["upstream_assertion"].is_null() && !c["descriptor"].is_null()),
        );
        match c["borrowser_expectation"]["status"].as_str() {
            Some("FAIL") => add("expectation_failures", 1),
            Some("XPASS") => add("expectation_unexpected_passes", 1),
            Some("XFAIL") => add("independent_expected_failures", 1),
            _ => {}
        }
        if !c["relationships"].is_null() {
            for engine in ["borrowser", "chromium"] {
                add(
                    if c["relationships"][engine]["status"] == "satisfied" {
                        if engine == "borrowser" {
                            "borrowser_relationships_satisfied"
                        } else {
                            "chromium_relationships_satisfied"
                        }
                    } else {
                        "relationship_failures"
                    },
                    1,
                );
            }
        }
    }
    s
}

pub fn assert_catalogue(
    report: &Value,
    exit: i32,
    descriptors: &Value,
    authored: &BTreeMap<String, Value>,
) {
    assert_initial_cases(report);
    assert_initial_summary(report);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(
        cases
            .iter()
            .map(|c| c["test_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        authored.keys().map(String::as_str).collect::<Vec<_>>(),
        "exact discovered case order"
    );
    let actual: Vec<_> = cases
        .iter()
        .flat_map(|c| [c, &c["reference_document"]])
        .filter(|d| !d["descriptor"].is_null())
        .map(|d| d["descriptor"].clone())
        .collect();
    assert_eq!(
        json!(actual),
        *descriptors,
        "exact ordered capture correspondence"
    );
    for c in cases {
        let id = c["test_id"].as_str().unwrap();
        let m = &authored[id];
        let classification = m["classification"]["kind"].as_str().unwrap();
        if !m["upstream_assertion"].is_null() {
            assert_eq!(c["upstream_assertion"]["status"], "unsupported");
            assert_eq!(
                c["upstream_assertion"]["form"],
                m["upstream_assertion"]["form"]
            );
            assert_eq!(
                c["upstream_assertion"]["coverage"],
                if classification == "runnable" {
                    "scoped computed style only"
                } else {
                    "excluded"
                }
            );
        } else {
            assert!(c["upstream_assertion"].is_null());
        }

        if classification != "runnable" {
            assert_eq!(c["status"], "incomplete");
            assert_eq!(c["coverage"]["classification"], classification);
            for key in [
                "descriptor",
                "captures",
                "projections",
                "reference_document",
                "relationships",
                "borrowser_expectation",
            ] {
                assert!(c[key].is_null(), "excluded {id}: {key}");
            }
            continue;
        }
        assert!(!c["descriptor"].is_null());
        let state = |selection: &Value| match selection["kind"].as_str() {
            None | Some("not_requested") => "not_requested",
            Some("compare") => "compared",
            Some("unsupported") => "unsupported",
            other => panic!("invalid authored selection {other:?}"),
        };
        let styles = m["observations"]["styles"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let compared = styles
            .iter()
            .filter(|s| state(&s["selection"]) == "compared")
            .count();
        let unsupported = styles
            .iter()
            .filter(|s| state(&s["selection"]) == "unsupported")
            .count();
        assert_eq!(
            c["coverage"],
            json!([
                {"family":"dom","state":state(&m["observations"]["dom"])},
                {"family":"style","requested_fields":compared+unsupported,"compared_fields":compared,
                 "unsupported_fields":unsupported,"not_requested_fields":styles.len()-compared-unsupported,
                 "state":if compared+unsupported == 0 { "not_requested" } else if compared == 0 { "unsupported" }
                    else if unsupported == 0 { "compared" } else { "partial" }},
                {"family":"layout","state":state(&m["observations"]["layout"])},
                {"family":"canvas","state":state(&m["observations"]["canvas"])},
            ])
        );
        let known_rgb = RGB.contains(&id);
        let has_unsupported = ["dom", "layout", "canvas"]
            .iter()
            .any(|key| m["observations"][key]["kind"] == "unsupported")
            || m["observations"]["styles"]
                .as_array()
                .is_some_and(|styles| {
                    styles
                        .iter()
                        .any(|s| s["selection"]["kind"] == "unsupported")
                });
        assert_eq!(
            c["status"],
            if known_rgb {
                "different"
            } else if has_unsupported || !m["upstream_assertion"].is_null() {
                "incomplete"
            } else {
                "equal_for_requested_scope"
            }
        );
        if !m["authored_expected"].is_null() {
            assert_eq!(
                c["borrowser_expectation"]["expected"],
                m["authored_expected"]
            );
            assert_eq!(
                c["borrowser_expectation"]["status"],
                if known_rgb {
                    "XFAIL"
                } else if has_unsupported {
                    "INCOMPLETE"
                } else {
                    "PASS"
                }
            );
            if !known_rgb {
                assert_eq!(
                    c["projections"]["borrowser"], m["authored_expected"],
                    "{id}: authored expectation"
                );
            }
        } else {
            assert!(c["borrowser_expectation"].is_null());
        }
        for (role, doc) in [("test", c), ("reference", &c["reference_document"])] {
            if doc.is_null() {
                continue;
            }
            assert_eq!(doc["descriptor"]["case_id"], id);
            assert_eq!(doc["descriptor"]["role"], role);
            assert!(!doc["captures"].is_null());
            if !known_rgb {
                assert_eq!(
                    doc["projections"]["borrowser"], doc["projections"]["chromium"],
                    "{id}/{role}: same-document agreement"
                );
                for field in doc["fields"].as_array().unwrap() {
                    assert!(field["status"] == "equal" || field["status"] == "unsupported");
                    if field["status"] == "equal" {
                        assert_eq!(field["borrowser"], field["chromium"]);
                    }
                }
            }
        }
        if !m["relationship"].is_null() {
            for engine in ["borrowser", "chromium"] {
                let r = &c["relationships"][engine];
                assert_eq!(r["kind"], m["relationship"]["kind"]);
                assert_eq!(r["status"], "satisfied");
                let mut values = vec![];
                for (role, doc) in [("test", c), ("reference", &c["reference_document"])] {
                    let styles = doc["projections"][engine]["styles"].as_array().unwrap();
                    assert_eq!(styles.len(), 1);
                    assert_eq!(
                        styles[0]["target"],
                        m["observations"]["styles"][0]["target"]
                    );
                    assert_eq!(styles[0]["property"], "background-color");
                    let v = &styles[0]["value"];
                    assert_eq!(v["state"], "observed");
                    assert_eq!(v["value"]["state"], "present");
                    assert_eq!(v["value"]["value"]["kind"], "rgba8");
                    let rgba = v["value"]["value"]["value"].as_array().unwrap();
                    assert_eq!(rgba.len(), 4);
                    assert!(rgba.iter().all(|n| n.as_u64().is_some_and(|n| n <= 255)));
                    assert!(rgba[3] == 255 || *rgba == vec![json!(0); 4]);
                    assert_eq!(r[role], *v);
                    values.push(v);
                }
                assert_eq!(values[0] == values[1], m["relationship"]["kind"] == "match");
            }
            assert!(
                c["reference_document"]
                    .get("borrowser_expectation")
                    .is_none()
            );
        } else {
            assert!(c["relationships"].is_null());
            assert!(c["reference_document"].is_null());
        }
        assert_eq!(
            c["upstream_assertion"].is_null(),
            m["upstream_assertion"].is_null()
        );
    }
    assert_eq!(
        report["summary"],
        summary(&cases.iter().collect::<Vec<_>>())
    );
    // Exact known RGB differences require failure, regardless of additional passes.
    assert_eq!(exit, 1);
    for key in [
        "expectation_failures",
        "expectation_unexpected_passes",
        "relationship_failures",
    ] {
        assert_eq!(report["summary"][key], 0);
    }
}

//! AG4's bounded wire boundary. Internal fixture identities remain registry-owned.
use super::{BrowserIdentity, CaptureInput, CapturedFixture, ChromiumCapture};
use crate::{
    environment::{HEIGHT, SAMPLE, URL, WIDTH},
    model::CanvasColor,
};
use serde::Deserialize;
use std::io::Read;

pub(crate) const MAX_CAPTURE_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBrowser {
    product: String,
    version: String,
    revision: String,
    protocol_version: String,
    platform: String,
    capture_profile: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    test_id: String,
    source: String,
    #[serde(deserialize_with = "super::inspection::required_nullable")]
    canvas_color_rgb8: Option<CanvasColor>,
    #[serde(deserialize_with = "super::inspection::required_nullable")]
    inspection: Option<super::inspection::Inspection>,
    browser: WireBrowser,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: String,
    document_url: String,
    viewport: [u32; 2],
    device_scale_factor: u32,
    sample_pixel: [u32; 2],
    observations: Vec<Observation>,
}

pub(crate) fn decode(
    reader: impl Read,
    inputs: &[CaptureInput<'_>],
) -> Result<Vec<CapturedFixture>, String> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_CAPTURE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("capture read: {e}"))?;
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err("capture exceeds 64 KiB".into());
    }
    let report: Report =
        serde_json::from_slice(&bytes).map_err(|e| format!("capture JSON: {e}"))?;
    if report.schema != "borrowser.chromium-phases.v1"
        || report.document_url != URL
        || report.viewport != [WIDTH, HEIGHT]
        || report.device_scale_factor != 1
        || report.sample_pixel != SAMPLE
        || inputs.is_empty()
        || inputs.len() > crate::fixtures::FIXTURES.len()
        || report.observations.len() != inputs.len()
    {
        return Err("capture environment or observation count mismatch".into());
    }
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("../../chromium-reference.json"))
            .map_err(|e| e.to_string())?;
    report
        .observations
        .into_iter()
        .zip(inputs)
        .enumerate()
        .map(|(index, (wire, input))| {
            if inputs[..index].iter().any(|other| other.id == input.id)
                || !crate::fixtures::FIXTURES
                    .iter()
                    .any(|f| f.id == input.id && f.source == input.source)
                || wire.test_id != input.id.0
                || wire.source != input.source
            {
                return Err("capture identity/order mismatch".into());
            }
            let b = wire.browser;
            if b.version != pin["version"]
                || b.product != format!("Chrome/{}", b.version)
                || b.revision != pin["revision"]
                || b.protocol_version != pin["protocol_version"]
                || b.capture_profile != pin["capture_profile"]
                || b.platform != "linux-x86_64"
            {
                return Err("capture browser identity mismatch".into());
            }
            let captured = CapturedFixture {
                id: input.id,
                source: wire.source,
                capture: ChromiumCapture {
                    color: wire.canvas_color_rgb8,
                    inspection: wire.inspection,
                    identity: BrowserIdentity {
                        product: b.product,
                        version: b.version,
                        revision: b.revision,
                        protocol_version: b.protocol_version,
                        platform: b.platform,
                        capture_profile: b.capture_profile,
                    },
                    #[cfg(test)]
                    default_favicon_aborted: false,
                },
            };
            crate::projection::chromium(&captured.capture, input.request)
                .map_err(|e| format!("capture observations: {}: {}", e.code, e.detail))?;
            Ok(captured)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn report() -> Value {
        let pin: Value =
            serde_json::from_str(include_str!("../../chromium-reference.json")).unwrap();
        json!({"schema":"borrowser.chromium-phases.v1", "document_url":URL,
            "viewport":[WIDTH,HEIGHT],"device_scale_factor":1,"sample_pixel":SAMPLE,
            "observations":[{"test_id":"canvas/root","source":"fixtures/root-canvas-color.html",
            "canvas_color_rgb8":[18,52,86],"inspection":null,"browser":{"product":format!("Chrome/{}",pin["version"].as_str().unwrap()),
            "version":pin["version"],"revision":pin["revision"],"protocol_version":pin["protocol_version"],
            "platform":"linux-x86_64","capture_profile":pin["capture_profile"]}}]})
    }
    fn check(v: &Value) -> Result<Vec<CapturedFixture>, String> {
        let input = CaptureInput {
            request: crate::projection::ObservationRequest::CANVAS,
            id: crate::fixtures::FIXTURES[0].id,
            source: crate::fixtures::FIXTURES[0].source,
            html: b"",
        };
        decode(serde_json::to_vec(v).unwrap().as_slice(), &[input])
    }
    #[test]
    fn validated_capture_preserves_registry_identity_and_sample() {
        let c = check(&report()).unwrap();
        assert_eq!(c[0].id, crate::fixtures::FIXTURES[0].id);
        assert_eq!(c[0].capture.color, Some(CanvasColor([18, 52, 86])));
    }
    #[test]
    fn rejects_incompatible_incomplete_duplicate_unrelated_and_misordered_data() {
        for pointer in [
            "/schema",
            "/document_url",
            "/observations/0/test_id",
            "/observations/0/source",
            "/observations/0/browser/platform",
            "/observations/0/browser/version",
            "/observations/0/browser/revision",
            "/observations/0/browser/protocol_version",
            "/observations/0/browser/capture_profile",
        ] {
            let mut v = report();
            *v.pointer_mut(pointer).unwrap() = json!("wrong");
            assert!(check(&v).is_err(), "{pointer}");
        }
        let mut v = report();
        v["observations"].as_array_mut().unwrap().clear();
        assert!(check(&v).is_err());
        let mut v = report();
        let o = v["observations"][0].clone();
        v["observations"].as_array_mut().unwrap().push(o);
        assert!(check(&v).is_err());
        let mut v = report();
        v["observations"][0]["canvas_color_rgb8"] = json!([256, 0, 0]);
        assert!(check(&v).is_err());
        let input = CaptureInput {
            request: crate::projection::ObservationRequest::CANVAS,
            id: crate::fixtures::FIXTURES[0].id,
            source: "fixtures/root-canvas-color.html",
            html: b"",
        };
        let wire = serde_json::to_string(&report()).unwrap().replacen(
            "{",
            "{\"schema\":\"duplicate\",",
            1,
        );
        assert!(decode(wire.as_bytes(), &[input]).is_err());
    }
    #[test]
    fn caps_reader_before_unrestricted_buffering() {
        let mut reader = std::io::repeat(b' ');
        assert!(decode(&mut reader, &[]).unwrap_err().contains("64 KiB"));
    }
    #[test]
    fn complete_registry_requires_exact_order_and_unique_observations() {
        let inputs: Vec<_> = crate::fixtures::FIXTURES[..2]
            .iter()
            .map(|f| CaptureInput {
                request: f.request,
                id: f.id,
                source: f.source,
                html: b"",
            })
            .collect();
        let mut wire = report();
        let mut second = wire["observations"][0].clone();
        second["test_id"] = json!(inputs[1].id.0);
        second["source"] = json!(inputs[1].source);
        wire["observations"].as_array_mut().unwrap().push(second);
        let check = |wire: &Value| decode(serde_json::to_vec(wire).unwrap().as_slice(), &inputs);
        assert_eq!(check(&wire).unwrap().len(), 2);
        wire["observations"].as_array_mut().unwrap().swap(0, 1);
        assert!(check(&wire).is_err());
        wire["observations"][1] = wire["observations"][0].clone();
        assert!(check(&wire).is_err());
        assert!(decode(b"{\"observations\":".as_slice(), &inputs).is_err());
    }
}

#[cfg(test)]
mod phase_tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn all_six_registry_requests_survive_encoding_and_strict_ag4_ingestion() {
        let pin: Value =
            serde_json::from_str(include_str!("../../chromium-reference.json")).unwrap();
        let inputs: Vec<_> = crate::fixtures::FIXTURES
            .iter()
            .map(|f| CaptureInput {
                id: f.id,
                source: f.source,
                html: b"",
                request: f.request,
            })
            .collect();
        let observations:Vec<_>=inputs.iter().map(|input|{
            let request=crate::projection::inspection_request(input.request);
            let inspect=if request.dom || !request.style_targets.is_empty() || !request.layout_targets.is_empty(){
                json!({"dom":if request.dom{json!({"node_type":9,"name":"","namespace":"","value":"","public_id":"","system_id":"","attributes":[],"children":[],"template":null})}else{Value::Null},
                    "styles":request.style_targets.iter().map(|id|json!({"target":id,"values":null})).collect::<Vec<_>>(),
                    "layout":request.layout_targets.iter().map(|id|json!({"target":id,"boxes":null})).collect::<Vec<_>>(),"inner_width":640,"inner_height":480,"layout_width":640})
            }else{Value::Null};
            json!({"test_id":input.id.0,"source":input.source,"canvas_color_rgb8":if matches!(input.request.canvas,crate::projection::Request::Compare(())){json!([18,52,86])}else{Value::Null},"inspection":inspect,
                "browser":{"product":format!("Chrome/{}",pin["version"].as_str().unwrap()),"version":pin["version"],"revision":pin["revision"],"protocol_version":pin["protocol_version"],"capture_profile":pin["capture_profile"],"platform":"linux-x86_64"}})
        }).collect();
        let wire = json!({"schema":"borrowser.chromium-phases.v1","document_url":URL,"viewport":[WIDTH,HEIGHT],"device_scale_factor":1,"sample_pixel":SAMPLE,"observations":observations});
        let check = |wire: &Value| decode(serde_json::to_vec(wire).unwrap().as_slice(), &inputs);
        assert_eq!(check(&wire).unwrap().len(), 6);
        // Both local capture and AG4 ingestion use this strict native boundary.
        // Usable width may exclude scrollbars; configured inner dimensions cannot.
        let mut narrower = wire.clone();
        narrower["observations"][3]["inspection"]["layout_width"] = json!(625);
        assert_eq!(
            check(&narrower).unwrap()[3]
                .capture
                .inspection
                .as_ref()
                .unwrap()
                .layout_width,
            625.0
        );
        for (field, value) in [
            ("inner_width", json!(625)),
            ("inner_height", json!(465)),
            ("layout_width", json!(641)),
            ("layout_width", json!(0)),
            ("layout_width", json!(625.5)),
        ] {
            let mut bad = narrower.clone();
            bad["observations"][3]["inspection"][field] = value;
            assert!(check(&bad).is_err(), "{field}");
        }
        for field in ["inner_width", "inner_height", "layout_width"] {
            let mut bad = narrower.clone();
            bad["observations"][3]["inspection"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(check(&bad).is_err(), "{field}");
        }
        for pointer in [
            "/observations/2/inspection/dom",
            "/observations/4/inspection/styles",
            "/observations/5/inspection/layout",
        ] {
            let mut bad = wire.clone();
            *bad.pointer_mut(pointer).unwrap() = Value::Null;
            assert!(check(&bad).is_err(), "{pointer}");
        }
        let mut bad = wire.clone();
        bad["observations"][5]["inspection"]["layout"][0]
            .as_object_mut()
            .unwrap()
            .remove("boxes");
        assert!(check(&bad).is_err());
        let mut bad = wire.clone();
        bad["schema"] = json!("borrowser.chromium-canvas.v1");
        assert!(check(&bad).is_err());
        let mut bad = wire.clone();
        bad["observations"][2]["canvas_color_rgb8"] = json!([1, 2, 3]);
        assert!(check(&bad).is_err());
        let mut bad = wire.clone();
        bad["observations"].as_array_mut().unwrap().swap(2, 3);
        assert!(check(&bad).is_err());
    }
}

#[cfg(test)]
mod mixed_presence_tests {
    use super::*;
    use crate::projection::{ObservationRequest, Request, StyleField, StyleProperty};
    use serde_json::{Value, json};

    #[test]
    fn mixed_dom_presence_is_consistent_at_local_and_ag4_boundaries() {
        let request = ObservationRequest {
            dom: Request::Compare(()),
            styles: &[StyleField {
                target: "x",
                property: Request::Compare(StyleProperty::Color),
                name: "color",
            }],
            layout: Request::Compare(&["x"]),
            ..ObservationRequest::NONE
        };
        let input = CaptureInput {
            id: crate::fixtures::FIXTURES[2].id,
            source: crate::fixtures::FIXTURES[2].source,
            html: b"",
            request,
        };
        let element = json!({"node_type":1,"name":"div","namespace":"http://www.w3.org/1999/xhtml","value":"","public_id":"","system_id":"","attributes":[{"namespace":"","prefix":"","name":"id","value":"x"}],"children":[],"template":null});
        let document = json!({"node_type":9,"name":"","namespace":"","value":"","public_id":"","system_id":"","attributes":[],"children":[element.clone()],"template":null});
        let base = json!({"dom":document,"styles":[{"target":"x","values":["rgb(18, 52, 86)","rgba(0, 0, 0, 0)","16px","block"]}],"layout":[{"target":"x","boxes":[]}],"inner_width":640,"inner_height":480,"layout_width":640});
        let mut variants = vec![("active present, zero boxes", base.clone(), true)];
        let mut missing = base.clone();
        missing["styles"][0]["values"] = Value::Null;
        missing["layout"][0]["boxes"] = Value::Null;
        variants.push(("active element reported missing", missing.clone(), false));
        missing["dom"]["children"] = json!([]);
        variants.push(("absent target reported missing", missing, true));
        let mut absent = base.clone();
        absent["dom"]["children"] = json!([]);
        variants.push(("absent target reported present", absent, false));
        let mut duplicate = base.clone();
        duplicate["dom"]["children"] = json!([element.clone(), element.clone()]);
        variants.push(("duplicate active targets", duplicate, false));
        let mut template = element.clone();
        template["name"] = json!("template");
        template["attributes"] = json!([]);
        let mut fragment = document.clone();
        fragment["node_type"] = json!(11);
        template["template"] = fragment;
        let mut inert = base.clone();
        inert["dom"]["children"] = json!([template]);
        variants.push(("inert target reported present", inert.clone(), false));
        inert["styles"][0]["values"] = Value::Null;
        inert["layout"][0]["boxes"] = Value::Null;
        variants.push(("inert target excluded", inert.clone(), true));
        inert["dom"]["children"]
            .as_array_mut()
            .unwrap()
            .push(element);
        inert["styles"] = base["styles"].clone();
        inert["layout"] = base["layout"].clone();
        variants.push(("inert duplicate does not shadow active target", inert, true));
        let mut foreign = base.clone();
        foreign["dom"]["children"][0]["namespace"] = json!("http://www.w3.org/2000/svg");
        variants.push(("non HTML target", foreign, false));
        let mut failures = vec![];
        for (label, inspection, accepted) in variants {
            let mut capture = super::super::tests::controlled_capture(CanvasColor([0, 0, 0]));
            capture.color = None;
            capture.identity.platform = "linux-x86_64".into();
            capture.inspection = Some(serde_json::from_value(inspection.clone()).unwrap());
            let local = crate::projection::chromium(&capture, request).is_ok();
            let wire = json!({"schema":"borrowser.chromium-phases.v1","document_url":URL,"viewport":[WIDTH,HEIGHT],"device_scale_factor":1,"sample_pixel":SAMPLE,"observations":[{"test_id":input.id.0,"source":input.source,"canvas_color_rgb8":null,"inspection":inspection,"browser":capture.identity}]});
            let remote = decode(
                serde_json::to_vec(&wire).unwrap().as_slice(),
                std::slice::from_ref(&input),
            )
            .is_ok();
            if local != accepted || remote != accepted {
                failures.push(format!(
                    "{label}: local={local}, AG4={remote}, expected={accepted}"
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

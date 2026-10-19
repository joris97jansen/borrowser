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
    canvas_color_rgb8: [u8; 3],
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
    if report.schema != "borrowser.chromium-canvas.v1"
        || report.document_url != URL
        || report.viewport != [WIDTH, HEIGHT]
        || report.device_scale_factor != 1
        || report.sample_pixel != SAMPLE
        || inputs.is_empty()
        || inputs.len() > 2
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
            Ok(CapturedFixture {
                id: input.id,
                source: wire.source,
                capture: ChromiumCapture {
                    color: CanvasColor(wire.canvas_color_rgb8),
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
            })
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
        json!({"schema":"borrowser.chromium-canvas.v1", "document_url":URL,
            "viewport":[WIDTH,HEIGHT],"device_scale_factor":1,"sample_pixel":SAMPLE,
            "observations":[{"test_id":"canvas/root","source":"fixtures/root-canvas-color.html",
            "canvas_color_rgb8":[18,52,86],"browser":{"product":format!("Chrome/{}",pin["version"].as_str().unwrap()),
            "version":pin["version"],"revision":pin["revision"],"protocol_version":pin["protocol_version"],
            "platform":"linux-x86_64","capture_profile":pin["capture_profile"]}}]})
    }
    fn check(v: &Value) -> Result<Vec<CapturedFixture>, String> {
        let input = CaptureInput {
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
        assert_eq!(c[0].capture.color, CanvasColor([18, 52, 86]));
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
        let inputs: Vec<_> = crate::fixtures::FIXTURES
            .iter()
            .map(|f| CaptureInput {
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

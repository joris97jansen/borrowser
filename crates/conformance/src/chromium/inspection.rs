//! Engine-specific native Chromium observations, separate from projections.
use super::{
    cdp::Cdp,
    error::{Error, Phase},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct InspectionRequest {
    pub(crate) dom: bool,
    pub(crate) style_targets: Vec<String>,
    pub(crate) layout_targets: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inspection {
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) dom: Option<NativeNode>,
    pub(crate) styles: Vec<NativeStyle>,
    pub(crate) layout: Vec<NativeLayout>,
    pub(crate) layout_width: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeNode {
    pub(crate) node_type: u8,
    pub(crate) name: String,
    pub(crate) namespace: String,
    pub(crate) value: String,
    pub(crate) public_id: String,
    pub(crate) system_id: String,
    pub(crate) attributes: Vec<NativeAttribute>,
    pub(crate) children: Vec<NativeNode>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) template: Option<Box<NativeNode>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeAttribute {
    pub(crate) namespace: String,
    pub(crate) prefix: String,
    pub(crate) name: String,
    pub(crate) value: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeStyle {
    pub(crate) target: String,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) values: Option<[String; 4]>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeLayout {
    pub(crate) target: String,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) boxes: Option<Vec<NativeRect>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeRect {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
}

pub(super) fn collect(
    cdp: &mut Cdp<'_>,
    frame_id: &str,
    request: &InspectionRequest,
    deadline: Instant,
) -> Result<Inspection, Error> {
    let world = cdp.call(
        "Page.createIsolatedWorld",
        json!({
            "frameId": frame_id, "worldName": "borrowser-ag5-read-only",
            "grantUniveralAccess": false,
        }),
        deadline,
        Phase::Capture,
    )?;
    let context = world["executionContextId"]
        .as_i64()
        .ok_or_else(|| Error::Capture("missing isolated inspection context".into()))?;
    let expression = format!(
        "{}({})",
        include_str!("inspect.js"),
        serde_json::to_string(request).map_err(|e| Error::Capture(e.to_string()))?
    );
    let response = cdp.call(
        "Runtime.evaluate",
        json!({
            "expression": expression, "contextId": context, "returnByValue": true,
            "timeout": deadline.saturating_duration_since(Instant::now()).as_millis() as u64,
        }),
        deadline,
        Phase::Capture,
    )?;
    if response.get("exceptionDetails").is_some() {
        return Err(Error::Capture(format!(
            "native inspection failed: {}",
            response["exceptionDetails"]
        )));
    }
    let inspection: Inspection = serde_json::from_value(response["result"]["value"].clone())
        .map_err(|e| Error::Capture(format!("incomplete native inspection: {e}")))?;
    validate(&inspection, request).map_err(Error::Capture)?;
    Ok(inspection)
}

pub(crate) fn validate(data: &Inspection, request: &InspectionRequest) -> Result<(), String> {
    let fail = || "incomplete, inconsistent, or oversized native inspection".to_string();
    let mut targets = std::collections::BTreeSet::new();
    targets.extend(request.style_targets.iter());
    targets.extend(request.layout_targets.iter());
    if targets.len() > 8
        || data.dom.is_some() != request.dom
        || data.styles.len() != request.style_targets.len()
        || data.layout.len() != request.layout_targets.len()
        || data.layout_width != f64::from(crate::environment::WIDTH)
    {
        return Err(fail());
    }
    let mut bytes = 0;
    fn tree(
        n: &NativeNode,
        depth: usize,
        count: &mut usize,
        bytes: &mut usize,
    ) -> Result<(), String> {
        *count += 1;
        *bytes += n.name.len()
            + n.namespace.len()
            + n.value.len()
            + n.public_id.len()
            + n.system_id.len();
        if *count > 128 || depth > 32 || n.attributes.len() > 32 {
            return Err("native tree limit".into());
        }
        for a in &n.attributes {
            *bytes += a.name.len() + a.namespace.len() + a.prefix.len() + a.value.len();
        }
        for c in &n.children {
            tree(c, depth + 1, count, bytes)?;
        }
        if let Some(t) = &n.template {
            tree(t, depth + 1, count, bytes)?;
        }
        Ok(())
    }
    if let Some(dom) = &data.dom {
        tree(dom, 0, &mut 0, &mut bytes)?;
    }
    for (s, id) in data.styles.iter().zip(&request.style_targets) {
        if &s.target != id {
            return Err(fail());
        }
        bytes += s.target.len();
        if let Some(values) = &s.values {
            bytes += values.iter().map(String::len).sum::<usize>();
        }
    }
    for (l, id) in data.layout.iter().zip(&request.layout_targets) {
        if &l.target != id {
            return Err(fail());
        }
        bytes += l.target.len();
        if let Some(boxes) = &l.boxes {
            if boxes.len() > 128 {
                return Err(fail());
            }
            for b in boxes {
                if ![b.left, b.top, b.right, b.bottom]
                    .iter()
                    .all(|v| v.is_finite())
                    || b.right < b.left
                    || b.bottom < b.top
                {
                    return Err(fail());
                }
            }
        }
        if let Some(s) = data.styles.iter().find(|s| s.target == l.target)
            && s.values.is_none() != l.boxes.is_none()
        {
            return Err(fail());
        }
    }
    if bytes > 8192 {
        return Err(fail());
    }
    if let Some(dom) = &data.dom {
        // Query the returned active tree only. Template fragments are inert and
        // have their own edge, so never enter them for rendered target lookup.
        fn matches<'a>(node: &'a NativeNode, id: &str, out: &mut Vec<&'a NativeNode>) {
            if node.node_type == 1
                && node.attributes.iter().any(|a| {
                    a.namespace.is_empty() && a.prefix.is_empty() && a.name == "id" && a.value == id
                })
            {
                out.push(node);
            }
            for child in &node.children {
                matches(child, id, out);
            }
        }
        for target in targets {
            let mut found = vec![];
            matches(dom, target, &mut found);
            if found.len() > 1
                || found
                    .first()
                    .is_some_and(|n| n.namespace != "http://www.w3.org/1999/xhtml")
            {
                return Err(format!(
                    "#{target}: native DOM target is not a unique HTML element"
                ));
            }
            let present = !found.is_empty();
            if data
                .styles
                .iter()
                .any(|s| &s.target == target && s.values.is_some() != present)
                || data
                    .layout
                    .iter()
                    .any(|l| &l.target == target && l.boxes.is_some() != present)
            {
                return Err(format!(
                    "#{target}: native target presence contradicts active DOM"
                ));
            }
        }
    }
    Ok(())
}
pub(crate) fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspection_missing_record_is_error_but_complete_missing_node_is_data() {
        let request = InspectionRequest {
            dom: false,
            style_targets: vec!["x".into()],
            layout_targets: vec!["x".into()],
        };
        let wire = json!({"dom":null,"styles":[{"target":"x","values":null}],"layout":[{"target":"x","boxes":null}],"layout_width":640});
        let data: Inspection = serde_json::from_value(wire.clone()).unwrap();
        validate(&data, &request).unwrap();
        for pointer in ["styles", "layout"] {
            let mut changed = wire.clone();
            changed[pointer] = json!([]);
            assert!(validate(&serde_json::from_value(changed).unwrap(), &request).is_err());
        }
        for (family, field) in [("styles", "values"), ("layout", "boxes")] {
            let mut changed = wire.clone();
            changed[family][0].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<Inspection>(changed).is_err());
        }
        let mut data = data.clone();
        data.layout[0].boxes = Some(vec![]);
        assert!(
            validate(&data, &request).is_err(),
            "same target cannot be both missing and present"
        );
    }
    #[test]
    fn inspection_rejects_oversized_inconsistent_and_unknown_data() {
        let request = InspectionRequest {
            dom: false,
            style_targets: vec!["x".into()],
            layout_targets: vec![],
        };
        let mut data:Inspection=serde_json::from_value(json!({"dom":null,"styles":[{"target":"x","values":["rgb(0, 0, 0)","rgba(0, 0, 0, 0)","16px","block"]}],"layout":[],"layout_width":640})).unwrap();
        validate(&data, &request).unwrap();
        data.styles[0].values.as_mut().unwrap()[0] = "x".repeat(8193);
        assert!(validate(&data, &request).is_err());
        data.styles[0].values = None;
        data.styles[0].target = "wrong".into();
        assert!(validate(&data, &request).is_err());
        let mut wire = serde_json::to_value(&data).unwrap();
        wire["unknown"] = json!(true);
        assert!(serde_json::from_value::<Inspection>(wire).is_err());
    }
    #[test]
    fn inspection_rejects_tree_depth_node_and_attribute_overflow_without_truncation() {
        let mut node:NativeNode=serde_json::from_value(json!({"node_type":9,"name":"","namespace":"","value":"","public_id":"","system_id":"","attributes":[],"children":[],"template":null})).unwrap();
        let request = InspectionRequest {
            dom: true,
            ..Default::default()
        };
        let check = |node| {
            validate(
                &Inspection {
                    dom: Some(node),
                    styles: vec![],
                    layout: vec![],
                    layout_width: 640.0,
                },
                &request,
            )
        };
        assert!(check(node.clone()).is_ok());
        node.children = vec![node.clone(); 128];
        assert!(check(node.clone()).is_err());
        node.children.clear();
        node.attributes = vec![
            NativeAttribute {
                namespace: String::new(),
                prefix: String::new(),
                name: "data-x".into(),
                value: String::new()
            };
            33
        ];
        assert!(check(node.clone()).is_err());
        node.attributes.clear();
        for _ in 0..33 {
            node.children = vec![node.clone()];
        }
        assert!(check(node).is_err());
    }
}

use super::{HarnessError, error};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BorderBox {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
}
fn rect(values: [f64; 4]) -> Result<BorderBox, HarnessError> {
    if !values.iter().all(|v| v.is_finite()) {
        return Err(error("nonfinite geometry"));
    }
    let [left, top, right, bottom] = values.map(|v| if v == 0.0 { 0.0 } else { v });
    if right < left || bottom < top {
        return Err(error("negative border-box extent"));
    }
    Ok(BorderBox {
        left,
        top,
        right,
        bottom,
    })
}
pub(crate) fn borrowser(
    artifact: &layout::RetainedLayoutArtifact,
    id: html::internal::Id,
) -> Result<Vec<BorderBox>, HarnessError> {
    fn walk(
        node: layout::RetainedBoxView<'_>,
        id: html::internal::Id,
        boxes: &mut Vec<BorderBox>,
    ) -> Result<(), HarnessError> {
        if node.direct_node_id() == Some(id) {
            let r = node.rect();
            // Widen exact production values before adding edges. Do not use
            // Rectangle::PartialEq, round coordinates, or add presentation origin.
            boxes.push(rect([
                f64::from(r.x),
                f64::from(r.y),
                f64::from(r.x) + f64::from(r.width),
                f64::from(r.y) + f64::from(r.height),
            ])?);
        }
        for child in node.children() {
            walk(child, id, boxes)?;
        }
        Ok(())
    }
    let mut boxes = vec![];
    walk(artifact.root(), id, &mut boxes)?;
    Ok(boxes)
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn chromium(
    r: &crate::chromium::inspection::NativeRect,
) -> Result<BorderBox, HarnessError> {
    rect([r.left, r.top, r.right, r.bottom])
}

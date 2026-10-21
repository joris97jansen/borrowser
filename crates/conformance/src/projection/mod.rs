//! Tooling-owned selection and common values. Never computes engine semantics.
mod dom;
mod layout;
mod style;
#[cfg(test)]
mod tests;
use crate::model::{CanvasColor, HarnessError};
pub(crate) use dom::DomNode;
pub(crate) use layout::BorderBox;
use serde::{Deserialize, Serialize};
pub(crate) use style::{StyleProperty, StyleValue};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Request<T> {
    NotRequested,
    Compare(T),
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Explicit unsupported requests are exercised by regression fixtures"
        )
    )]
    Unsupported {
        reason: &'static str,
    },
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct StyleField {
    pub(crate) target: &'static str,
    pub(crate) property: Request<StyleProperty>,
    pub(crate) name: &'static str,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ObservationRequest {
    pub(crate) dom: Request<()>,
    pub(crate) styles: &'static [StyleField],
    pub(crate) layout: Request<&'static [&'static str]>,
    pub(crate) canvas: Request<()>,
}

// Owned wire correspondence for the existing request, not another observation API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum RequestSelection<T> {
    NotRequested,
    Compare(T),
    Unsupported(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestedStyle {
    target: String,
    property: String,
    selection: RequestSelection<()>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestDescriptor {
    dom: RequestSelection<()>,
    styles: Vec<RequestedStyle>,
    layout: RequestSelection<Vec<String>>,
    canvas: RequestSelection<()>,
}
impl ObservationRequest {
    pub(crate) fn descriptor(self) -> RequestDescriptor {
        fn own<T, U>(r: Request<T>, f: impl FnOnce(T) -> U) -> RequestSelection<U> {
            match r {
                Request::NotRequested => RequestSelection::NotRequested,
                Request::Compare(v) => RequestSelection::Compare(f(v)),
                Request::Unsupported { reason } => RequestSelection::Unsupported(reason.into()),
            }
        }
        RequestDescriptor {
            dom: own(self.dom, |v| v),
            canvas: own(self.canvas, |v| v),
            styles: self
                .styles
                .iter()
                .map(|f| RequestedStyle {
                    target: f.target.into(),
                    property: f.name.into(),
                    selection: own(f.property, |_| ()),
                })
                .collect(),
            layout: own(self.layout, |v| v.iter().map(|s| (*s).into()).collect()),
        }
    }
}
impl ObservationRequest {
    #[allow(
        dead_code,
        reason = "Metadata-generated requests may spell out every family"
    )]
    pub(crate) const NONE: Self = Self {
        dom: Request::NotRequested,
        styles: &[],
        layout: Request::NotRequested,
        canvas: Request::NotRequested,
    };
    #[allow(
        dead_code,
        reason = "Metadata-generated requests may spell out every family"
    )]
    pub(crate) const CANVAS: Self = Self {
        canvas: Request::Compare(()),
        ..Self::NONE
    };
    pub(crate) fn needs_frame(self) -> bool {
        !matches!(self.dom, Request::NotRequested)
            || !self.styles.is_empty()
            || !matches!(self.layout, Request::NotRequested)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Observation<T> {
    NotRequested,
    Unsupported(String),
    Observed(T),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum TargetValue<T> {
    MissingNode,
    Present(T),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StyleObservation {
    pub(crate) target: String,
    pub(crate) property: String,
    pub(crate) value: Observation<TargetValue<StyleValue>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutObservation {
    pub(crate) target: String,
    pub(crate) value: TargetValue<Vec<BorderBox>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectedObservations {
    pub(crate) dom: Observation<DomNode>,
    pub(crate) styles: Vec<StyleObservation>,
    pub(crate) layout: Observation<Vec<LayoutObservation>>,
    pub(crate) canvas: Observation<CanvasColor>,
}
impl Default for ProjectedObservations {
    fn default() -> Self {
        Self {
            dom: Observation::NotRequested,
            styles: vec![],
            layout: Observation::NotRequested,
            canvas: Observation::NotRequested,
        }
    }
}
// An owned selection from one borrowed accepted frame. Width is provenance,
// deliberately outside common equality: egui and Chromium constrain differently.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct BorrowserCapture {
    pub(crate) observations: ProjectedObservations,
    pub(crate) layout_viewport_width: Option<f64>,
    pub(crate) layout_artifact_source_width: Option<f64>,
}
fn error(detail: impl Into<String>) -> HarnessError {
    HarnessError::new("observation.invalid", detail)
}
fn select<T>(
    request: Request<()>,
    capture: impl FnOnce() -> Result<T, HarnessError>,
) -> Result<Observation<T>, HarnessError> {
    Ok(match request {
        Request::NotRequested => Observation::NotRequested,
        Request::Unsupported { reason } => Observation::Unsupported(reason.into()),
        Request::Compare(()) => Observation::Observed(capture()?),
    })
}
pub(crate) fn from_frame(
    frame: &browser::tab::CompletedPageFrame<'_>,
    request: ObservationRequest,
) -> Result<BorrowserCapture, HarnessError> {
    // Project only while the Tab borrow is alive. No references escape.
    let dom = select(request.dom, || dom::borrowser(frame.document()))?;
    let styles = request
        .styles
        .iter()
        .map(|field| {
            let value = match field.property {
                Request::NotRequested => Observation::NotRequested,
                Request::Unsupported { reason } => Observation::Unsupported(reason.into()),
                Request::Compare(property) => {
                    Observation::Observed(match dom::target(frame.document(), field.target)? {
                        None => TargetValue::MissingNode,
                        Some(element) => {
                            let style = frame
                                .element_styles()
                                .iter()
                                .find(|s| s.node_id() == element.id())
                                .ok_or_else(|| {
                                    error(format!(
                                        "missing accepted-frame style for #{}",
                                        field.target
                                    ))
                                })?;
                            TargetValue::Present(style::borrowser(style.style(), property)?)
                        }
                    })
                }
            };
            Ok(StyleObservation {
                target: field.target.into(),
                property: field.name.into(),
                value,
            })
        })
        .collect::<Result<_, HarnessError>>()?;
    let layout = match request.layout {
        Request::NotRequested => Observation::NotRequested,
        Request::Unsupported { reason } => Observation::Unsupported(reason.into()),
        Request::Compare(targets) => Observation::Observed(
            targets
                .iter()
                .map(|target| {
                    let value = match dom::target(frame.document(), target)? {
                        None => TargetValue::MissingNode,
                        Some(element) => {
                            TargetValue::Present(layout::borrowser(frame.layout(), element.id())?)
                        }
                    };
                    Ok(LayoutObservation {
                        target: (*target).into(),
                        value,
                    })
                })
                .collect::<Result<_, HarnessError>>()?,
        ),
    };
    let canvas = select(request.canvas, || {
        crate::execute::capture_canvas_color(frame.output())
    })?;
    Ok(BorrowserCapture {
        observations: ProjectedObservations {
            dom,
            styles,
            layout,
            canvas,
        },
        layout_viewport_width: Some(f64::from(frame.layout_viewport_width())),
        layout_artifact_source_width: Some(f64::from(frame.layout().source_viewport_width())),
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn inspection_request(
    request: ObservationRequest,
) -> crate::chromium::inspection::InspectionRequest {
    let mut style_targets = Vec::new();
    for field in request.styles {
        if matches!(field.property, Request::Compare(_))
            && !style_targets.iter().any(|v| v == field.target)
        {
            style_targets.push(field.target.into());
        }
    }
    crate::chromium::inspection::InspectionRequest {
        dom: matches!(request.dom, Request::Compare(())),
        style_targets,
        layout_targets: match request.layout {
            Request::Compare(targets) => targets.iter().map(|s| (*s).into()).collect(),
            _ => vec![],
        },
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn chromium(
    capture: &crate::chromium::ChromiumCapture,
    request: ObservationRequest,
) -> Result<ProjectedObservations, HarnessError> {
    validate_request(request)?;
    let inspect = capture.inspection.as_ref();
    let needed = inspection_request(request);
    if needed.dom || !needed.style_targets.is_empty() || !needed.layout_targets.is_empty() {
        crate::chromium::inspection::validate(
            inspect.ok_or_else(|| error("missing native inspection"))?,
            &needed,
        )
        .map_err(error)?;
    } else if inspect.is_some() {
        return Err(error("unrequested native inspection"));
    }
    let dom = select(request.dom, || {
        dom::chromium(
            inspect
                .and_then(|i| i.dom.as_ref())
                .ok_or_else(|| error("missing DOM capture"))?,
        )
    })?;
    let styles = request
        .styles
        .iter()
        .map(|field| {
            let value = match field.property {
                Request::NotRequested => Observation::NotRequested,
                Request::Unsupported { reason } => Observation::Unsupported(reason.into()),
                Request::Compare(property) => {
                    let record = inspect
                        .and_then(|i| i.styles.iter().find(|s| s.target == field.target))
                        .ok_or_else(|| error("missing style target record"))?;
                    Observation::Observed(match &record.values {
                        None => TargetValue::MissingNode,
                        Some(values) => TargetValue::Present(style::chromium(
                            &values[property.index()],
                            property,
                        )?),
                    })
                }
            };
            Ok(StyleObservation {
                target: field.target.into(),
                property: field.name.into(),
                value,
            })
        })
        .collect::<Result<_, HarnessError>>()?;
    let layout = match request.layout {
        Request::NotRequested => Observation::NotRequested,
        Request::Unsupported { reason } => Observation::Unsupported(reason.into()),
        Request::Compare(targets) => Observation::Observed(
            targets
                .iter()
                .map(|target| {
                    let record = inspect
                        .and_then(|i| i.layout.iter().find(|l| l.target == *target))
                        .ok_or_else(|| error("missing layout target record"))?;
                    Ok(LayoutObservation {
                        target: (*target).into(),
                        value: match &record.boxes {
                            None => TargetValue::MissingNode,
                            Some(boxes) => TargetValue::Present(
                                boxes
                                    .iter()
                                    .map(layout::chromium)
                                    .collect::<Result<_, _>>()?,
                            ),
                        },
                    })
                })
                .collect::<Result<_, HarnessError>>()?,
        ),
    };
    let canvas = select(request.canvas, || {
        capture.color.ok_or_else(|| error("missing canvas capture"))
    })?;
    if !matches!(request.canvas, Request::Compare(())) && capture.color.is_some() {
        return Err(error("unrequested canvas capture"));
    }
    Ok(ProjectedObservations {
        dom,
        styles,
        layout,
        canvas,
    })
}

#[derive(Debug, Serialize)]
pub(crate) struct Difference {
    pub(crate) path: String,
    pub(crate) left: Option<serde_json::Value>,
    pub(crate) right: Option<serde_json::Value>,
}
pub(crate) fn differences(
    path: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> Vec<Difference> {
    fn walk(
        path: &str,
        l: Option<&serde_json::Value>,
        r: Option<&serde_json::Value>,
        out: &mut Vec<Difference>,
    ) {
        use serde_json::Value;
        if l == r {
            return;
        }
        match (l, r) {
            (Some(Value::Object(l)), Some(Value::Object(r))) => {
                let keys: std::collections::BTreeSet<_> = l.keys().chain(r.keys()).collect();
                for key in keys {
                    walk(&format!("{path}/{key}"), l.get(key), r.get(key), out);
                }
            }
            (Some(Value::Array(l)), Some(Value::Array(r))) => {
                for i in 0..l.len().max(r.len()) {
                    walk(&format!("{path}[{i}]"), l.get(i), r.get(i), out);
                }
            }
            _ => out.push(Difference {
                path: path.into(),
                left: l.cloned(),
                right: r.cloned(),
            }),
        }
    }
    let mut out = vec![];
    walk(path, Some(left), Some(right), &mut out);
    out
}
impl ProjectedObservations {
    /// Validate record correspondence and the existing color value domain,
    /// never the cardinality of observed content.
    pub(crate) fn validate(&self, request: ObservationRequest) -> Result<(), HarnessError> {
        validate_request(request)?;
        fn required(detail: impl Into<String>) -> HarnessError {
            HarnessError::new("observation.required_data", detail)
        }
        fn state<T, U>(
            id: &str,
            observation: &Observation<T>,
            request: Request<U>,
        ) -> Result<(), HarnessError> {
            let valid = match (observation, request) {
                (Observation::NotRequested, Request::NotRequested)
                | (Observation::Observed(_), Request::Compare(_)) => true,
                (Observation::Unsupported(actual), Request::Unsupported { reason }) => {
                    actual == reason
                }
                _ => false,
            };
            if valid {
                Ok(())
            } else {
                Err(required(format!("{id}: state does not match request")))
            }
        }
        state("dom", &self.dom, request.dom)?;
        state("canvas", &self.canvas, request.canvas)?;
        state("layout", &self.layout, request.layout)?;
        if self.styles.len() != request.styles.len() {
            return Err(required(format!(
                "style: expected {} records, received {}",
                request.styles.len(),
                self.styles.len()
            )));
        }
        for (record, field) in self.styles.iter().zip(request.styles) {
            let id = format!("style:#{}/{}", field.target, field.name);
            if record.target != field.target || record.property != field.name {
                return Err(required(format!(
                    "{id}: record identity/order does not match request"
                )));
            }
            state(&id, &record.value, field.property)?;
            if let (
                Request::Compare(StyleProperty::Color | StyleProperty::BackgroundColor),
                Observation::Observed(TargetValue::Present(value)),
            ) = (field.property, &record.value)
            {
                style::validate_color(value)
                    .map_err(|error| required(format!("{id}: {}", error.detail)))?;
            }
        }
        if let (Observation::Observed(records), Request::Compare(targets)) =
            (&self.layout, request.layout)
        {
            if records.len() != targets.len() {
                return Err(required(format!(
                    "layout: expected {} records, received {}",
                    targets.len(),
                    records.len()
                )));
            }
            for (record, target) in records.iter().zip(targets) {
                if record.target != *target {
                    return Err(required(format!(
                        "layout:#{target}: record identity/order does not match request"
                    )));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn incomplete(&self) -> bool {
        let none_observed = !matches!(self.dom, Observation::Observed(_))
            && !matches!(self.layout, Observation::Observed(_))
            && !matches!(self.canvas, Observation::Observed(_))
            && !self
                .styles
                .iter()
                .any(|s| matches!(s.value, Observation::Observed(_)));
        none_observed
            || matches!(self.dom, Observation::Unsupported(_))
            || matches!(self.layout, Observation::Unsupported(_))
            || matches!(self.canvas, Observation::Unsupported(_))
            || self
                .styles
                .iter()
                .any(|s| matches!(s.value, Observation::Unsupported(_)))
    }
}

pub(crate) fn validate_request(request: ObservationRequest) -> Result<(), HarnessError> {
    let mut targets = std::collections::BTreeSet::new();
    let mut fields = std::collections::BTreeSet::new();
    for field in request.styles {
        if !fields.insert((field.target, field.name)) {
            return Err(error("duplicate style request"));
        }
        if let Request::Compare(property) = field.property {
            if property.name() != field.name {
                return Err(error("style request property/name mismatch"));
            }
            targets.insert(field.target);
        }
    }
    if let Request::Compare(ids) = request.layout {
        if ids.is_empty() {
            return Err(error("layout Compare requires at least one target"));
        }
        let mut layout = std::collections::BTreeSet::new();
        for id in ids {
            if !layout.insert(id) {
                return Err(error("duplicate layout request"));
            }
            targets.insert(id);
        }
    }
    if targets.len() > 8 || targets.iter().any(|id| id.is_empty() || id.len() > 8192) {
        return Err(error("observation target limit"));
    }
    Ok(())
}

// Shared exact equality/diff semantics; identity and provenance are never inputs.
pub(crate) fn compare_values(
    id: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> (bool, Vec<Difference>) {
    let equal = left == right;
    (
        equal,
        if equal {
            vec![]
        } else {
            differences(id, left, right)
        },
    )
}

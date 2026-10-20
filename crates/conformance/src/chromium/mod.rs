mod cdp;
mod error;
pub(crate) mod ingest;
pub(crate) mod inspection;
mod process;
mod screenshot;
#[cfg(test)]
pub(crate) mod tests;

use crate::{
    environment::{HEIGHT, SAMPLE, URL, WIDTH},
    model::{CanvasColor, Fixture, FixtureKind, TestId},
};
pub(crate) use error::Failure;
use error::{Error, Phase};
use process::{Cancellation, OwnedChromium};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct BrowserIdentity {
    product: String,
    version: String,
    revision: String,
    protocol_version: String,
    platform: String,
    capture_profile: String,
}
#[derive(Debug)]
pub(super) struct ChromiumCapture {
    pub(crate) color: Option<CanvasColor>,
    pub(crate) inspection: Option<inspection::Inspection>,
    pub(crate) identity: BrowserIdentity,
    #[cfg(test)]
    default_favicon_aborted: bool,
}
struct ChromiumConfig {
    executable: PathBuf,
    startup: Duration,
    navigation: Duration,
    capture: Duration,
    shutdown: Duration,
    #[cfg(test)]
    scripts_disabled: bool,
}
impl ChromiumConfig {
    fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            startup: Duration::from_secs(10),
            navigation: Duration::from_secs(5),
            capture: Duration::from_secs(5),
            shutdown: Duration::from_secs(5),
            #[cfg(test)]
            scripts_disabled: true,
        }
    }
}
fn flags() -> Vec<String> {
    [
        "--headless",
        "--remote-debugging-pipe",
        "--no-first-run",
        "--no-default-browser-check",
        "--no-startup-window",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-extensions",
        "--disable-component-extensions-with-background-pages",
        "--disable-default-apps",
        "--disable-sync",
        "--metrics-recording-only",
        "--force-color-profile=srgb",
        "--force-device-scale-factor=1",
        "--window-size=640,480",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
fn identity(value: Value) -> Result<BrowserIdentity, Error> {
    let pin: Value = serde_json::from_str(include_str!("../../chromium-reference.json"))
        .map_err(|e| Error::Configuration(format!("invalid reference pin: {e}")))?;
    let product = cdp::string(&value, "product")?.to_owned();
    let version = product
        .strip_prefix("Chrome/")
        .ok_or_else(|| {
            Error::IncompatibleBrowser(format!("unexpected browser product: {product}"))
        })?
        .to_owned();
    let revision = cdp::string(&value, "revision")?.to_owned();
    let protocol_version = cdp::string(&value, "protocolVersion")?.to_owned();
    if version != pin["version"]
        || revision != pin["revision"]
        || protocol_version != pin["protocol_version"]
    {
        return Err(Error::IncompatibleBrowser(format!(
            "reference mismatch: product={product}, revision={revision}, protocol={protocol_version}"
        )));
    }
    Ok(BrowserIdentity {
        product,
        version,
        revision,
        protocol_version,
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        capture_profile: cdp::string(&pin, "capture_profile")?.into(),
    })
}
#[cfg(test)]
fn capture_html(
    html: &[u8],
    config: &ChromiumConfig,
    cancel: &Cancellation,
) -> Result<ChromiumCapture, Failure> {
    capture_requested_html(
        html,
        crate::projection::ObservationRequest::CANVAS,
        config,
        cancel,
    )
}
fn capture_requested_html(
    html: &[u8],
    request: crate::projection::ObservationRequest,
    config: &ChromiumConfig,
    cancel: &Cancellation,
) -> Result<ChromiumCapture, Failure> {
    #[cfg(test)]
    if !config.scripts_disabled {
        tests::require_script_positive_control(html).map_err(|error| Failure {
            primary: Some(error),
            cleanup: vec![],
        })?;
    }
    crate::projection::validate_request(request).map_err(|e| configuration(&e.detail))?;
    let executable = std::fs::canonicalize(&config.executable).map_err(|e| Failure {
        primary: Some(Error::Configuration(format!(
            "Chromium executable unavailable: {e}"
        ))),
        cleanup: vec![],
    })?;
    let start = Instant::now() + config.startup;
    let mut child = OwnedChromium::launch(&executable, &flags(), start, cancel)?;
    let mut cdp = cdp::Cdp::new(&mut child, cancel);
    let result = (|| {
        let identity =
            identity(cdp.call("Browser.getVersion", json!({}), start, Phase::Startup)?)?;
        // An ordinary headless page has the presentation path qualified by the
        // Linux A/B run. It needs no bootstrap; there is no hidden-target fallback.
        let target = cdp.call(
            "Target.createTarget",
            json!({"url":"about:blank"}),
            start,
            Phase::Startup,
        )?;
        cdp.target = Some(cdp::string(&target, "targetId")?.into());
        cdp.call(
            "Target.setDiscoverTargets",
            // Observe page/frame targets. Browser-owned background workers
            // are outside the fixture session and its network guarantee.
            json!({"discover":true,"filter":[{"type":"page"},{"type":"iframe"}]}),
            start,
            Phase::Startup,
        )?;
        let session = cdp.call(
            "Target.attachToTarget",
            json!({"targetId":cdp::string(&target,"targetId")?,"flatten":true}),
            start,
            Phase::Startup,
        )?;
        cdp.session = Some(cdp::string(&session, "sessionId")?.into());
        #[cfg(test)]
        let scripts_disabled = config.scripts_disabled;
        #[cfg(not(test))]
        let scripts_disabled = true;
        for (method, params) in [
            ("Page.enable", json!({})),
            ("Page.setLifecycleEventsEnabled", json!({"enabled":true})),
            ("Network.enable", json!({})),
            ("Network.setCacheDisabled", json!({"cacheDisabled":true})),
            ("Network.setBypassServiceWorker", json!({"bypass":true})),
            (
                "Emulation.setScriptExecutionDisabled",
                json!({"value":scripts_disabled}),
            ),
            (
                "Emulation.setDeviceMetricsOverride",
                json!({"width":WIDTH,"height":HEIGHT,"deviceScaleFactor":1,"mobile":false,"screenWidth":WIDTH,"screenHeight":HEIGHT}),
            ),
            ("Emulation.setPageScaleFactor", json!({"pageScaleFactor":1})),
            (
                "Fetch.enable",
                json!({"patterns":[{"urlPattern":"*","requestStage":"Request"}]}),
            ),
        ] {
            cdp.call(method, params, start, Phase::Startup)?;
        }
        let tree = cdp.call("Page.getFrameTree", json!({}), start, Phase::Startup)?;
        let frame = cdp::string(&tree["frameTree"]["frame"], "id")?.to_owned();
        cdp.navigation = Some(cdp::Navigation::new(frame, html));
        let nav_deadline = Instant::now() + config.navigation;
        let nav = cdp.call(
            "Page.navigate",
            json!({"url":URL}),
            nav_deadline,
            Phase::Navigation,
        )?;
        cdp.navigation.as_mut().unwrap().acknowledge(&nav)?;
        cdp.wait_ready(nav_deadline)?;
        let capture_deadline = Instant::now() + config.capture;
        let document = cdp.call(
            "DOM.getDocument",
            json!({"depth":0}),
            capture_deadline,
            Phase::Capture,
        )?;
        #[cfg(test)]
        cdp.capture_operation("verify document URL/mode");
        let document_node = cdp
            .navigation
            .as_mut()
            .unwrap()
            .inspect_document(&document)?;
        #[cfg(test)]
        cdp.capture_operation_completed();
        // Query Chromium's parsed live document; do not parse HTML or rel tokens
        // in the harness. Even inert link elements are outside this narrow profile.
        let links = cdp.call(
            "DOM.querySelector",
            json!({"nodeId":document_node,"selector":"link"}),
            capture_deadline,
            Phase::Capture,
        )?;
        let tree = cdp.call(
            "Page.getFrameTree",
            json!({}),
            capture_deadline,
            Phase::Capture,
        )?;
        cdp.navigation
            .as_mut()
            .unwrap()
            .verify_document_policy(&links, &tree)?;
        let metrics = cdp.call(
            "Page.getLayoutMetrics",
            json!({}),
            capture_deadline,
            Phase::Capture,
        )?;
        #[cfg(test)]
        cdp.capture_operation("verify viewport");
        let layout_width = validate_viewport(&metrics, request)?;
        #[cfg(test)]
        cdp.capture_operation_completed();
        let requested = crate::projection::inspection_request(request);
        let inspection = if requested.dom
            || !requested.style_targets.is_empty()
            || !requested.layout_targets.is_empty()
        {
            let frame_id = cdp::string(&tree["frameTree"]["frame"], "id")?;
            let inspection = inspection::collect(&mut cdp, frame_id, &requested, capture_deadline)?;
            if inspection.layout_width != layout_width {
                return Err(Error::Capture(
                    "inspection layout width disagrees with CSS layout viewport".into(),
                ));
            }
            Some(inspection)
        } else {
            None
        };
        let color = if matches!(request.canvas, crate::projection::Request::Compare(())) {
            let screenshot = cdp.call(
                "Page.captureScreenshot",
                json!({"format":"png","fromSurface":true,"captureBeyondViewport":false}),
                capture_deadline,
                Phase::Capture,
            )?;
            #[cfg(test)]
            cdp.capture_operation("decode PNG and sample pixel");
            let color = screenshot::sample(cdp::string(&screenshot, "data")?)?;
            Some(color)
        } else {
            None
        };
        #[cfg(test)]
        cdp.capture_operation_completed();
        // A post-capture frame query is a protocol barrier and verifies that
        // the screenshot still belongs to the acknowledged loaded document.
        let tree = cdp.call(
            "Page.getFrameTree",
            json!({}),
            capture_deadline,
            Phase::Capture,
        )?;
        #[cfg(test)]
        cdp.capture_operation("verify frame identity and final capture deadline");
        cdp.navigation.as_ref().unwrap().verify_tree(&tree)?;
        cancel.check(capture_deadline, Phase::Capture)?;
        if !cdp.navigation.as_ref().unwrap().ready() {
            return Err(Error::Navigation(
                "document changed during screenshot".into(),
            ));
        }
        cdp.wait_resource_completion(capture_deadline)?;
        // Resource completion may consume more events. Recheck the document
        // after it, and finish any candidate first observed by this final barrier.
        let tree = cdp.call(
            "Page.getFrameTree",
            json!({}),
            capture_deadline,
            Phase::Capture,
        )?;
        cdp.navigation.as_ref().unwrap().verify_tree(&tree)?;
        cdp.wait_resource_completion(capture_deadline)?;
        #[cfg(test)]
        cdp.capture_operation_completed();
        cdp.close_browser(capture_deadline)?;
        Ok(ChromiumCapture {
            color,
            inspection,
            identity,
            #[cfg(test)]
            default_favicon_aborted: cdp.navigation.as_ref().unwrap().verified_favicon(),
        })
    })();
    #[cfg(test)]
    if result.is_err() {
        eprintln!("AG2 capture failure: {}", cdp.failure_diagnostic());
    }
    drop(cdp);
    #[cfg(test)]
    tests::comparison_before_cleanup(&child, cancel);
    let cleanup = child.finish(config.shutdown);
    let result = result.and_then(|capture| {
        // Cancellation during cleanup still suppresses a success report.
        cancel.check_cancelled()?;
        Ok(capture)
    });
    match (result, cleanup.is_empty()) {
        (Ok(capture), true) => Ok(capture),
        (result, _) => Err(Failure {
            primary: result.err(),
            cleanup,
        }),
    }
}

// Native phase inspection verifies the configured inner dimensions separately.
// Client dimensions exclude platform scrollbars; canvas retains its full-size guard.
fn validate_viewport(
    metrics: &Value,
    request: crate::projection::ObservationRequest,
) -> Result<f64, Error> {
    let canvas = matches!(request.canvas, crate::projection::Request::Compare(()));
    let number = |object: &str, field: &str| {
        metrics
            .get(object)
            .and_then(|v| v.get(field))
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
    };
    let mut rejected = vec![];
    let mut check = |object: &str, field: &str, valid: bool, expected: Value| {
        if !valid {
            let actual = metrics.get(object).and_then(|v| v.get(field));
            rejected.push(json!({"field":format!("{object}.{field}"),"expected":expected,"actual":diagnostic_scalar(actual),"reason":match actual {None=>"missing",Some(Value::Number(_))=>"numeric_mismatch",_=>"malformed"}}));
        }
    };
    for object in ["cssLayoutViewport", "cssVisualViewport"] {
        for (field, limit) in [("clientWidth", WIDTH), ("clientHeight", HEIGHT)] {
            let actual = number(object, field);
            let valid = actual.is_some_and(|n| {
                if canvas {
                    n == f64::from(limit)
                } else {
                    n > 0.0
                        && n <= f64::from(limit)
                        && n.fract() == 0.0
                        && (object == "cssLayoutViewport"
                            || Some(n) == number("cssLayoutViewport", field))
                }
            });
            let expected = if canvas {
                json!(limit)
            } else {
                json!({"positive_integer_at_most":limit,"matches_layout_viewport":object=="cssVisualViewport"})
            };
            check(object, field, valid, expected);
        }
        for field in ["pageX", "pageY"] {
            check(object, field, number(object, field) == Some(0.0), json!(0));
        }
    }
    for field in ["offsetX", "offsetY"] {
        check(
            "cssVisualViewport",
            field,
            number("cssVisualViewport", field) == Some(0.0),
            json!(0),
        );
    }
    check(
        "cssVisualViewport",
        "scale",
        number("cssVisualViewport", "scale") == Some(1.0),
        json!(1),
    );
    // Optional in the pinned protocol. Present malformed/non-unit zoom is invalid.
    if metrics
        .get("cssVisualViewport")
        .and_then(|v| v.get("zoom"))
        .is_some()
    {
        check(
            "cssVisualViewport",
            "zoom",
            number("cssVisualViewport", "zoom") == Some(1.0),
            json!(1),
        );
    }
    if !rejected.is_empty() {
        let message = if rejected.iter().all(|field| {
            field["field"] == "cssVisualViewport.scale"
                || field["field"] == "cssVisualViewport.zoom"
        }) {
            "unexpected visual scale"
        } else {
            "unexpected viewport dimensions or scroll"
        };
        return Err(Error::Capture(format!(
            "{message}; viewport={}",
            viewport_diagnostic(metrics, canvas, &rejected)
        )));
    }
    Ok(number("cssLayoutViewport", "clientWidth").expect("validated layout width"))
}

fn diagnostic_scalar(value: Option<&Value>) -> Value {
    match value {
        None => json!({"missing":true}),
        Some(Value::Number(n)) => Value::Number(n.clone()),
        Some(Value::Null) => json!({"malformed":"null"}),
        Some(Value::Bool(_)) => json!({"malformed":"boolean"}),
        Some(Value::String(_)) => json!({"malformed":"string"}),
        Some(Value::Array(_)) => json!({"malformed":"array"}),
        Some(Value::Object(_)) => json!({"malformed":"object"}),
    }
}

fn viewport_diagnostic(metrics: &Value, canvas: bool, rejected: &[Value]) -> Value {
    // Copy only fixed protocol fields, never arbitrary remote strings or nested
    // objects. Missing and malformed facts stay explicit, without defaults.
    let mut observed = serde_json::Map::new();
    for (name, fields) in [
        (
            "cssLayoutViewport",
            &["pageX", "pageY", "clientWidth", "clientHeight"][..],
        ),
        (
            "cssVisualViewport",
            &[
                "offsetX",
                "offsetY",
                "pageX",
                "pageY",
                "clientWidth",
                "clientHeight",
                "scale",
                "zoom",
            ][..],
        ),
        ("cssContentSize", &["x", "y", "width", "height"][..]),
    ] {
        let value = metrics.get(name);
        observed.insert(
            name.into(),
            if value.is_some_and(Value::is_object) {
                Value::Object(
                    fields
                        .iter()
                        .map(|field| {
                            (
                                (*field).into(),
                                diagnostic_scalar(value.and_then(|v| v.get(field))),
                            )
                        })
                        .collect(),
                )
            } else {
                diagnostic_scalar(value)
            },
        );
    }
    json!({"expected":{"configured_width":WIDTH,"configured_height":HEIGHT,"canvas_full_client_size":canvas,"pageX":0,"pageY":0,"offsetX":0,"offsetY":0,"scale":1,"zoom_if_present":1},"observed":observed,"rejected":rejected})
}

fn attribute_capture_failure(mut failure: Failure, input: &CaptureInput<'_>) -> Failure {
    if let Some(Error::Capture(detail)) = &mut failure.primary {
        use crate::projection::Request;
        let request = input.request;
        let families: Vec<_> = [
            ("dom", !matches!(request.dom, Request::NotRequested)),
            ("style", !request.styles.is_empty()),
            ("layout", !matches!(request.layout, Request::NotRequested)),
            ("canvas", !matches!(request.canvas, Request::NotRequested)),
        ]
        .into_iter()
        .filter_map(|(name, requested)| requested.then_some(name))
        .collect();
        // Registry paths are relative to the conformance crate. Bound the
        // context even for internal test inputs; never include HTML or expected data.
        let context = json!({"id":input.id.0.chars().take(128).collect::<String>(),"source":format!("crates/conformance/{}", input.source.chars().take(256).collect::<String>()),"families":families});
        *detail = format!("{detail}; fixture={context}");
    }
    failure
}

#[derive(Serialize)]
struct Observation<'a> {
    test_id: &'a str,
    source: &'a str,
    canvas_color_rgb8: Option<CanvasColor>,
    inspection: Option<inspection::Inspection>,
    browser: BrowserIdentity,
}
#[derive(Serialize)]
struct CaptureReport<'a> {
    schema: &'static str,
    document_url: &'static str,
    viewport: [u32; 2],
    device_scale_factor: u32,
    sample_pixel: [u32; 2],
    observations: Vec<Observation<'a>>,
}
/// Only fixture bytes and identity cross the capture boundary, never expectations.
pub(crate) struct CaptureInput<'a> {
    pub(crate) id: TestId,
    pub(crate) source: &'a str,
    pub(crate) html: &'a [u8],
    pub(crate) request: crate::projection::ObservationRequest,
}
#[derive(Debug)]
pub(crate) struct CapturedFixture {
    pub(crate) id: TestId,
    pub(crate) source: String,
    pub(crate) capture: ChromiumCapture,
}

pub(crate) fn configuration(detail: &str) -> Failure {
    Failure {
        primary: Some(Error::Configuration(detail.into())),
        cleanup: vec![],
    }
}

pub(crate) struct Options<'a> {
    pub(crate) executable: Option<PathBuf>,
    test_id: Option<&'a str>,
}

pub(crate) fn options(args: &[String]) -> Result<Options<'_>, Failure> {
    let mut executable = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE").map(PathBuf::from);
    let mut id = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--chromium-executable" {
            executable =
                Some(PathBuf::from(args.next().ok_or_else(|| {
                    configuration("--chromium-executable requires a path")
                })?));
        } else if arg.starts_with('-') || id.replace(arg.as_str()).is_some() {
            return Err(configuration(
                "expected --chromium-executable PATH and at most one fixture ID",
            ));
        }
    }
    Ok(Options {
        executable,
        test_id: id,
    })
}
impl Options<'_> {
    pub(crate) fn select_fixtures(&self) -> Result<Vec<&'static Fixture>, Failure> {
        crate::fixtures::select(self.test_id)
            .map_err(|e| configuration(&format!("{}: {}", e.code, e.detail)))
    }
}

/// Standalone startup phase only: no parser/runtime workers, concurrent FD
/// openers, unrelated children, or competing reapers may exist in this process.
/// Complete the entire selected batch before starting Borrowser. Each capture
/// verifies cleanup before the next launch. No native operations may follow
/// the later Borrowser phase. This is not a general-purpose library launcher.
pub(crate) fn capture_batch(
    inputs: &[CaptureInput<'_>],
    executable: &Path,
) -> Result<Vec<CapturedFixture>, Failure> {
    let standalone = process::Standalone::enter().map_err(|error| Failure {
        primary: Some(error),
        cleanup: vec![],
    })?;
    let config = ChromiumConfig::new(executable.to_owned());
    let cancel = Cancellation::default();
    let mut captures = Vec::with_capacity(inputs.len());
    for input in inputs {
        #[cfg(test)]
        crate::comparison::tests::record(crate::comparison::tests::Event::CaptureStarted(input.id));
        let capture = capture_requested_html(input.html, input.request, &config, &cancel)
            .map_err(|failure| attribute_capture_failure(failure, input))?;
        #[cfg(test)]
        crate::comparison::tests::record(crate::comparison::tests::Event::CaptureCleaned(input.id));
        captures.push(CapturedFixture {
            id: input.id,
            source: input.source.into(),
            capture,
        });
    }
    cancel.check_cancelled().map_err(|error| Failure {
        primary: Some(error),
        cleanup: vec![],
    })?;
    drop(standalone);
    #[cfg(test)]
    crate::comparison::tests::record(crate::comparison::tests::Event::StandaloneReleased);
    Ok(captures)
}

pub(crate) fn run(args: &[String], out: &mut impl Write) -> io::Result<u8> {
    let execute = || -> Result<CaptureReport<'_>, Failure> {
        let options = options(args)?;
        let fixtures = options.select_fixtures()?;
        let inputs = fixtures
            .iter()
            .filter_map(|fixture| {
                let FixtureKind::Runnable { html, .. } = fixture.kind else {
                    return None;
                };
                Some(CaptureInput {
                    id: fixture.id,
                    source: fixture.source,
                    html,
                    request: fixture.request,
                })
            })
            .collect::<Vec<_>>();
        if inputs.is_empty() {
            return Err(configuration("selection contains no runnable fixtures"));
        }
        let executable = options.executable.as_deref().ok_or_else(|| {
            configuration("set BORROWSER_CHROMIUM_EXECUTABLE or --chromium-executable")
        })?;
        let captures = capture_batch(&inputs, executable)?;
        let observations = inputs
            .iter()
            .zip(captures)
            .map(|(input, capture)| Observation {
                test_id: input.id.0,
                source: input.source,
                canvas_color_rgb8: capture.capture.color,
                inspection: capture.capture.inspection,
                browser: capture.capture.identity,
            })
            .collect();
        Ok(CaptureReport {
            schema: "borrowser.chromium-phases.v1",
            document_url: URL,
            viewport: [WIDTH, HEIGHT],
            device_scale_factor: 1,
            sample_pixel: SAMPLE,
            observations,
        })
    };
    match execute() {
        Ok(report) => {
            let mut bytes = serde_json::to_vec(&report)?;
            bytes.push(b'\n');
            if bytes.len() > ingest::MAX_CAPTURE_BYTES {
                configuration("capture exceeds 64 KiB")
                    .write_diagnostics(&mut io::stderr().lock())?;
                return Ok(2);
            }
            out.write_all(&bytes)?;
            Ok(0)
        }
        Err(failure) => {
            failure.write_diagnostics(&mut io::stderr().lock())?;
            Ok(2)
        }
    }
}

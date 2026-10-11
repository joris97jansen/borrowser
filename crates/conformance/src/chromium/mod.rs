mod cdp;
mod error;
mod process;
mod screenshot;
#[cfg(test)]
mod tests;

use crate::{
    environment::{HEIGHT, SAMPLE, URL, WIDTH},
    model::{CanvasColor, FixtureKind},
};
use error::{Error, Failure, Phase};
use process::{Cancellation, OwnedChromium};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::PathBuf,
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
    color: CanvasColor,
    identity: BrowserIdentity,
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
fn capture_html(
    html: &[u8],
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
        for name in ["cssLayoutViewport", "cssVisualViewport"] {
            let v = &metrics[name];
            if v["clientWidth"] != WIDTH
                || v["clientHeight"] != HEIGHT
                || v["pageX"] != 0
                || v["pageY"] != 0
            {
                return Err(Error::Capture(
                    "unexpected viewport dimensions or scroll".into(),
                ));
            }
        }
        if metrics["cssVisualViewport"]["scale"] != 1 {
            return Err(Error::Capture("unexpected visual scale".into()));
        }
        #[cfg(test)]
        cdp.capture_operation_completed();
        let screenshot = cdp.call(
            "Page.captureScreenshot",
            json!({"format":"png","fromSurface":true,"captureBeyondViewport":false}),
            capture_deadline,
            Phase::Capture,
        )?;
        #[cfg(test)]
        cdp.capture_operation("decode PNG and sample pixel");
        let color = screenshot::sample(cdp::string(&screenshot, "data")?)?;
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

#[derive(Serialize)]
struct Observation<'a> {
    test_id: &'a str,
    source: &'a str,
    canvas_color_rgb8: [u8; 3],
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
pub(crate) fn run(args: &[String], out: &mut impl Write) -> io::Result<u8> {
    let execute = || -> Result<CaptureReport<'_>, Failure> {
        let error = |s: &str| Failure {
            primary: Some(Error::Configuration(s.into())),
            cleanup: vec![],
        };
        let mut executable = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE").map(PathBuf::from);
        let mut id = None;
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if arg == "--chromium-executable" {
                executable =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        error("--chromium-executable requires a path")
                    })?));
            } else if arg.starts_with('-') || id.replace(arg.as_str()).is_some() {
                return Err(error(
                    "expected --chromium-executable PATH and at most one fixture ID",
                ));
            }
        }
        let executable = executable
            .ok_or_else(|| error("set BORROWSER_CHROMIUM_EXECUTABLE or --chromium-executable"))?;
        let fixtures: Vec<_> = crate::fixtures::FIXTURES
            .iter()
            .filter(|f| id.is_none_or(|id| id == f.id.0))
            .collect();
        if fixtures.is_empty() {
            return Err(error("unknown fixture ID"));
        }
        let _standalone = process::Standalone::enter().map_err(|e| Failure {
            primary: Some(e),
            cleanup: vec![],
        })?;
        let config = ChromiumConfig::new(executable);
        let cancel = Cancellation::default();
        let mut observations = Vec::new();
        for fixture in fixtures {
            let FixtureKind::Runnable { html, .. } = fixture.kind else {
                return Err(error("fixture is not runnable"));
            };
            let capture = capture_html(html, &config, &cancel)?;
            observations.push(Observation {
                test_id: fixture.id.0,
                source: fixture.source,
                canvas_color_rgb8: capture.color.0,
                browser: capture.identity,
            });
        }
        Ok(CaptureReport {
            schema: "borrowser.chromium-canvas.v1",
            document_url: URL,
            viewport: [WIDTH, HEIGHT],
            device_scale_factor: 1,
            sample_pixel: SAMPLE,
            observations,
        })
    };
    match execute() {
        Ok(report) => {
            serde_json::to_writer(&mut *out, &report)?;
            writeln!(out)?;
            Ok(0)
        }
        Err(failure) => {
            if let Some(error) = failure.primary {
                eprintln!("ERROR chromium.{}: {error}", error.code());
            }
            for error in failure.cleanup {
                eprintln!("ERROR chromium.cleanup: {error}");
            }
            Ok(2)
        }
    }
}

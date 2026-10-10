//! Temporary qualification experiment, compiled only by the Rust test harness.
//! No target fallback or request-policy change is available to the CLI.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Hidden,
    Page,
}

pub(super) fn require_original_fixture(html: &[u8]) -> Result<(), Error> {
    if crate::fixtures::FIXTURES.iter().any(|fixture| {
        matches!(fixture.kind, FixtureKind::Runnable { html: original, .. } if original == html)
    }) {
        Ok(())
    } else {
        Err(Error::Configuration(
            "target probe requires unmodified AG1 fixture bytes".into(),
        ))
    }
}

pub(super) enum Action {
    FixturePolicy,
    Recorded,
    AbortFavicon(String),
}

// The original fixtures contain no authored resources, including icons. Only
// within that byte-checked experiment can the default favicon be attributed to
// browser behavior. URL/type/initiator alone are NOT a production authority to
// exempt requests: an authored <link rel=icon> can use the same browser helper.
const FAVICON: &str = "https://borrowser.invalid/favicon.ico";

#[derive(Default)]
pub(super) struct Resources {
    network: Option<String>,
    fetch: Option<String>,
    terminal: bool,
}

impl Resources {
    fn failure() -> Error {
        Error::Navigation("ambiguous or duplicate target-probe favicon request".into())
    }

    fn id(value: &Value, name: &str) -> Result<String, Error> {
        let id = cdp::string(value, name)?;
        if id.is_empty() || id.len() > 128 {
            return Err(Self::failure());
        }
        Ok(id.into())
    }

    fn correlate(&self) -> Result<(), Error> {
        if let (Some(network), Some(fetch)) = (&self.network, &self.fetch)
            && network != fetch
        {
            return Err(Self::failure());
        }
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<(), Error> {
        self.correlate()?;
        match (&self.network, &self.fetch, self.terminal) {
            (None, None, false) | (Some(_), Some(_), true) => Ok(()),
            _ => Err(Error::Navigation(
                "incomplete target-probe favicon interception".into(),
            )),
        }
    }

    pub(super) fn event(&mut self, method: &str, p: &Value, frame: &str) -> Result<Action, Error> {
        let default_icon = p["frameId"] == frame
            && p["request"]["url"] == FAVICON
            && p["request"]["method"] == "GET";
        match method {
            "Network.requestWillBeSent" if p["type"] != "Document" => {
                if !default_icon
                    || p["type"] != "Other"
                    || p["initiator"]["type"] != "other"
                    || p.get("redirectResponse").is_some()
                {
                    eprintln!("AG2 target probe: page-authored or unclassified resource; rejected");
                    return Ok(Action::FixturePolicy);
                }
                if self.network.is_some() {
                    return Err(Self::failure());
                }
                self.network = Some(Self::id(p, "requestId")?);
                self.correlate()?;
                eprintln!(
                    "AG2 target probe: browser default favicon; original fixture has no authored resources; type=Other initiator=other network_id={:?}",
                    self.network
                );
                Ok(Action::Recorded)
            }
            "Fetch.requestPaused" if p["resourceType"] != "Document" => {
                if !default_icon
                    || p["resourceType"] != "Other"
                    || p.get("redirectedRequestId").is_some()
                {
                    eprintln!(
                        "AG2 target probe: page-authored or unclassified Fetch request; rejected"
                    );
                    return Ok(Action::FixturePolicy);
                }
                if self.fetch.is_some() {
                    return Err(Self::failure());
                }
                self.fetch = Some(Self::id(p, "networkId")?);
                self.correlate()?;
                let request = Self::id(p, "requestId")?;
                eprintln!(
                    "AG2 target probe: abort default favicon before network; fetch_id={request}"
                );
                Ok(Action::AbortFavicon(request))
            }
            "Network.loadingFailed" | "Network.loadingFinished" | "Network.responseReceived"
                if p["requestId"].as_str().is_some_and(|id| {
                    self.network.as_deref() == Some(id) || self.fetch.as_deref() == Some(id)
                }) =>
            {
                // A favicon was intercepted at Request stage and aborted. No
                // real HTTP response or successful transfer is permissible.
                if method != "Network.loadingFailed" || self.fetch.is_none() || self.terminal {
                    return Err(Self::failure());
                }
                self.terminal = true;
                eprintln!("AG2 target probe: default favicon loading failed after interception");
                Ok(Action::Recorded)
            }
            _ => Ok(Action::FixturePolicy),
        }
    }
}

#[test]
#[ignore = "diagnostic A/B experiment; requires pinned Chromium and --test-threads=1; not Linux acceptance"]
fn real_chromium_target_presentation_experiment() {
    let executable = PathBuf::from(
        std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
            .expect("explicit target probe requires pinned Chromium"),
    );
    let _standalone = process::Standalone::enter().unwrap();
    let mut failures = Vec::new();
    for fixture in crate::fixtures::FIXTURES {
        let FixtureKind::Runnable { html, expected, .. } = fixture.kind else {
            unreachable!()
        };
        for target in [Target::Hidden, Target::Page] {
            eprintln!(
                "AG2 target probe: fixture={} iteration=1/1 target={target:?} starting; screenshot_feature=source-default-disabled,no-launch-override,effective-state-unobserved",
                fixture.id.0
            );
            let config = ChromiumConfig {
                target_probe: Some(target),
                ..ChromiumConfig::new(executable.clone())
            };
            match capture_html(html, &config, &Cancellation::default()) {
                Ok(capture) => {
                    // The ordinary sampler already validates PNG dimensions,
                    // color metadata, opacity and physical sample coordinates.
                    eprintln!(
                        "AG2 target probe: fixture={} target={target:?} screenshot=valid-png-640x480 rgb8={:?} cleanup=verified identity={:?}",
                        fixture.id.0, capture.color.0, capture.identity
                    );
                    if capture.color != expected {
                        failures.push(format!("{} {target:?}: wrong rendered color", fixture.id.0));
                    }
                }
                Err(failure) => {
                    eprintln!(
                        "AG2 target probe: fixture={} target={target:?} {failure:?}",
                        fixture.id.0
                    );
                    assert!(
                        failure.cleanup.is_empty(),
                        "stop after unverified cleanup: {failure:?}"
                    );
                    // Preserve all failures and still attempt the other target.
                    // A reproduced hidden-target timeout is not a passing test.
                    failures.push(format!("{} {target:?}: {failure:?}", fixture.id.0));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "target experiment failures: {failures:?}"
    );
}

#[test]
fn probe_cannot_exempt_authored_resources_or_non_original_bytes() {
    for fixture in crate::fixtures::FIXTURES {
        let FixtureKind::Runnable { html, .. } = fixture.kind else {
            unreachable!()
        };
        require_original_fixture(html).unwrap();
        let mut changed = html.to_vec();
        changed.extend_from_slice(b"<link rel=icon href=/favicon.ico>");
        assert!(matches!(
            require_original_fixture(&changed),
            Err(Error::Configuration(_))
        ));
    }
    let request = json!({"requestId":"icon", "frameId":"frame", "type":"Other",
        "initiator":{"type":"other"}, "request":{"url":FAVICON,"method":"GET"}});
    let mut probe = Resources::default();
    assert!(matches!(
        probe.event("Network.requestWillBeSent", &request, "frame"),
        Ok(Action::Recorded)
    ));
    assert!(
        probe
            .event("Network.requestWillBeSent", &request, "frame")
            .is_err()
    );
    for (key, value) in [
        ("type", json!("Image")),
        ("initiator", json!({"type":"parser"})),
        ("frameId", json!("other")),
        ("redirectResponse", json!({})),
    ] {
        let mut authored = request.clone();
        authored[key] = value;
        assert!(matches!(
            Resources::default().event("Network.requestWillBeSent", &authored, "frame"),
            Ok(Action::FixturePolicy)
        ));
    }
    let fetched = json!({"requestId":"fetch", "networkId":"different", "frameId":"frame", "resourceType":"Other", "request":{"url":FAVICON,"method":"GET"}});
    assert!(
        probe
            .event("Fetch.requestPaused", &fetched, "frame")
            .is_err()
    );
}

#[test]
fn probe_requires_correlated_aborted_favicon_or_no_favicon() {
    Resources::default().finish().unwrap();
    let network = json!({"requestId":"icon", "frameId":"frame", "type":"Other",
        "initiator":{"type":"other"}, "request":{"url":FAVICON,"method":"GET"}});
    let fetch = json!({"requestId":"fetch", "networkId":"icon", "frameId":"frame", "resourceType":"Other",
        "request":{"url":FAVICON,"method":"GET"}});
    for fetch_first in [false, true] {
        let mut probe = Resources::default();
        let mut events = [
            ("Network.requestWillBeSent", &network),
            ("Fetch.requestPaused", &fetch),
        ];
        if fetch_first {
            events.reverse();
        }
        for (method, event) in events {
            let action = probe.event(method, event, "frame").unwrap();
            if method == "Fetch.requestPaused" {
                assert!(matches!(action, Action::AbortFavicon(id) if id == "fetch"));
            } else {
                assert!(matches!(action, Action::Recorded));
            }
            assert!(
                probe.finish().is_err(),
                "incomplete interception cannot pass"
            );
        }
        for method in ["Network.responseReceived", "Network.loadingFinished"] {
            assert!(
                probe
                    .event(method, &json!({"requestId":"icon"}), "frame")
                    .is_err()
            );
        }
        assert!(matches!(
            probe.event(
                "Network.loadingFailed",
                &json!({"requestId":"icon"}),
                "frame"
            ),
            Ok(Action::Recorded)
        ));
        probe.finish().unwrap();
        assert!(probe.event("Fetch.requestPaused", &fetch, "frame").is_err());
    }
    for id in [Value::Null, json!(0), json!(""), json!("x".repeat(129))] {
        let mut invalid = fetch.clone();
        invalid["networkId"] = id;
        assert!(
            Resources::default()
                .event("Fetch.requestPaused", &invalid, "frame")
                .is_err()
        );
    }
}

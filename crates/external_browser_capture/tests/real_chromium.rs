//! Explicit local qualification only. Neither workspace tests nor --all-features launch a browser.
#![cfg(feature = "chromium-cdp")]

#[test]
#[ignore = "requires explicit pinned Chromium distribution and supported rootless Linux host"]
fn pinned_chromium_mechanism_qualification() {
    let configuration =
        std::env::var("AG9G_CONFIGURATION").expect("explicit AG9G_CONFIGURATION required");
    let distribution = std::env::var_os("AG9G_BROWSER_DISTRIBUTION")
        .expect("explicit AG9G_BROWSER_DISTRIBUTION required");
    let collector = std::env::var_os("AG9G_COLLECTOR")
        .expect("explicit path to the conformance-capture executable required");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // The Rust test harness is multithreaded and is not the identified collector.
    // Execute the same explicit single-threaded binary used by local qualification.
    let output = std::process::Command::new(collector)
        .current_dir(root)
        .args([
            "qualify",
            "--purpose",
            "mechanism",
            "--configuration",
            &configuration,
            "--browser-distribution",
        ])
        .arg(distribution)
        .output()
        .expect("launch explicit collector");
    assert!(
        output.status.success(),
        "mechanism qualification NOT ESTABLISHED: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("AG9g Stage 0 mechanism feasibility: GO\n")
    );
}

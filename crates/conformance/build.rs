#[path = "build_support/fixtures.rs"]
mod fixtures;

fn main() {
    println!("cargo:rerun-if-changed=fixtures");
    println!("cargo:rerun-if-changed=build_support");
    println!("cargo:rerun-if-changed=src/metadata_links.rs");
    let result = fixtures::generate(std::path::Path::new(
        &std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"),
    ))
    .and_then(|source| {
        let output =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
        std::fs::write(output.join("fixtures_generated.rs"), source).map_err(|e| e.to_string())
    });
    if let Err(error) = result {
        eprintln!("ERROR fixture.discovery: {error}");
        std::process::exit(1);
    }
    println!("cargo:rerun-if-env-changed=BORROWSER_AG4_SOURCE_TREE");
    let tree = std::env::var("BORROWSER_AG4_SOURCE_TREE")
        .ok()
        .filter(|tree| {
            (tree.len() == 40 || tree.len() == 64)
                && tree
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .unwrap_or_default();
    // Override even an invalid inherited value; ordinary local builds stay usable.
    // AG4's artifact entry point refuses an unstamped controller.
    println!("cargo:rustc-env=BORROWSER_AG4_SOURCE_TREE={tree}");
}

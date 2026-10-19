fn main() {
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

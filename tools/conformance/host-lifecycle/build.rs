use std::process::Command;
fn main() {
    println!("cargo:rerun-if-changed=.");
    // HEAD may be symbolic and worktrees keep refs outside their git directory.
    let symbolic = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
        .expect("git symbolic ref");
    let reference = String::from_utf8(symbolic.stdout).expect("ref encoding");
    for name in ["HEAD", "index", "packed-refs", reference.trim()] {
        if name.is_empty() {
            continue;
        }
        let path = Command::new("git")
            .args(["rev-parse", "--git-path", name])
            .output()
            .expect("git metadata path");
        assert!(path.status.success());
        println!(
            "cargo:rerun-if-changed={}",
            String::from_utf8(path.stdout)
                .expect("git path encoding")
                .trim()
        );
    }
    let revision = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git source identity");
    assert!(revision.status.success());
    let revision = String::from_utf8(revision.stdout).expect("source identity encoding");
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all", "--", "."])
        .output()
        .expect("git source status");
    assert!(status.status.success());
    println!(
        "cargo:rustc-env=LIFECYCLE_SOURCE_REVISION={}",
        revision.trim()
    );
    println!(
        "cargo:rustc-env=LIFECYCLE_SOURCE_CLEAN={}",
        status.stdout.is_empty()
    );
}

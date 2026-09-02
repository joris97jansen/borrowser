use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};

use wpt_test_support::{
    WptSummaryCheck, check_repository_wpt_summary, generate_repository_wpt_summary,
    load_wpt_source_set, materialize_wpt_source_set, update_repository_wpt_summary,
};

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), String> {
    let repository = repository_root();
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice(){[]=>check(&repository),[arg] if arg=="--check"=>check(&repository),[arg] if arg=="--update"=>{let summary=generate_repository_wpt_summary(&repository).map_err(debug)?;update_repository_wpt_summary(&repository,&summary).map_err(debug)?;std::io::stdout().write_all(summary.as_bytes()).map_err(|_|"failed to write canonical summary".to_owned())},[materialize,from,path] if materialize=="--materialize"&&from=="--from-wpt-checkout"=>{let set=load_wpt_source_set(&repository).map_err(debug)?;materialize_wpt_source_set(&repository,Path::new(path),&set).map_err(debug)?;let summary=generate_repository_wpt_summary(&repository).map_err(debug)?;std::io::stdout().write_all(summary.as_bytes()).map_err(|_|"failed to write canonical summary".to_owned())},_=>Err("usage: conformance-wpt-import [--check|--update|--materialize --from-wpt-checkout <path>]".to_owned())}
}
fn check(repository: &Path) -> Result<(), String> {
    let summary = generate_repository_wpt_summary(repository).map_err(debug)?;
    std::io::stdout()
        .write_all(summary.as_bytes())
        .map_err(|_| "failed to write canonical summary".to_owned())?;
    match check_repository_wpt_summary(repository, &summary).map_err(debug)? {
        WptSummaryCheck::Current => Ok(()),
        WptSummaryCheck::Missing => Err("checked-in WPT accounting summary is missing".to_owned()),
        WptSummaryCheck::Stale => Err("checked-in WPT accounting summary is stale".to_owned()),
    }
}
fn debug(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace layout")
        .to_path_buf()
}

//! Single explicit collector entry point. Stage 0 only; never used by normal aggregate execution.
use external_browser_capture::{CaptureError as E, Result, qualification};
use std::path::PathBuf;

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("qualify")) {
        return Err(E::UnavailableMode);
    }
    let mut purpose = None;
    let mut configuration = None;
    let mut distribution = None;
    let mut output = None;
    while let Some(key) = args.next() {
        let value = args.next().ok_or(E::Arguments)?;
        let slot = match key.to_str() {
            Some("--purpose") => &mut purpose,
            Some("--configuration") => &mut configuration,
            Some("--browser-distribution") => &mut distribution,
            Some("--output") => &mut output,
            _ => return Err(E::Arguments),
        };
        if slot.replace(value).is_some() {
            return Err(E::Arguments);
        }
    }
    if purpose.as_deref() != Some(std::ffi::OsStr::new("mechanism")) {
        return Err(E::UnavailableMode);
    }
    let configuration = configuration
        .ok_or(E::Arguments)?
        .into_string()
        .map_err(|_| E::Path)?;
    let root = std::env::current_dir().map_err(|_| E::Read)?;
    let distribution = PathBuf::from(distribution.ok_or(E::Arguments)?);
    let result = qualification::mechanism(&root, &configuration, &distribution)?;
    if let Some(path) = output {
        // This is a feasibility diagnostic, not an artifact/provenance/evidence publication API.
        use std::io::Write;
        let path = PathBuf::from(path);
        let parent = path
            .parent()
            .ok_or(E::Path)?
            .canonicalize()
            .map_err(|_| E::Path)?;
        if parent.starts_with(root.canonicalize().map_err(|_| E::Path)?) {
            return Err(E::Path);
        }
        std::fs::create_dir(&path).map_err(|_| E::Publication)?;
        let mut f = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(path.join("mechanism.txt"))
            .map_err(|_| E::Publication)?;
        f.write_all(result.diagnostic().as_bytes())
            .map_err(|_| E::Publication)?;
    }
    print!("{}", result.diagnostic());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}\nAG9g Stage 0 mechanism qualification: NOT ESTABLISHED");
        std::process::exit(1);
    }
}

use borrowser_host_readiness::{Result, evidence::HostReadinessManifest, require};
use std::{io::Write, path::Path};
fn main() {
    if let Err(error) = run() {
        eprintln!("host-readiness failed: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    require(
        args.len() <= 8 && args.iter().all(|s| s.len() <= 8192),
        "argument bounds",
    )?;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    match args.first().map(String::as_str) {
        Some("__host-driver") => {
            std::io::stdout().write_all(&borrowser_host_readiness::linux::driver(&args[1..])?)?;
            return Ok(());
        }
        Some("__host-namespace") if args.len() == 2 => {
            std::io::stdout().write_all(&borrowser_host_readiness::linux::namespace_worker(
                &args[1],
            )?)?;
            return Ok(());
        }
        Some("__host-idle") if args.len() == 1 => return borrowser_host_readiness::linux::idle(),
        Some(
            "host-seccomp-prerequisite"
            | "host-unshare-fd-prerequisite"
            | "host-process-inspection-prerequisite",
        ) => {
            std::io::stdout().write_all(&borrowser_host_readiness::linux::run(&args)?)?;
            return Ok(());
        }
        _ => {}
    }
    #[cfg(unix)]
    if args.len() == 3 && args[0] == "verify" {
        let bytes = borrowser_host_readiness::artifacts::read_manifest(Path::new(&args[1]))?;
        let manifest = HostReadinessManifest::parse(&bytes)?;
        borrowser_host_readiness::artifacts::verify(
            Path::new(&args[2]),
            &manifest,
            std::time::Instant::now() + std::time::Duration::from_secs(86400),
        )?;
        // Existing canonical bytes only: no timestamps or inferred native-readiness claims.
        std::io::stdout().write_all(&bytes)?;
        return Ok(());
    }
    Err("usage: verify MANIFEST BUNDLE_ROOT | host-seccomp-prerequisite | host-{unshare-fd,process-inspection}-prerequisite UNSHARE SHA256 VERSION; runtime probes require native Linux x86-64 (independent provenance still required)".into())
}

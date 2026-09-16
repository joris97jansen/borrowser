use qualification_prep::{
    Error, Result, canonical,
    configuration::{self, HostRecord, PinRecord},
    distribution::{self, CandidateDistributionManifest},
    identity::BrowserIdentity,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().ok_or(Error::Invalid("command required"))?;
    #[cfg(target_os = "linux")]
    if mode == "__probe-worker" {
        use std::io::Write;
        let raw = args
            .next()
            .ok_or(Error::Invalid("worker input"))?
            .into_string()
            .map_err(|_| Error::Invalid("worker input UTF-8"))?;
        if args.next().is_some() {
            return Err(Error::Invalid("worker arguments"));
        }
        let bytes = qualification_prep::probe_linux::worker(&raw)?;
        std::io::stdout().write_all(&bytes)?;
        return Ok(());
    }
    let mut flags = BTreeMap::new();
    while let Some(key) = args.next() {
        let key = key
            .into_string()
            .map_err(|_| Error::Invalid("argument name"))?;
        let value = args.next().ok_or(Error::Invalid("argument value"))?;
        if flags.len() >= 10 || key.len() > 64 || value.as_encoded_bytes().len() > 65536 {
            return Err(Error::Invalid("argument bounds"));
        }
        if flags.insert(key, value).is_some() {
            return Err(Error::Invalid("duplicate argument"));
        }
    }
    let mut take = |key: &str| -> Result<PathBuf> {
        flags
            .remove(key)
            .map(PathBuf::from)
            .ok_or(Error::Invalid("missing explicit argument"))
    };
    let output = take("--output")?;
    let bytes = match mode.to_str() {
        Some("record") => {
            let kind = take("--kind")?;
            let input = take("--input")?;
            if !flags.is_empty() {
                return Err(Error::Invalid("unknown arguments"));
            }
            let bytes = canonical::read(&input, canonical::RECORD_BYTES)?;
            match kind.to_str() {
                Some("pin") => {
                    let p: PinRecord =
                        serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("pin schema"))?;
                    p.validate()?;
                    canonical::json(&p)?
                }
                Some("host") => {
                    let h: HostRecord = serde_json::from_slice(&bytes)
                        .map_err(|_| Error::Invalid("host schema"))?;
                    h.verify_actual()?;
                    canonical::json(&h)?
                }
                _ => return Err(Error::Invalid("record kind")),
            }
        }
        Some("manifest") => {
            let root = take("--distribution-root")?;
            if !flags.is_empty() {
                return Err(Error::Invalid("unknown arguments"));
            }
            distribution::inventory(&root)?.canonical_bytes()?
        }
        Some("configuration") => {
            let root = take("--collector-source-root")?;
            let pin = take("--reviewed-pin")?;
            let manifest = take("--distribution-manifest")?;
            let browser = take("--browser-identity")?;
            let host = take("--host-record")?;
            if !flags.is_empty() {
                return Err(Error::Invalid("unknown arguments"));
            }
            let pin: PinRecord = canonical::record(&pin)?;
            let host: HostRecord = canonical::record(&host)?;
            let browser: BrowserIdentity = canonical::record(&browser)?;
            let manifest = CandidateDistributionManifest::load(&manifest)?;
            configuration::produce(&root, &pin, &host, &browser, &manifest)?
        }
        Some("identity") => {
            let root = take("--distribution-root")?;
            let manifest = take("--distribution-manifest")?;
            let executable = take("--executable")?;
            let unshare = take("--unshare")?;
            let hash = take("--unshare-sha256")?;
            let version = take("--unshare-version")?;
            if !flags.is_empty() {
                return Err(Error::Invalid("unknown arguments"));
            }
            #[cfg(target_os = "linux")]
            {
                let text = |p: &Path| {
                    p.to_str()
                        .map(String::from)
                        .ok_or(Error::Invalid("identity argument UTF-8"))
                };
                canonical::json(&qualification_prep::probe_linux::probe(
                    &qualification_prep::probe_linux::ProbeInput {
                        distribution: root,
                        manifest: CandidateDistributionManifest::load(&manifest)?,
                        executable: text(&executable)?,
                        unshare,
                        unshare_sha256: text(&hash)?,
                        unshare_version: text(&version)?,
                    },
                )?)?
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (root, manifest, executable, unshare, hash, version);
                return Err(Error::Unsupported);
            }
        }
        _ => {
            return Err(Error::Invalid(
                "supported commands: manifest, identity, configuration, record",
            ));
        }
    };
    canonical::publish_new(Path::new(&output), &bytes)?;
    eprintln!("Candidate preparation input written; not qualification evidence.");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

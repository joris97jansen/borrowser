use crate::{Result, require};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Bootstrap,
    Status,
}
fn command(args: &[String]) -> Result<Command> {
    require(
        args.len() <= 2 && args.iter().all(|s| s.len() <= 32),
        "command bounds",
    )?;
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["authority", "bootstrap"] => Ok(Command::Bootstrap),
        ["status"] => Ok(Command::Status),
        _ => Err(crate::Error(
            "expected authority bootstrap | status; provider operations unavailable",
        )),
    }
}
pub fn run_cli() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).take(3).collect();
    let selected = command(&args)?;
    run(selected)
}
#[cfg(not(target_os = "linux"))]
fn run(_: Command) -> Result<()> {
    Err(crate::Error("production authority commands require Linux"))
}
#[cfg(target_os = "linux")]
fn run(selected: Command) -> Result<()> {
    let tool = build_identity();
    if selected == Command::Bootstrap {
        tool.validate()?;
    }
    let deployment = crate::linux::deployment()?;
    let journal = crate::linux::open_authority(&deployment, selected == Command::Bootstrap, tool)?;
    #[derive(serde::Serialize)]
    struct Report<'a> {
        authority: &'static str,
        root: crate::deployment::AuthorityRootV2,
        state: &'a crate::model::AuthorityStateV2,
    }
    println!(
        "{}",
        serde_json::to_string(&Report {
            authority: crate::model::AUTHORITY,
            root: deployment.marker()?,
            state: journal.state()
        })
        .map_err(|_| crate::Error("report encoding"))?
    );
    Ok(())
}
#[cfg(target_os = "linux")]
fn build_identity() -> crate::model::ToolIdentityV2 {
    crate::model::ToolIdentityV2 {
        package: "borrowser-host-lifecycle".into(),
        package_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: 2,
        source_revision: env!("LIFECYCLE_SOURCE_REVISION").into(),
        cargo_lock_sha256: crate::canonical::sha256(include_bytes!("../Cargo.lock")),
        source_clean: env!("LIFECYCLE_SOURCE_CLEAN") == "true",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_local_authority_commands_exist() {
        assert_eq!(command(&["status".into()]).unwrap(), Command::Status);
        assert_eq!(
            command(&["authority".into(), "bootstrap".into()]).unwrap(),
            Command::Bootstrap
        );
        for name in [
            "allocate",
            "cancel",
            "terminate",
            "reconcile",
            "watch",
            "resolve",
            "publish",
            "RunInstances",
            "TerminateInstances",
        ] {
            assert!(command(&[name.into()]).is_err());
        }
        assert!(command(&[]).is_err());
        assert!(command(&["status".into(), "extra".into()]).is_err());
    }
}

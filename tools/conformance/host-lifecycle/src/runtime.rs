#[cfg(not(target_os = "linux"))]
pub fn run_cli() -> crate::Result<()> {
    Err(crate::Error("production authority commands require Linux"))
}

#[cfg(target_os = "linux")]
pub fn run_cli() -> crate::Result<()> {
    use crate::identity::*;
    use crate::{
        Error, Result, linux, model::*, orchestrator::*, provider::AllocationRequest, require,
        transport::RobotHttp,
    };
    use serde::Deserialize;
    fn input<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
        use std::{io::Read, os::unix::fs::OpenOptionsExt};
        let f = std::fs::File::options()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Error("input open"))?;
        require(
            f.metadata().map_err(|_| Error("input metadata"))?.is_file(),
            "input regular file",
        )?;
        let mut bytes = Vec::new();
        f.take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| Error("input read"))?;
        require(bytes.len() <= 65_536, "input bound")?;
        serde_json::from_slice(&bytes).map_err(|_| Error("input schema"))
    }
    fn report(state: &AccountState) -> Result<()> {
        #[derive(serde::Serialize)]
        struct Report<'a> {
            authority: &'static str,
            state: &'a AccountState,
            phases: Vec<(&'a str, &'static str)>,
        }
        let report = Report {
            authority: AUTHORITY,
            state,
            phases: state
                .operations
                .iter()
                .map(|o| (o.id.as_str(), o.phase()))
                .collect(),
        };
        println!(
            "{}",
            serde_json::to_string(&report).map_err(|_| Error("report encoding"))?
        );
        Ok(())
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    require(
        args.len() <= 4 && args.iter().all(|s| s.len() <= 4096),
        "argument bounds",
    )?;
    let command=args.first().map(String::as_str).ok_or(Error("expected authority bootstrap | allocate --request FILE | status | reconcile --operation ID | watch | cancel --authorization FILE | resolve --resolution FILE"))?;
    let d = linux::deployment()?;
    let bootstrap = args == ["authority", "bootstrap"];
    let tool = build_identity();
    require(
        command == "status" || tool.source_clean,
        "production commands require a clean committed tool build",
    )?;
    let mut store = linux::open_authority(&d, bootstrap)?;
    if command == "status" {
        require(args.len() == 1, "status arguments")?;
        report(store.state())?;
        return Ok(());
    }
    require(
        tool.source_clean,
        "production mutation/observation requires a clean committed tool build",
    )?;
    if bootstrap {
        let e = Envelope {
            account_id: d.account_id,
            authority: AUTHORITY.into(),
            authority_id: d.authority_id,
            event: Event::AuthorityInitialized,
            format: FORMAT.into(),
            operation_id: None,
            previous_sha256: None,
            schema_version: 1,
            sequence: 0,
            time: linux::now()?,
            tool,
        };
        store.append(&e)?;
        return Ok(());
    }
    require(
        store.state().account_id.as_ref() == Some(&d.account_id)
            && store.state().authority_id.as_ref() == Some(&d.authority_id),
        "deployment authority mismatch",
    )?;
    let mut controller = Controller::production(store)?;
    if command == "resolve" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Resolution {
            operation_id: OperationId,
            expected_head: EventDigest,
            event: Event,
            evidence_path: String,
        }
        require(
            args.len() == 3 && args[1] == "--resolution",
            "resolve arguments",
        )?;
        let r: Resolution = input(&args[2])?;
        require(
            controller.state().head.as_ref() == Some(&r.expected_head),
            "stale resolution",
        )?;
        let evidence = r.event.evidence().ok_or(Error("unsupported resolution"))?;
        evidence.validate()?;
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        let f = std::fs::File::options()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&r.evidence_path)
            .map_err(|_| Error("evidence open"))?;
        require(
            f.metadata()
                .map_err(|_| Error("evidence metadata"))?
                .is_file(),
            "evidence regular file",
        )?;
        let mut bytes = Vec::new();
        f.take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|_| Error("evidence read"))?;
        require(bytes.len() as u64 == evidence.bytes, "evidence length")?;
        controller.retain_evidence(&bytes, &evidence.sha256)?;
        controller.resolve(&r.operation_id, &r.expected_head, r.event)?;
    } else {
        require(
            !controller.state().authentication_blocked,
            "authentication suppressed until explicit resolution",
        )?;
        let credentials = linux::Credentials::load()?;
        let mut robot = if command == "allocate" {
            RobotHttp::new(
                credentials,
                linux::product_approval(&d)?,
                d.authority_id.clone(),
            )
        } else {
            RobotHttp::recovery(credentials, d.account_id.clone(), d.authority_id.clone())
        };
        match command {
            "allocate" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    operation_id: OperationId,
                    request: AllocationRequest,
                    authorization: String,
                }
                require(
                    args.len() == 3 && args[1] == "--request",
                    "allocate arguments",
                )?;
                let r: Input = input(&args[2])?;
                require(
                    r.request.product_id == d.product_id,
                    "unapproved product id",
                )?;
                controller.allocate(&r.operation_id, r.request, r.authorization, &mut robot)?;
            }
            "reconcile" => {
                require(
                    args.len() == 3 && args[1] == "--operation",
                    "reconcile arguments",
                )?;
                controller.reconcile(&args[2].parse()?, &mut robot)?;
            }
            "watch" => {
                require(args.len() == 1, "watch arguments")?;
                let scan = controller.watch(&mut robot)?;
                println!(
                    "{}",
                    serde_json::to_string(&scan).map_err(|_| Error("watch report encoding"))?
                );
                report(controller.state())?;
                return require(
                    scan.complete_scan,
                    "watch incomplete; see obligation report",
                );
            }
            "cancel" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    operation_id: OperationId,
                    server_number: ServerNumber,
                    authorization: String,
                }
                require(
                    args.len() == 3 && args[1] == "--authorization",
                    "cancel arguments",
                )?;
                let r: Input = input(&args[2])?;
                controller.cancel(
                    &r.operation_id,
                    r.server_number,
                    r.authorization,
                    &mut robot,
                )?;
            }
            _ => return Err(Error("unsupported command")),
        }
    }
    report(controller.state())
}

#[cfg(target_os = "linux")]
pub(crate) fn build_identity() -> crate::model::ToolIdentityV1 {
    use crate::{canonical, model::ToolIdentityV1};
    ToolIdentityV1 {
        package: "borrowser-host-lifecycle".into(),
        package_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: 1,
        source_revision: env!("LIFECYCLE_SOURCE_REVISION").into(),
        cargo_lock_sha256: canonical::sha256(include_bytes!("../Cargo.lock")),
        source_clean: env!("LIFECYCLE_SOURCE_CLEAN") == "true",
    }
}

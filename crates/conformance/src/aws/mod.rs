//! AG4's single disposable CloudFormation/SSH execution path.
#[cfg(all(test, feature = "aws-acceptance"))]
mod acceptance;
mod artifact;
mod cloudformation;
mod command;
mod config;
mod eice;
mod ssh;
mod state;
mod template;
#[cfg(test)]
mod tests;

use cloudformation::Aws;
use command::{Commands, Owner, Spec};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

fn flags(
    args: &[String],
    allowed: &[&str],
) -> Result<(BTreeMap<String, String>, Option<String>), String> {
    let mut options = BTreeMap::new();
    let mut test = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if allowed.contains(&arg.as_str()) {
            let value = iter.next().ok_or("missing option value")?;
            if value.starts_with("--") || options.insert(arg.clone(), value.clone()).is_some() {
                return Err("duplicate or missing option".into());
            }
        } else if arg.starts_with('-') || test.replace(arg.clone()).is_some() {
            return Err("unexpected AG4 argument".into());
        }
    }
    Ok((options, test))
}
fn required(options: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    options
        .get(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{key} is required"))
}

pub(crate) fn artifact_info(out: &mut impl Write) -> io::Result<u8> {
    let identity = match artifact::identity() {
        Ok(identity) => identity,
        Err(error) => {
            writeln!(io::stderr().lock(), "ERROR artifact.identity: {error}")?;
            return Ok(2);
        }
    };
    serde_json::to_writer(&mut *out, &identity)?;
    writeln!(out)?;
    Ok(0)
}
pub(crate) fn eice_proxy(args: &[String]) -> io::Result<u8> {
    match eice::proxy(args) {
        Ok(()) => unreachable!("successful exec never returns"),
        Err(e) => {
            // Helper validation errors contain no descriptor values/credentials.
            writeln!(io::stderr().lock(), "ERROR ag4.eice.proxy: {e}")?;
            Ok(2)
        }
    }
}
pub(crate) fn run(
    args: &[String],
    out: &mut impl Write,
    diagnostics: &mut impl Write,
) -> io::Result<u8> {
    let result = (|| -> Result<_, String> {
        let (options, id) = flags(args, &["--config", "--artifact-dir", "--state-dir"])?;
        let fixtures = crate::fixtures::select(id.as_deref())
            .map_err(|e| format!("{}: {}", e.code, e.detail))?;
        let inputs: Vec<_> = fixtures
            .iter()
            .filter_map(|f| match f.kind {
                crate::model::FixtureKind::Runnable { html, .. } => {
                    Some(crate::chromium::CaptureInput {
                        id: f.id,
                        source: f.source,
                        html,

                        request: f.request,
                    })
                }
                _ => None,
            })
            .collect();
        if inputs.is_empty() {
            return Ok((fixtures, vec![]));
        }
        let mut config = config::Config::load(&required(&options, "--config")?)?;
        let artifact = artifact::Prepared::load(&required(&options, "--artifact-dir")?)?;
        let state_root = required(&options, "--state-dir")?;
        let captures = capture_and_cleanup(
            &inputs,
            id.as_deref(),
            &mut config,
            &artifact,
            &state_root,
            diagnostics,
        )?;
        Ok((fixtures, captures))
    })();
    match result {
        Ok((fixtures, captures)) => {
            crate::comparison::publish_captured(&fixtures, captures, out, diagnostics)
        }
        Err(e) => {
            writeln!(diagnostics, "ERROR aws: {e}")?;
            Ok(2)
        }
    }
}
fn capture_and_cleanup(
    inputs: &[crate::chromium::CaptureInput<'_>],
    id: Option<&str>,
    config: &mut config::Config,
    artifact: &artifact::Prepared,
    state_root: &Path,
    diagnostics: &mut impl Write,
) -> Result<Vec<crate::chromium::CapturedFixture>, String> {
    let mut owner = Owner::enter()?;
    let environment = config::AwsEnvironment::load()?;
    config.tools(&mut owner, &environment)?;
    let runner: Value = template::json(include_bytes!("../../aws/runner.json"))?;
    if runner["image"].is_null() || runner["ubuntu_snapshot"].is_null() {
        return Err("reviewed literal AMI identity and Ubuntu snapshot must be pinned in runner.json before live execution".into());
    }
    verify_cdk_source(&config.cdk_app)?;
    let deadline = owner.elapsed() + Duration::from_secs(1800);
    // Shared physical identities are needed for synthesis. These authenticated
    // reads create no resources and require no durable recovery record.
    let mut aws = Aws {
        commands: &mut owner,
        executable: config.aws_cli.clone(),
        environment,
        deadline,
        cleanup: false,
    };
    aws.account(&config.account)?;
    preflight(&mut aws, config, &runner)?;
    let endpoint = eice::verify_shared(&mut aws, config)?;
    writeln!(
        diagnostics,
        "AG4 shared EICE identity and effective rules verified"
    )
    .map_err(|e| format!("preflight diagnostic write failed: {e}"))?;
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let run_id = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let binding = template::Binding {
        account: config.account.clone(),
        region: "eu-west-1".into(),
        stack_name: format!("borrowser-ag4-{run_id}"),
        role_arn: config.cloudformation_role_arn.clone(),
        tags: BTreeMap::from([
            ("borrowser:purpose".into(), "ag4".into()),
            ("borrowser:run-id".into(), run_id.clone()),
        ]),
        run_id,
        timeout_minutes: 10,
        disable_rollback: false,
        termination_protection: false,
    };
    let work = tempfile::Builder::new()
        .prefix("ag4-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    fs::set_permissions(work.path(), fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    // macOS TMPDIR may start with /var -> /private/var. Resolve our freshly
    // created private directory once in the parent; the helper follows no links.
    let work_path = work.path().canonicalize().map_err(|e| e.to_string())?;
    let key = ssh::key(aws.commands, config, &work_path)?;
    let request = json!({"binding":binding,"network":{"vpc_id":config.vpc_id,"subnet_id":config.subnet_id,
        "endpoint_sg_id":endpoint.endpoint_sg_id,"target_sg_id":endpoint.target_sg_id},
        "reviewed_image":runner["image"],"ubuntu_snapshot":runner["ubuntu_snapshot"],"ssh_public_key":String::from_utf8(config::read(&key.with_extension("pub"),4096)?).map_err(|e|e.to_string())?});
    let input = work_path.join("synth-input.json");
    fs::write(
        &input,
        serde_json::to_vec(&request).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut synth = Spec::new(
        &config.python,
        vec![
            config.cdk_app.as_os_str().to_owned(),
            "--input".into(),
            input.as_os_str().to_owned(),
            "--out".into(),
            work_path.join("cdk.out").into_os_string(),
        ],
    );
    synth.timeout = Duration::from_secs(60);
    synth.stdout_limit = 51200;
    synth.env = vec![
        ("JSII_NODE".into(), config.node.as_os_str().to_owned()),
        (
            "JSII_RUNTIME_PACKAGE_CACHE_ROOT".into(),
            work_path.join("jsii-cache").into_os_string(),
        ),
        ("AWS_EC2_METADATA_DISABLED".into(), "true".into()),
    ];
    let synthesized = aws.commands.run(synth).map_err(|e| e.to_string())?;
    if synthesized.code != 0 {
        return Err(format!(
            "CDK synthesis failed: {}",
            String::from_utf8_lossy(&synthesized.stderr)
        ));
    }
    let template = template::json(&synthesized.stdout)?;
    if template::binding(&template)? != binding {
        return Err("CDK operation binding mismatch".into());
    }
    eice::validate_live_template(&template, config, &endpoint)?;
    let template_path = work_path.join("template.json");
    fs::write(&template_path, &synthesized.stdout).map_err(|e| e.to_string())?;
    // All preparation is complete before identity is durably recorded or AWS creation is possible.
    prepare_state_root(state_root)?;
    let store = state::Store::open(&state_root.join(&binding.run_id), true)?;
    let mut record = state::Record::new(&binding, template::digest(&template)?);
    store.save(&record)?;
    writeln!(
        diagnostics,
        "AG4 operation {}\nRecovery: --aws-cleanup --operation {} --aws-cli {}",
        binding.run_id,
        store.path().display(),
        config.aws_cli.display()
    )
    .map_err(|e| e.to_string())?;
    let (primary, cleanup) = capture_then_cleanup(&mut aws, &mut record, &store, |aws, record| {
        aws.create(record, &store, &binding, &template_path)?;
        aws.deadline = aws.commands.elapsed() + Duration::from_secs(600);
        aws.wait_ready(record, &store)?;
        aws.deadline = deadline;
        let host = ssh::ready(aws, record, &store, &work_path, key, &endpoint)?;
        writeln!(
            diagnostics,
            "AG4 private target and console-bound host key verified for {}",
            host.target.instance_id
        )
        .map_err(|e| format!("readiness diagnostic write failed: {e}"))?;
        let bytes = ssh::capture(aws, config, &host, artifact, id, diagnostics)?;
        if aws.commands.elapsed() > deadline {
            return Err("capture work deadline exceeded".into());
        }
        let captures = crate::chromium::ingest::decode(bytes.as_slice(), inputs)?;
        record.capture = state::Capture::Validated;
        store.save(record)?;
        Ok(captures)
    });
    let primary = if primary.is_ok() && cleanup.is_ok() {
        aws.cleanup = false;
        aws.deadline = deadline;
        match eice::verify_shared(&mut aws, config) {
            Ok(after) if after == endpoint => primary,
            Ok(_) => Err("shared EICE resources changed during capture".into()),
            Err(e) => Err(format!(
                "post-cleanup shared identity verification failed: {e}"
            )),
        }
    } else {
        primary
    };
    let mut environment = aws.environment;
    let quiescent = owner.quiescent().is_ok();
    let commands = owner.finish(); // Restores runtime/signal ownership before comparison.
    if !quiescent {
        // A poisoned owner cannot authorize removal under a surviving helper.
        // This private orphan is never operation/recovery authority.
        let _ = work.keep();
        environment.retain_unresolved_home();
    }
    let primary = match (primary, commands.as_ref()) {
        (Ok(_), Err(error)) => Err(error.clone()),
        (result, _) => result,
    };
    let primary = primary.map_err(|e| environment.redact(&e));
    let cleanup = cleanup.map_err(|e| environment.redact(&e));
    let persisted = cloudformation::record_failure(
        &mut record,
        &store,
        primary.as_ref().err().map(String::as_str),
        cleanup.as_ref().err().map(String::as_str),
    );
    match (primary, cleanup, persisted, commands) {
        (Ok(captures), Ok(()), Ok(()), Ok(())) => Ok(captures),
        (p, c, s, o) => Err(format!(
            "primary={:?}; stack cleanup={:?}; persistence={:?}; controller cleanup={:?}; recover with --aws-cleanup --operation {} --aws-cli {}",
            p.err(),
            c.err(),
            s.err(),
            o.err(),
            store.path().display(),
            config.aws_cli.display()
        )),
    }
}
type CaptureResult = Result<Vec<crate::chromium::CapturedFixture>, String>;
// One present AG4 seam: every capture-stage outcome reaches the same cleanup
// path. Scripted tests exercise failures without launching any AWS resource.
fn capture_then_cleanup<C: Commands>(
    aws: &mut Aws<'_, C>,
    record: &mut state::Record,
    store: &state::Store,
    capture: impl FnOnce(&mut Aws<'_, C>, &mut state::Record) -> CaptureResult,
) -> (CaptureResult, Result<(), String>) {
    let primary = capture(aws, record);
    let cleanup = aws.cleanup(record, store);
    (primary, cleanup)
}
fn prepare_state_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() {
        return Err("absolute outside-repository state directory required".into());
    }
    if root.ancestors().any(|p| p.join(".git").exists()) {
        return Err("state must be outside repository".into());
    }
    if !root.exists() {
        fs::create_dir(root).map_err(|e| e.to_string())?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        fs::File::open(root.parent().ok_or("state root parent required")?)
            .and_then(|parent| parent.sync_all())
            .map_err(|e| e.to_string())?;
    }
    let meta = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || !meta.is_dir() || meta.permissions().mode() & 0o777 != 0o700
    {
        return Err("private mode-0700 state root required".into());
    }
    Ok(())
}
fn verify_cdk_source(app: &Path) -> Result<(), String> {
    if app.file_name().and_then(|name| name.to_str()) != Some("app.py") {
        return Err("the checked-in AG4 app.py is the only synthesis entry point".into());
    }
    let dir = app.parent().ok_or("missing CDK directory")?;
    for (name, expected) in [
        ("app.py", include_bytes!("../../aws/app.py").as_slice()),
        ("stack.py", include_bytes!("../../aws/stack.py").as_slice()),
        (
            "guest_capture.py",
            include_bytes!("../../aws/guest_capture.py").as_slice(),
        ),
        (
            "bootstrap.sh",
            include_bytes!("../../aws/bootstrap.sh").as_slice(),
        ),
        (
            "runner.json",
            include_bytes!("../../aws/runner.json").as_slice(),
        ),
        (
            "toolchain.json",
            include_bytes!("../../aws/toolchain.json").as_slice(),
        ),
        (
            "requirements.lock",
            include_bytes!("../../aws/requirements.lock").as_slice(),
        ),
    ] {
        if config::read(&dir.join(name), 256 * 1024)? != expected {
            return Err(format!(
                "CDK source does not match controller snapshot: {name}"
            ));
        }
    }
    if config::read(&dir.join("../chromium-reference.json"), 16384)?
        != include_bytes!("../../chromium-reference.json")
    {
        return Err("CDK Chromium pin differs from artifact".into());
    }
    Ok(())
}
fn preflight<C: Commands>(
    aws: &mut Aws<'_, C>,
    c: &config::Config,
    runner: &Value,
) -> Result<(), String> {
    // The reviewed c6i.large has no instance store. Accepting the two inert AMI
    // virtual declarations below must never enable an instance-store variant.
    if runner["region"] != "eu-west-1"
        || runner["instance_type"] != "c6i.large"
        || runner["architecture"] != "x86_64"
        || runner["root_gib"] != 16
    {
        return Err("reviewed EBS-only c6i.large launch configuration required".into());
    }
    let image = &runner["image"];
    let id = cloudformation::string(image, "id")?;
    let name = cloudformation::string(image, "name")?;
    let root_snapshot = cloudformation::string(image, "root_snapshot")?;
    let source_root_gib = image["source_root_gib"]
        .as_u64()
        .filter(|size| (1..=16).contains(size))
        .ok_or("reviewed source root size must fit the 16-GiB guest disk")?;
    if !name.starts_with("ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-")
        || !root_snapshot.starts_with("snap-")
    {
        return Err("reviewed Canonical Noble server image identity required".into());
    }
    let response = aws.call("ec2", "describe-images", &["--image-ids".into(), id.into()])?;
    let images = cloudformation::array(&response, "Images")?;
    if images.len() != 1 {
        return Err("pinned AMI unavailable".into());
    }
    let actual = &images[0];
    if actual["ImageId"] != id
        || actual["OwnerId"] != "099720109477"
        || actual["Architecture"] != "x86_64"
        || actual["State"] != "available"
        || actual["VirtualizationType"] != "hvm"
        || actual["RootDeviceType"] != "ebs"
        || actual["RootDeviceName"] != "/dev/sda1"
        || actual["Name"] != name
    {
        return Err("pinned AMI identity mismatch".into());
    }
    validate_ami_storage(actual, root_snapshot, source_root_gib)?;
    let response = aws.call(
        "ec2",
        "describe-subnets",
        &["--subnet-ids".into(), c.subnet_id.clone()],
    )?;
    let subnets = cloudformation::array(&response, "Subnets")?;
    if subnets.len() != 1
        || subnets[0]["SubnetId"] != c.subnet_id
        || subnets[0]["VpcId"] != c.vpc_id
        || subnets[0]["State"] != "available"
    {
        return Err("shared subnet identity mismatch".into());
    }
    Ok(())
}

fn validate_ami_storage(
    image: &Value,
    root_snapshot: &str,
    source_root_gib: u64,
) -> Result<(), String> {
    let mappings = cloudformation::array(image, "BlockDeviceMappings")?;
    let mut devices = std::collections::BTreeSet::new();
    for mapping in mappings {
        let fields = mapping.as_object().ok_or("malformed AMI storage mapping")?;
        let device = cloudformation::string(mapping, "DeviceName")?;
        if !devices.insert(device) {
            return Err("duplicate AMI storage device".into());
        }
        match device {
            "/dev/sda1" => {
                // Only an EBS root, never a mixed Ebs/VirtualName/NoDevice entry.
                if fields.len() != 2
                    || !mapping["Ebs"].is_object()
                    || mapping["Ebs"]["SnapshotId"] != root_snapshot
                    || mapping["Ebs"]["VolumeSize"].as_u64() != Some(source_root_gib)
                {
                    return Err("pinned AMI root storage mismatch".into());
                }
            }
            "/dev/sdb" | "/dev/sdc" => {
                // These exact reviewed pairs cannot materialize storage on our
                // fixed EBS-only type. Unknown fields/kinds/virtual IDs fail.
                let virtual_name = if device == "/dev/sdb" {
                    "ephemeral0"
                } else {
                    "ephemeral1"
                };
                if mapping != &json!({"DeviceName":device,"VirtualName":virtual_name}) {
                    return Err("unreviewed AMI virtual mapping".into());
                }
            }
            _ => return Err("unexpected AMI storage device".into()),
        }
    }
    if !devices.contains("/dev/sda1") {
        return Err("missing AMI EBS root".into());
    }
    Ok(())
}
pub(crate) fn cleanup(args: &[String], diagnostics: &mut impl Write) -> io::Result<u8> {
    let result: Result<(), String> = (|| {
        let (options, id) = flags(args, &["--operation", "--aws-cli"])?;
        if id.is_some() {
            return Err("cleanup accepts no fixture".into());
        }
        let executable = required(&options, "--aws-cli")?;
        if !executable.is_absolute() {
            return Err("absolute pinned AWS CLI path required".into());
        }
        let store = state::Store::open(&required(&options, "--operation")?, false)?;
        let mut record = store.read()?;
        let mut owner = Owner::enter()?;
        let environment = config::AwsEnvironment::load()?;
        let mut version_spec = Spec::new(&executable, ["--version"]);
        environment.apply(&mut version_spec);
        let version = owner.run(version_spec).map_err(|e| e.to_string())?;
        let pins: Value = template::json(include_bytes!("../../aws/toolchain.json"))?;
        if version.code != 0
            || !String::from_utf8_lossy(&version.stdout).starts_with(&format!(
                "aws-cli/{} ",
                pins["aws_cli"].as_str().ok_or("invalid CLI pin")?
            ))
        {
            return Err("AWS CLI pin mismatch".into());
        }
        let deadline = owner.elapsed() + Duration::from_secs(900);
        let result = Aws {
            commands: &mut owner,
            executable,
            environment,
            deadline,
            cleanup: true,
        }
        .cleanup(&mut record, &store);
        cloudformation::record_failure(
            &mut record,
            &store,
            None,
            result.as_ref().err().map(String::as_str),
        )?;
        result?;
        owner.finish()?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            writeln!(diagnostics, "AG4 cleanup confirmed")?;
            Ok(0)
        }
        Err(e) => {
            writeln!(diagnostics, "ERROR aws.cleanup: {e}")?;
            Ok(2)
        }
    }
}

//! CloudFormation is the sole mutating AWS API owner. EC2 calls only observe.
use super::{
    command::{Commands, Spec},
    state::{Capture, Cleanup, Creation, Record, Store},
    template::{self, Binding},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct Aws<'a, C: Commands> {
    pub commands: &'a mut C,
    pub executable: PathBuf,
    pub environment: super::config::AwsEnvironment,
    pub deadline: Duration,
    pub cleanup: bool,
}
enum Confirmation {
    Pending,
    Confirmed,
}
impl<C: Commands> Aws<'_, C> {
    pub fn call(&mut self, service: &str, action: &str, args: &[String]) -> Result<Value, String> {
        let mutation = matches!(
            (service, action),
            ("cloudformation", "create-stack" | "delete-stack")
        );
        if !matches!(
            (service, action),
            (
                "cloudformation",
                "create-stack"
                    | "delete-stack"
                    | "describe-stacks"
                    | "get-template"
                    | "list-stack-resources"
                    | "describe-stack-events"
            ) | ("sts", "get-caller-identity")
                | (
                    "ec2",
                    "describe-images"
                        | "describe-subnets"
                        | "describe-instances"
                        | "describe-volumes"
                        | "describe-instance-connect-endpoints"
                        | "describe-security-groups"
                        | "describe-network-interfaces"
                        | "get-console-output"
                )
        ) {
            return Err("unsupported AG4 AWS operation".into());
        }
        if service == "ec2" {
            let exact = match action {
                "describe-instance-connect-endpoints" => {
                    Some(("--instance-connect-endpoint-ids", "eice-"))
                }
                "describe-security-groups" => Some(("--group-ids", "sg-")),
                "describe-network-interfaces" => Some(("--network-interface-ids", "eni-")),
                _ => None,
            };
            if let Some((selector, prefix)) = exact
                && (args.len() != 2
                    || args[0] != selector
                    || !args[1].strip_prefix(prefix).is_some_and(|id| {
                        id.len() == 17
                            && id
                                .bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    }))
            {
                return Err("exact reviewed EC2 transport resource ID required".into());
            }
            let selector = match action {
                "describe-images" => "--image-ids",
                "describe-subnets" => "--subnet-ids",
                "get-console-output" => "--instance-id",
                "describe-instance-connect-endpoints" => "--instance-connect-endpoint-ids",
                "describe-security-groups" => "--group-ids",
                "describe-network-interfaces" => "--network-interface-ids",
                _ => "--filters",
            };
            if !args.iter().any(|arg| arg == selector) {
                return Err("targeted EC2 selector required".into());
            }
        }
        let mut last = "AWS attempt unavailable".to_string();
        for attempt in 0..if mutation { 1 } else { 3 } {
            if self.commands.interrupted(self.cleanup) {
                return Err("operation interrupted".into());
            }
            let remaining = self.deadline.saturating_sub(self.commands.elapsed());
            if remaining.is_zero() {
                return Err("AWS phase deadline exceeded".into());
            }
            let mut spec = Spec::new(
                &self.executable,
                [
                    service,
                    action,
                    "--region",
                    "eu-west-1",
                    "--output",
                    "json",
                    "--no-cli-pager",
                    "--no-cli-auto-prompt",
                    "--no-paginate",
                    "--cli-connect-timeout",
                    "5",
                    "--cli-read-timeout",
                    "15",
                ],
            );
            spec.args.extend(args.iter().map(Into::into));
            spec.cleanup = self.cleanup;
            spec.timeout = remaining.min(Duration::from_secs(20));
            self.environment.apply(&mut spec);
            match self.commands.run(spec) {
                Ok(out) if out.code == 0 => {
                    let value = if action == "delete-stack"
                        && out.stdout.iter().all(u8::is_ascii_whitespace)
                    {
                        json!({})
                    } else {
                        template::json(&out.stdout)?
                    };
                    complete(&value)?;
                    return Ok(value);
                }
                Ok(out) => {
                    last = format!(
                        "{service}.{action} exited {}: {}",
                        out.code,
                        self.environment
                            .redact(&String::from_utf8_lossy(&out.stderr))
                            .chars()
                            .take(1024)
                            .collect::<String>()
                    );
                }
                Err(e) => {
                    if e.cleanup.is_some() {
                        return Err(e.to_string());
                    }
                    last = e.to_string();
                }
            }
            if attempt < 2 && !mutation {
                self.pause(Duration::from_secs(1 << attempt))?;
            }
        }
        Err(last)
    }
    pub fn pause(&mut self, duration: Duration) -> Result<(), String> {
        let remaining = self.deadline.saturating_sub(self.commands.elapsed());
        if remaining.is_zero() {
            return Err("AWS phase deadline exceeded".into());
        }
        self.commands.pause(duration.min(remaining), self.cleanup)
    }
    pub fn account(&mut self, expected: &str) -> Result<(), String> {
        if self.call("sts", "get-caller-identity", &[])?["Account"] != expected {
            return Err("AWS account mismatch".into());
        }
        Ok(())
    }
    pub fn describe(&mut self, r: &Record) -> Result<Value, String> {
        let name = r.stack_id.as_deref().unwrap_or(&r.stack_name);
        let v = self.call(
            "cloudformation",
            "describe-stacks",
            &["--stack-name".into(), name.into()],
        )?;
        let stacks = array(&v, "Stacks")?;
        if stacks.len() != 1 {
            return Err("missing or ambiguous operation stack".into());
        }
        Ok(stacks[0].clone())
    }
    pub fn authorize(&mut self, r: &mut Record, store: &Store) -> Result<Value, String> {
        let stack = self.describe(r)?;
        let id = string(&stack, "StackId")?;
        r.check_stack_id(id)?;
        let envelope = self.call(
            "cloudformation",
            "get-template",
            &[
                "--stack-name".into(),
                id.into(),
                "--template-stage".into(),
                "Original".into(),
            ],
        )?;
        let original =
            template::original(&serde_json::to_vec(&envelope).map_err(|e| e.to_string())?)?;
        if template::digest(&original)? != r.template_sha256 {
            return Err("deployed template identity mismatch".into());
        }
        let b = template::binding(&original)?;
        verify_binding(r, &b, &stack)?;
        if r.stack_id.is_none() {
            r.stack_id = Some(id.into());
            r.creation = Creation::Bound;
            store.save(r)?;
        }
        Ok(stack)
    }
    pub fn create(
        &mut self,
        r: &mut Record,
        store: &Store,
        b: &Binding,
        template_path: &Path,
    ) -> Result<(), String> {
        if r.creation != Creation::Prepared || r.stack_id.is_some() {
            return Err("creation cannot be retried or recovered".into());
        }
        r.creation = Creation::Submitted;
        store.save(r)?; // Durable before the request can leave this process.
        let tags: Vec<_> = b
            .tags
            .iter()
            .map(|(k, v)| json!({"Key":k,"Value":v}))
            .collect();
        let response = self.call(
            "cloudformation",
            "create-stack",
            &[
                "--stack-name".into(),
                r.stack_name.clone(),
                "--template-body".into(),
                format!("file://{}", template_path.display()),
                "--client-request-token".into(),
                format!("ag4-create-{}", r.run_id),
                "--role-arn".into(),
                b.role_arn.clone(),
                "--tags".into(),
                serde_json::to_string(&tags).map_err(|e| e.to_string())?,
                "--timeout-in-minutes".into(),
                "10".into(),
                "--no-disable-rollback".into(),
                "--no-enable-termination-protection".into(),
            ],
        )?;
        #[cfg(feature = "aws-acceptance")]
        if std::env::var_os("BORROWSER_AG4_ACCEPTED_CREATE_CRASH").is_some() {
            // Acceptance-only: the actual successful response exists, but neither
            // this function nor the durable operation record has consumed its ARN.
            unsafe { libc::_exit(86) };
        }
        let id = string(&response, "StackId")?;
        r.check_stack_id(id)?;
        r.stack_id = Some(id.into());
        r.creation = Creation::Bound;
        store.save(r)?;
        #[cfg(feature = "aws-acceptance")]
        if std::env::var_os("BORROWSER_AG4_CANCEL_AFTER_CREATE").is_some() {
            unsafe { libc::kill(libc::getpid(), libc::SIGINT) };
        }
        Ok(())
    }
    pub fn wait_ready(&mut self, r: &mut Record, store: &Store) -> Result<(), String> {
        for _ in 0..120 {
            let stack = self.authorize(r, store)?;
            match string(&stack, "StackStatus")? {
                "CREATE_COMPLETE" => {
                    self.resources(r, store)?;
                    return Ok(());
                }
                "CREATE_IN_PROGRESS" => self.pause(Duration::from_secs(5))?,
                status => return Err(format!("deployment ended in {status}")),
            }
        }
        Err("deployment polling exhausted".into())
    }
    pub fn resources(&mut self, r: &mut Record, store: &Store) -> Result<Vec<Value>, String> {
        let v = self.call(
            "cloudformation",
            "list-stack-resources",
            &[
                "--stack-name".into(),
                r.stack_id.clone().ok_or("unbound stack")?,
            ],
        )?;
        let rows = array(&v, "StackResourceSummaries")?;
        if rows.len() > 2 {
            return Err("unexpected stack resources".into());
        }
        let mut seen = vec![];
        for row in rows {
            let name = string(row, "LogicalResourceId")?;
            if seen.contains(&name) {
                return Err("duplicate stack resource".into());
            }
            seen.push(name);
            let expected = match name {
                "Runner" => "AWS::EC2::Instance",
                "RunnerSecurityGroup" => "AWS::EC2::SecurityGroup",
                _ => return Err("unrelated stack resource".into()),
            };
            if string(row, "ResourceType")? != expected {
                return Err("conflicting stack resource type".into());
            }
            if name == "Runner"
                && let Some(id) = row.get("PhysicalResourceId")
            {
                let id = id.as_str().ok_or("invalid instance id")?;
                if !id.starts_with("i-") || r.instance_id.as_ref().is_some_and(|known| known != id)
                {
                    return Err("conflicting instance identity".into());
                }
                if r.instance_id.is_none() {
                    r.instance_id = Some(id.into());
                    store.save(r)?;
                }
            }
        }
        if r.instance_id.is_some()
            && (!seen.contains(&"Runner") || !seen.contains(&"RunnerSecurityGroup"))
        {
            return Err("materialized instance missing from complete stack records".into());
        }
        Ok(rows.clone())
    }
    pub fn instance(&mut self, r: &mut Record, store: &Store) -> Result<Option<Value>, String> {
        let id = r
            .instance_id
            .clone()
            .ok_or("instance identity unavailable")?;
        let v = self.call(
            "ec2",
            "describe-instances",
            &["--filters".into(), format!("Name=instance-id,Values={id}")],
        )?;
        let mut instances = vec![];
        for reservation in array(&v, "Reservations")? {
            instances.extend(array(reservation, "Instances")?.iter());
        }
        if instances.len() > 1 {
            return Err("ambiguous instance response".into());
        }
        let Some(instance) = instances.first() else {
            return Ok(None);
        };
        if instance["InstanceId"] != id {
            return Err("unrelated instance response".into());
        }
        instance_state(instance)?;
        if instance["State"]["Name"] != "terminated" {
            verify_resource_tags(instance, r)?;
            let root = string(instance, "RootDeviceName")?;
            let mappings = array(instance, "BlockDeviceMappings")?;
            if mappings.len() != 1
                || mappings[0]["DeviceName"] != root
                || mappings[0]["Ebs"]["DeleteOnTermination"] != true
            {
                return Err("unexpected root storage mapping".into());
            }
            let volume = string(&mappings[0]["Ebs"], "VolumeId")?;
            if r.root_volume_id
                .as_ref()
                .is_some_and(|known| known != volume)
            {
                return Err("conflicting root volume identity".into());
            }
            if r.root_volume_id.is_none() {
                r.root_volume_id = Some(volume.into());
                store.save(r)?;
            }
        }
        Ok(Some((*instance).clone()))
    }
    pub fn cleanup(&mut self, r: &mut Record, store: &Store) -> Result<(), String> {
        self.cleanup = true;
        self.deadline = self.commands.elapsed() + Duration::from_secs(900);
        if r.creation == Creation::Prepared && r.stack_id.is_none() {
            r.cleanup = Cleanup::NotRequired;
            store.save(r)?;
            return Ok(());
        }
        self.account(&r.account)?;
        // A lost create response allows only bounded lookup, never CreateStack.
        let cleanup_deadline = self.deadline;
        if r.stack_id.is_none() {
            self.deadline = self
                .deadline
                .min(self.commands.elapsed() + Duration::from_secs(300));
        }
        let mut authorized = None;
        for _ in 0..60 {
            match self.authorize(r, store) {
                Ok(s) => {
                    authorized = Some(s);
                    break;
                }
                Err(e) => {
                    if r.stack_id.is_some() {
                        return Err(e);
                    }
                    self.pause(Duration::from_secs(5))?;
                }
            }
        }
        let mut stack =
            authorized.ok_or("creation outcome unresolved after bounded exact-name lookup")?;
        self.deadline = cleanup_deadline;
        // A later cleanup invocation must be able to retry a previous DELETE_FAILED
        // without reusing that invocation's client tokens. This nonce is not identity.
        let mut nonce = [0_u8; 8];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let nonce = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let mut deletes = 0;
        for _ in 0..180 {
            let status = string(&stack, "StackStatus")?;
            if status == "DELETE_COMPLETE" {
                match self.confirm(r, store)? {
                    Confirmation::Confirmed => {
                        r.cleanup = Cleanup::Confirmed;
                        r.cleanup_failure = None;
                        store.save(r)?;
                        return Ok(());
                    }
                    Confirmation::Pending => {
                        self.pause(Duration::from_secs(5))?;
                        stack = self.authorize(r, store)?;
                        continue;
                    }
                }
            }
            if !matches!(
                status,
                "DELETE_IN_PROGRESS" | "CREATE_IN_PROGRESS" | "ROLLBACK_IN_PROGRESS"
            ) {
                if !matches!(
                    status,
                    "CREATE_COMPLETE"
                        | "CREATE_FAILED"
                        | "ROLLBACK_COMPLETE"
                        | "ROLLBACK_FAILED"
                        | "DELETE_FAILED"
                ) {
                    return Err("unexpected stack lifecycle state; deletion refused".into());
                }
                if deletes >= 3 {
                    return Err("stack deletion attempts exhausted".into());
                }
                // Ownership authorization and resource discovery are distinct from confirmation.
                self.resources(r, store)?;
                // A deleted stack cannot subsequently be looked up by name. Keep
                // its verified ARN durable before deletion, including when a prior
                // write after CreateStack failed. On a continuing storage failure,
                // leave explicit recoverable infrastructure rather than lose its ID.
                store
                    .save(r)
                    .map_err(|e| format!("cannot durably bind stack before deletion: {e}"))?;
                let result = self.call(
                    "cloudformation",
                    "delete-stack",
                    &[
                        "--stack-name".into(),
                        r.stack_id.clone().ok_or("unbound stack")?,
                        "--client-request-token".into(),
                        format!("ag4-delete-{}-{nonce}-{deletes}", r.run_id),
                    ],
                );
                deletes += 1;
                #[cfg(feature = "aws-acceptance")]
                if result.is_ok()
                    && std::env::var_os("BORROWSER_AG4_ACCEPTED_DELETE_CRASH").is_some()
                {
                    unsafe { libc::_exit(87) };
                }
                if let Err(e) = result {
                    r.cleanup_failure = Some(e.chars().take(1024).collect());
                    store.save(r)?;
                }
            }
            self.pause(Duration::from_secs(5))?;
            stack = self.authorize(r, store)?;
        }
        Err("cleanup polling exhausted".into())
    }
    fn confirm(&mut self, r: &mut Record, store: &Store) -> Result<Confirmation, String> {
        let mut pending = false;
        let rows = self.resources(r, store)?;
        for row in &rows {
            let status = string(row, "ResourceStatus")?;
            if status != "DELETE_COMPLETE"
                && !(status == "CREATE_FAILED" && row.get("PhysicalResourceId").is_none())
            {
                return Err("stack resource disposal unresolved".into());
            }
        }
        if r.instance_id.is_some() {
            if let Some(i) = self.instance(r, store)?
                && i["State"]["Name"] != "terminated"
            {
                pending = true;
            }
        } else {
            // Missing IDs are not proof of non-creation. Require the complete
            // operation history and terminal CloudFormation handling first.
            let history = self.call(
                "cloudformation",
                "describe-stack-events",
                &[
                    "--stack-name".into(),
                    r.stack_id.clone().ok_or("unbound stack")?,
                ],
            )?;
            let events = array(&history, "StackEvents")?;
            if !events.iter().any(|e| {
                e["ResourceType"] == "AWS::CloudFormation::Stack"
                    && e["ResourceStatus"] == "CREATE_IN_PROGRESS"
                    && e["ClientRequestToken"] == format!("ag4-create-{}", r.run_id)
            }) {
                return Err("incomplete creation history".into());
            }
            if events.iter().any(|e| {
                e["ResourceType"] == "AWS::EC2::Instance"
                    && e.get("PhysicalResourceId")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty())
            }) {
                return Err("instance identity omitted from resource records".into());
            }
            let v = self.call("ec2", "describe-instances", &tag_filters(r)?)?;
            let mut count = 0;
            for reservation in array(&v, "Reservations")? {
                for i in array(reservation, "Instances")? {
                    count += 1;
                    if count > 1 || !string(i, "InstanceId")?.starts_with("i-") {
                        return Err("ambiguous scoped instance observation".into());
                    }
                    verify_resource_tags(i, r)?;
                    if instance_state(i)? != "terminated" {
                        pending = true;
                    }
                }
            }
        }
        let filters = if let Some(id) = &r.root_volume_id {
            vec!["--filters".into(), format!("Name=volume-id,Values={id}")]
        } else {
            tag_filters(r)?
        };
        let v = self.call("ec2", "describe-volumes", &filters)?;
        let volumes = array(&v, "Volumes")?;
        if volumes.len() > 1 {
            return Err("ambiguous root volume observation".into());
        }
        for volume in volumes {
            let id = string(volume, "VolumeId")?;
            if !id.starts_with("vol-")
                || r.root_volume_id.as_deref().is_some_and(|known| known != id)
            {
                return Err("conflicting root volume observation".into());
            }
            verify_resource_tags(volume, r)?;
            if !matches!(
                string(volume, "State")?,
                "creating" | "available" | "in-use" | "deleting" | "deleted" | "error"
            ) {
                return Err("invalid root volume state".into());
            }
            pending = true;
        }
        Ok(if pending {
            Confirmation::Pending
        } else {
            Confirmation::Confirmed
        })
    }
}
fn instance_state(instance: &Value) -> Result<&str, String> {
    let state = string(&instance["State"], "Name")?;
    if !matches!(
        state,
        "pending" | "running" | "stopping" | "stopped" | "shutting-down" | "terminated"
    ) {
        return Err("invalid instance state".into());
    }
    Ok(state)
}
fn tag_filters(r: &Record) -> Result<Vec<String>, String> {
    Ok(vec![
        "--filters".into(),
        format!("Name=tag:borrowser:run-id,Values={}", r.run_id),
        format!(
            "Name=tag:borrowser:stack-id,Values={}",
            r.stack_id.as_deref().ok_or("unbound stack")?
        ),
        "Name=tag:borrowser:purpose,Values=ag4".into(),
    ])
}
pub(super) fn complete(v: &Value) -> Result<(), String> {
    for key in ["NextToken", "NextMarker", "Marker"] {
        if v.get(key).is_some_and(|v| !v.is_null()) {
            return Err("unexpected API continuation; observation incomplete".into());
        }
    }
    Ok(())
}
pub(super) fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    v.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array {key}"))
}
pub(super) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing string {key}"))
}
fn tags(v: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut tags = BTreeMap::new();
    for t in array(v, "Tags")? {
        if tags
            .insert(string(t, "Key")?.into(), string(t, "Value")?.into())
            .is_some()
        {
            return Err("duplicate ownership tag".into());
        }
    }
    Ok(tags)
}
fn verify_resource_tags(v: &Value, r: &Record) -> Result<(), String> {
    let tags = tags(v)?;
    for (key, value) in [
        ("borrowser:run-id", r.run_id.as_str()),
        (
            "borrowser:stack-id",
            r.stack_id.as_deref().ok_or("unbound stack")?,
        ),
        ("borrowser:purpose", "ag4"),
    ] {
        if tags.get(key).map(String::as_str) != Some(value) {
            return Err("resource ownership mismatch".into());
        }
    }
    Ok(())
}
fn verify_binding(r: &Record, b: &Binding, stack: &Value) -> Result<(), String> {
    // Deleted-stack responses can omit this field. Deletion cannot complete
    // with protection enabled; absence is compatible only with this exact
    // terminal lifecycle state and an explicitly unprotected binding.
    let termination_protection_matches = stack.get("EnableTerminationProtection")
        == Some(&json!(b.termination_protection))
        || (!b.termination_protection
            && stack["StackStatus"] == "DELETE_COMPLETE"
            && stack.get("EnableTerminationProtection").is_none());
    if b.run_id != r.run_id
        || b.account != r.account
        || b.region != r.region
        || b.stack_name != r.stack_name
        || stack["StackName"] != b.stack_name
        || stack["RoleARN"] != b.role_arn
        || tags(stack)? != b.tags
        || stack["DisableRollback"] != b.disable_rollback
        || !termination_protection_matches
        || stack["TimeoutInMinutes"] != b.timeout_minutes
        || stack
            .get("Parameters")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err("stack operation binding mismatch".into());
    }
    Ok(())
}
pub(super) fn record_failure(
    r: &mut Record,
    store: &Store,
    primary: Option<&str>,
    cleanup: Option<&str>,
) -> Result<(), String> {
    if let Some(p) = primary {
        r.primary_failure = Some(p.chars().take(1024).collect());
        r.capture = Capture::Failed;
    }
    if let Some(c) = cleanup {
        r.cleanup_failure = Some(c.chars().take(1024).collect());
        r.cleanup = Cleanup::Pending;
    }
    store.save(r)
}

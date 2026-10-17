use super::*;
use command::{Commands, Failure, Output, Spec};
use state::{Cleanup, Creation, Record, Store};
use std::collections::VecDeque;

fn template() -> Value {
    template::json(include_bytes!("../../aws/tests/synthesized.json")).unwrap()
}
pub(super) fn stack_id() -> String {
    "arn:aws:cloudformation:eu-west-1:123456789012:stack/borrowser-ag4-0123456789abcdef0123456789abcdef/01234567-89ab-cdef-0123-456789abcdef".into()
}
pub(super) fn record() -> Record {
    let t = template();
    Record::new(
        &template::binding(&t).unwrap(),
        template::digest(&t).unwrap(),
    )
}
pub(super) fn store() -> (tempfile::TempDir, Store) {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(&d.path().join("operation"), true).unwrap();
    (d, s)
}
pub(super) fn stack(status: &str) -> Value {
    let b = template::binding(&template()).unwrap();
    json!({"Stacks":[{"StackId":stack_id(),"StackName":b.stack_name,"StackStatus":status,"RoleARN":b.role_arn,
    "DisableRollback":false,"EnableTerminationProtection":false,"TimeoutInMinutes":10,
    "Tags":b.tags.iter().map(|(k,v)|json!({"Key":k,"Value":v})).collect::<Vec<_>>()}]})
}
pub(super) fn resources(status: &str, instance: bool) -> Value {
    let mut rows = vec![
        json!({"LogicalResourceId":"RunnerSecurityGroup","ResourceType":"AWS::EC2::SecurityGroup","ResourceStatus":status,"PhysicalResourceId":"sg-0123456789abcdef0"}),
    ];
    if instance {
        rows.push(json!({"LogicalResourceId":"Runner","ResourceType":"AWS::EC2::Instance","ResourceStatus":status,"PhysicalResourceId":"i-0123456789abcdef0"}));
    }
    json!({"StackResourceSummaries":rows})
}
#[derive(Default)]
pub(super) struct Script {
    pub(super) steps: VecDeque<(&'static str, Result<Value, String>)>,
    pub(super) seen: Vec<String>,
    now: Duration,
    persisted_before_create: Option<PathBuf>,
    fail_persistence_after_create: Option<PathBuf>,
    pub(super) arguments: Vec<Vec<String>>,
    cancel_count: usize,
}
impl Script {
    pub(super) fn push(&mut self, action: &'static str, value: Value) {
        self.steps.push_back((action, Ok(value)));
    }
    fn verify(&mut self, status: &str) {
        self.push("describe-stacks", stack(status));
        self.push("get-template", json!({"TemplateBody":template()}));
    }
    pub(super) fn ready_cleanup(&mut self) {
        self.push("get-caller-identity", json!({"Account":"123456789012"}));
        self.verify("CREATE_COMPLETE");
        self.push("list-stack-resources", resources("CREATE_COMPLETE", true));
        self.push("delete-stack", json!({}));
        self.verify("DELETE_COMPLETE");
        self.push("list-stack-resources", resources("DELETE_COMPLETE", true));
        self.push("describe-instances", json!({"Reservations":[]}));
        self.push("describe-volumes", json!({"Volumes":[]}));
    }
}
impl Commands for Script {
    fn run(&mut self, spec: Spec) -> Result<Output, Failure> {
        assert!(spec.args.iter().any(|s| s == "--no-cli-pager"));
        assert!(spec.args.iter().any(|s| s == "--no-paginate"));
        assert!(spec.timeout <= Duration::from_secs(20));
        let action = spec.args[1].to_string_lossy().to_string();
        self.seen.push(action.clone());
        self.arguments.push(
            spec.args
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect(),
        );
        if action == "create-stack"
            && let Some(p) = &self.persisted_before_create
        {
            let r: Record = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
            assert_eq!(r.creation, Creation::Submitted);
            assert!(r.stack_id.is_none());
        }
        if action == "create-stack"
            && let Some(path) = self.fail_persistence_after_create.take()
        {
            fs::rename(&path, path.with_extension("saved")).unwrap();
            fs::create_dir(path).unwrap();
        }
        let (expected, result) = self.steps.pop_front().expect("unexpected AWS call");
        assert_eq!(action, expected);
        result
            .map(|v| Output {
                stdout: serde_json::to_vec(&v).unwrap(),
                stderr: vec![],
                code: 0,
            })
            .map_err(|e| Failure {
                primary: e,
                cleanup: None,
            })
    }
    fn elapsed(&self) -> Duration {
        self.now
    }
    fn pause(&mut self, d: Duration, cleanup: bool) -> Result<(), String> {
        if self.interrupted(cleanup) {
            return Err("interrupted".into());
        }
        self.now += d;
        Ok(())
    }
    fn interrupted(&self, cleanup: bool) -> bool {
        self.cancel_count >= if cleanup { 2 } else { 1 }
    }
}
pub(super) fn aws(s: &mut Script) -> Aws<'_, Script> {
    Aws {
        commands: s,
        executable: "/test/aws".into(),
        environment: config::AwsEnvironment::from_lookup(|key| match key {
            "AWS_ACCESS_KEY_ID" | "AWS_SECRET_ACCESS_KEY" => Some("synthetic-test-value".into()),
            _ => None,
        })
        .unwrap(),
        deadline: Duration::from_secs(1800),
        cleanup: false,
    }
}

fn reviewed_runner() -> Value {
    template::json(include_bytes!("../../aws/runner.json")).unwrap()
}

fn recorded_image() -> Value {
    // Relevant fields from the 2026-10-14 DescribeImages observation. This is a
    // credential-free regression fixture, not a new live observation.
    json!({
        "ImageId":"ami-00bf3d24573e7276a", "OwnerId":"099720109477",
        "Name":"ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-20260923",
        "State":"available", "Architecture":"x86_64", "VirtualizationType":"hvm",
        "RootDeviceType":"ebs", "RootDeviceName":"/dev/sda1",
        "BlockDeviceMappings":[
            {"DeviceName":"/dev/sda1", "Ebs":{"SnapshotId":"snap-0f8dbea1d46057399",
                "VolumeSize":8, "VolumeType":"gp3", "Encrypted":false, "DeleteOnTermination":true}},
            {"DeviceName":"/dev/sdb", "VirtualName":"ephemeral0"},
            {"DeviceName":"/dev/sdc", "VirtualName":"ephemeral1"}
        ]
    })
}

fn check_preflight(image: Value, runner: &Value) -> (Result<(), String>, Script) {
    let mut config = template::json(include_bytes!("../../aws/operator.example.json")).unwrap();
    config["vpc_id"] = json!("vpc-0123456789abcdef0");
    config["subnet_id"] = json!("subnet-0123456789abcdef0");
    let config: config::Config = serde_json::from_value(config).unwrap();
    let mut script = Script::default();
    script.push("describe-images", json!({"Images":[image]}));
    script.push(
        "describe-subnets",
        json!({"Subnets":[{"SubnetId":config.subnet_id,
        "VpcId":config.vpc_id,"State":"available"}]}),
    );
    let result = preflight(&mut aws(&mut script), &config, runner);
    (result, script)
}

#[test]
fn preflight_accepts_reviewed_ebs_only_image_declarations_in_any_order() {
    let original = recorded_image();
    for order in [
        vec![0, 1, 2],
        vec![0, 2, 1],
        vec![1, 0, 2],
        vec![1, 2, 0],
        vec![2, 0, 1],
        vec![2, 1, 0],
        vec![0],
        vec![1, 0],
        vec![2, 0],
    ] {
        let mut image = original.clone();
        image["BlockDeviceMappings"] = Value::Array(
            order
                .iter()
                .map(|i| original["BlockDeviceMappings"][*i].clone())
                .collect(),
        );
        let (result, script) = check_preflight(image, &reviewed_runner());
        assert!(result.is_ok(), "{order:?}: {result:?}");
        assert_eq!(script.seen, ["describe-images", "describe-subnets"]);
        assert!(script.steps.is_empty());
        assert!(
            script.arguments[0]
                .windows(2)
                .any(|args| args == ["--image-ids", "ami-00bf3d24573e7276a"])
        );
    }
}

#[test]
fn preflight_rejects_extra_ambiguous_or_malformed_storage() {
    let image = recorded_image();
    let m = image["BlockDeviceMappings"].as_array().unwrap();
    let root = m[0].clone();
    let virtual0 = m[1].clone();
    let virtual1 = m[2].clone();
    let mut cases = vec![
        ("missing root", json!([virtual0, virtual1])),
        ("empty mappings", json!([])),
        ("nonarray mappings", json!({})),
        ("duplicate root", json!([root, root])),
        (
            "extra EBS",
            json!([root, {"DeviceName":"/dev/sdd","Ebs":{"VolumeSize":8}}]),
        ),
        (
            "EBS on virtual device",
            json!([root, {"DeviceName":"/dev/sdb","Ebs":{"VolumeSize":8}}]),
        ),
        ("duplicate device", json!([root, virtual0, virtual0])),
        (
            "duplicate virtual identity",
            json!([root, virtual0, {"DeviceName":"/dev/sdc","VirtualName":"ephemeral0"}]),
        ),
        (
            "unknown device",
            json!([root, {"DeviceName":"/dev/sdd","VirtualName":"ephemeral2"}]),
        ),
        (
            "wrong virtual pair",
            json!([root, {"DeviceName":"/dev/sdb","VirtualName":"ephemeral1"}]),
        ),
        ("malformed entry", json!([root, null])),
        (
            "missing device",
            json!([root, {"VirtualName":"ephemeral0"}]),
        ),
        (
            "missing virtual identity",
            json!([root, {"DeviceName":"/dev/sdb"}]),
        ),
        (
            "malformed virtual identity",
            json!([root, {"DeviceName":"/dev/sdb","VirtualName":0}]),
        ),
    ];
    for (label, key, value) in [
        ("virtual plus EBS", "Ebs", json!({"VolumeSize":8})),
        ("virtual plus null EBS", "Ebs", Value::Null),
        ("virtual plus NoDevice", "NoDevice", json!("")),
        ("unknown ancillary field", "Unknown", json!(true)),
    ] {
        let mut virtual_mapping = virtual0.clone();
        virtual_mapping[key] = value;
        cases.push((label, json!([root, virtual_mapping])));
    }
    for (label, key, value) in [
        ("root plus virtual", "VirtualName", json!("ephemeral0")),
        ("root plus NoDevice", "NoDevice", json!("")),
        ("malformed root EBS", "Ebs", json!("invalid")),
        ("unknown root field", "Unknown", json!(true)),
    ] {
        let mut changed = root.clone();
        changed[key] = value;
        cases.push((label, json!([changed, virtual0, virtual1])));
    }
    for size in [
        json!(0),
        json!(17),
        json!(7),
        json!(-1),
        json!("8"),
        Value::Null,
    ] {
        let mut changed = root.clone();
        changed["Ebs"]["VolumeSize"] = size;
        cases.push((
            "wrong or out-of-bounds root size",
            json!([changed, virtual0, virtual1]),
        ));
    }
    let mut wrong_snapshot = root.clone();
    wrong_snapshot["Ebs"]["SnapshotId"] = json!("snap-0123456789abcdef0");
    cases.push((
        "wrong snapshot",
        json!([wrong_snapshot, virtual0, virtual1]),
    ));
    for (label, mappings) in cases {
        let mut changed = image.clone();
        changed["BlockDeviceMappings"] = mappings;
        let (result, script) = check_preflight(changed, &reviewed_runner());
        assert!(result.is_err(), "accepted {label}");
        assert_eq!(script.seen, ["describe-images"], "{label}");
    }
}

#[test]
fn preflight_preserves_identity_and_fixed_instance_type_checks() {
    for (key, value) in [
        ("ImageId", "ami-0123456789abcdef0"),
        ("OwnerId", "123456789012"),
        ("Name", "another-image"),
        ("State", "pending"),
        ("Architecture", "arm64"),
        ("VirtualizationType", "paravirtual"),
        ("RootDeviceType", "instance-store"),
        ("RootDeviceName", "/dev/sdb"),
    ] {
        let mut image = recorded_image();
        image[key] = json!(value);
        let (result, script) = check_preflight(image, &reviewed_runner());
        assert!(result.is_err(), "accepted {key}");
        assert_eq!(script.seen, ["describe-images"]);
    }
    for (pointer, value) in [
        ("/instance_type", json!("c6id.large")),
        ("/region", json!("eu-central-1")),
        ("/architecture", json!("arm64")),
        ("/root_gib", json!(32)),
        ("/image/source_root_gib", json!(0)),
        ("/image/source_root_gib", json!(17)),
        ("/image/source_root_gib", Value::Null),
    ] {
        let mut runner = reviewed_runner();
        *runner.pointer_mut(pointer).unwrap() = value;
        let (result, script) = check_preflight(recorded_image(), &runner);
        assert!(result.is_err(), "accepted {pointer}");
        assert!(script.seen.is_empty());
    }
}

#[test]
fn launched_instance_still_rejects_extra_or_retained_storage() {
    let root = json!({"DeviceName":"/dev/sda1","Ebs":{
        "VolumeId":"vol-0123456789abcdef0","DeleteOnTermination":true}});
    let mut retained = root.clone();
    retained["Ebs"]["DeleteOnTermination"] = json!(false);
    for mappings in [
        json!([root, {"DeviceName":"/dev/sdb","Ebs":{"VolumeId":"vol-0123456789abcdef1","DeleteOnTermination":true}}]),
        json!([root, {"DeviceName":"/dev/sdb","VirtualName":"ephemeral0"}]),
        json!([retained]),
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.instance_id = Some("i-0123456789abcdef0".into());
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut script = Script::default();
        script.push(
            "describe-instances",
            json!({"Reservations":[{"Instances":[{
            "InstanceId":r.instance_id,"State":{"Name":"running"},"Tags":owned_tags(&r),
            "RootDeviceName":"/dev/sda1","BlockDeviceMappings":mappings}]}]}),
        );
        assert!(aws(&mut script).instance(&mut r, &store).is_err());
        assert!(store.read().unwrap().root_volume_id.is_none());
        assert_eq!(script.seen, ["describe-instances"]);
    }
}

#[test]
fn template_mutations_never_authorize_deletion() {
    for pointer in [
        "/Resources/Runner/Properties/UserData/Fn::Base64",
        "/Resources/Runner/Properties/Tags/0/Value",
        "/Resources/Runner/Properties/BlockDeviceMappings/0/Ebs/VolumeSize",
        "/Resources/Runner/DeletionPolicy",
        "/Resources/Runner/Properties/NetworkInterfaces/0/DeleteOnTermination",
        "/Resources/RunnerSecurityGroup/Properties/GroupDescription",
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut t = template();
        *t.pointer_mut(pointer).unwrap() = json!("changed");
        let mut s = Script::default();
        s.push("get-caller-identity", json!({"Account":"123456789012"}));
        s.push("describe-stacks", stack("CREATE_COMPLETE"));
        s.push("get-template", json!({"TemplateBody":t}));
        assert!(aws(&mut s).cleanup(&mut r, &store).is_err(), "{pointer}");
        assert!(!s.seen.contains(&"delete-stack".into()));
        assert_ne!(r.cleanup, Cleanup::Confirmed);
    }
}
#[test]
fn create_identity_is_durable_before_request_and_cleanup_is_confirmed() {
    let (_d, store) = store();
    let mut r = record();
    store.save(&r).unwrap();
    let mut s = Script {
        persisted_before_create: Some(store.path().join("operation.json")),
        ..Default::default()
    };
    s.push("create-stack", json!({"StackId":stack_id()}));
    s.ready_cleanup();
    aws(&mut s)
        .create(
            &mut r,
            &store,
            &template::binding(&template()).unwrap(),
            Path::new("/template.json"),
        )
        .unwrap();
    assert_eq!(store.read().unwrap().stack_id, Some(stack_id()));
    aws(&mut s).cleanup(&mut r, &store).unwrap();
    assert_eq!(store.read().unwrap().cleanup, Cleanup::Confirmed);
    assert!(s.steps.is_empty());
}
#[test]
fn accepted_creation_lost_response_recovers_by_name_without_recreating() {
    let (_d, store) = store();
    let mut r = record();
    store.save(&r).unwrap();
    let mut s = Script::default();
    s.steps.push_back((
        "create-stack",
        Err("accepted request, transport response lost".into()),
    ));
    s.ready_cleanup();
    assert!(
        aws(&mut s)
            .create(
                &mut r,
                &store,
                &template::binding(&template()).unwrap(),
                Path::new("/template.json")
            )
            .is_err()
    );
    let mut recovered = store.read().unwrap();
    assert_eq!(recovered.creation, Creation::Submitted);
    assert!(recovered.stack_id.is_none());
    aws(&mut s).cleanup(&mut recovered, &store).unwrap();
    assert_eq!(s.seen.iter().filter(|a| *a == "create-stack").count(), 1);
    assert!(s.steps.is_empty());
    let lookups: Vec<_> = s
        .arguments
        .iter()
        .filter(|args| args[1] == "describe-stacks")
        .collect();
    assert!(lookups[0].contains(&recovered.stack_name));
    assert!(lookups[1].contains(&stack_id()));
    let volumes = s
        .arguments
        .iter()
        .find(|args| args[1] == "describe-volumes")
        .unwrap();
    assert!(volumes.contains(&format!(
        "Name=tag:borrowser:stack-id,Values={}",
        stack_id()
    )));
}

#[test]
fn accepted_create_with_failed_arn_persistence_remains_name_recoverable() {
    let (_d, store) = store();
    let mut r = record();
    store.save(&r).unwrap();
    let path = store.path().join("operation.json");
    let mut s = Script {
        fail_persistence_after_create: Some(path.clone()),
        ..Default::default()
    };
    s.push("create-stack", json!({"StackId": stack_id()}));
    assert!(
        aws(&mut s)
            .create(
                &mut r,
                &store,
                &template::binding(&template()).unwrap(),
                Path::new("/template.json")
            )
            .is_err()
    );
    s.push("get-caller-identity", json!({"Account":"123456789012"}));
    s.verify("CREATE_COMPLETE");
    s.push("list-stack-resources", resources("CREATE_COMPLETE", true));
    // Even a valid in-memory ARN cannot permit deletion while persistence fails.
    assert!(aws(&mut s).cleanup(&mut r, &store).is_err());
    assert!(!s.seen.contains(&"delete-stack".into()));
    fs::remove_dir(&path).unwrap();
    fs::rename(path.with_extension("saved"), &path).unwrap();
    let mut recovered = store.read().unwrap();
    assert_eq!(recovered.creation, Creation::Submitted);
    assert!(recovered.stack_id.is_none());
    s.ready_cleanup();
    aws(&mut s).cleanup(&mut recovered, &store).unwrap();
    assert_eq!(store.read().unwrap().cleanup, Cleanup::Confirmed);
    assert_eq!(s.seen.iter().filter(|a| *a == "create-stack").count(), 1);
    assert!(s.steps.is_empty());
}

#[test]
fn delayed_create_visibility_is_retried_without_submitting_again() {
    let (_d, store) = store();
    let mut r = record();
    r.creation = Creation::Submitted;
    store.save(&r).unwrap();
    let mut s = Script::default();
    s.push("get-caller-identity", json!({"Account":"123456789012"}));
    s.push("describe-stacks", json!({"Stacks":[]}));
    s.verify("CREATE_COMPLETE");
    s.push("list-stack-resources", resources("CREATE_COMPLETE", true));
    s.push("delete-stack", json!({}));
    s.verify("DELETE_COMPLETE");
    s.push("list-stack-resources", resources("DELETE_COMPLETE", true));
    s.push("describe-instances", json!({"Reservations":[]}));
    s.push("describe-volumes", json!({"Volumes":[]}));
    aws(&mut s).cleanup(&mut r, &store).unwrap();
    assert_eq!(r.cleanup, Cleanup::Confirmed);
    assert!(!s.seen.contains(&"create-stack".into()));
    assert_eq!(s.now, Duration::from_secs(10));
    assert!(s.steps.is_empty());
}
#[test]
fn persistence_failure_prevents_submission_and_lock_is_exclusive() {
    let (_d, store) = store();
    let r = record();
    store.save(&r).unwrap();
    assert!(Store::open(store.path(), false).is_err());
    let record_path = store.path().join("operation.json");
    fs::remove_file(&record_path).unwrap();
    fs::create_dir(&record_path).unwrap();
    let mut r = record();
    let mut s = Script::default();
    assert!(
        aws(&mut s)
            .create(
                &mut r,
                &store,
                &template::binding(&template()).unwrap(),
                Path::new("/template.json")
            )
            .is_err()
    );
    assert!(s.seen.is_empty());
}
#[test]
fn unresolved_absence_pagination_and_volume_presence_cannot_confirm() {
    for bad in [
        json!({}),
        json!({"Volumes":[],"NextToken":"more"}),
        json!({"Volumes":[{"VolumeId":"vol-0123456789abcdef0"}]}),
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut s = Script::default();
        s.ready_cleanup();
        s.steps.back_mut().unwrap().1 = Ok(bad);
        assert!(aws(&mut s).cleanup(&mut r, &store).is_err());
        assert_eq!(r.cleanup, Cleanup::Pending);
    }
}
#[test]
fn known_volume_id_uses_exact_filter_and_missing_id_uses_stack_tags() {
    for known in [false, true] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        if known {
            r.root_volume_id = Some("vol-0123456789abcdef0".into());
            r.instance_id = Some("i-0123456789abcdef0".into());
        }
        store.save(&r).unwrap();
        let mut s = Script::default();
        s.ready_cleanup();
        aws(&mut s).cleanup(&mut r, &store).unwrap();
        assert_eq!(r.cleanup, Cleanup::Confirmed);
    }
}
#[test]
fn no_instance_requires_complete_history_and_scoped_empty_observations() {
    let (_d, store) = store();
    let mut r = record();
    r.creation = Creation::Bound;
    r.stack_id = Some(stack_id());
    store.save(&r).unwrap();
    let mut s = Script::default();
    s.push("get-caller-identity", json!({"Account":"123456789012"}));
    s.verify("DELETE_COMPLETE");
    s.push("list-stack-resources", resources("DELETE_COMPLETE", false));
    s.push("describe-stack-events",json!({"StackEvents":[{"ResourceType":"AWS::CloudFormation::Stack","ResourceStatus":"CREATE_IN_PROGRESS","ClientRequestToken":format!("ag4-create-{}",r.run_id)}]}));
    s.push("describe-instances", json!({"Reservations":[]}));
    s.push("describe-volumes", json!({"Volumes":[]}));
    aws(&mut s).cleanup(&mut r, &store).unwrap();
    assert_eq!(r.cleanup, Cleanup::Confirmed);
    assert!(!s.seen.contains(&"delete-stack".into()));
}

fn owned_tags(r: &Record) -> Value {
    json!([{"Key":"borrowser:purpose","Value":"ag4"}, {"Key":"borrowser:run-id","Value":r.run_id}, {"Key":"borrowser:stack-id","Value":stack_id()}])
}
#[test]
fn matching_resources_settle_without_another_deletion_or_creation() {
    for settles in [true, false] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        r.instance_id = Some("i-0123456789abcdef0".into());
        r.root_volume_id = Some("vol-0123456789abcdef0".into());
        store.save(&r).unwrap();
        let mut s = Script::default();
        s.push("get-caller-identity", json!({"Account":r.account}));
        s.verify("DELETE_COMPLETE");
        for _ in 0..if settles { 1 } else { 180 } {
            s.push("list-stack-resources", resources("DELETE_COMPLETE", true));
            s.push("describe-instances", json!({"Reservations":[{"Instances":[{"InstanceId":r.instance_id,"State":{"Name":"shutting-down"},"Tags":owned_tags(&r),"RootDeviceName":"/dev/sda1","BlockDeviceMappings":[{"DeviceName":"/dev/sda1","Ebs":{"VolumeId":r.root_volume_id,"DeleteOnTermination":true}}]}]}]}));
            s.push("describe-volumes", json!({"Volumes":[{"VolumeId":r.root_volume_id,"State":"deleting","Tags":owned_tags(&r)}]}));
            s.verify("DELETE_COMPLETE");
        }
        if settles {
            s.push("list-stack-resources", resources("DELETE_COMPLETE", true));
            s.push("describe-instances", json!({"Reservations":[]}));
            s.push("describe-volumes", json!({"Volumes":[]}));
        }
        let result = aws(&mut s).cleanup(&mut r, &store);
        assert_eq!(result.is_ok(), settles);
        assert_eq!(r.cleanup == Cleanup::Confirmed, settles);
        assert_eq!(s.now, Duration::from_secs(if settles { 5 } else { 900 }));
        assert!(
            !s.seen
                .iter()
                .any(|a| matches!(a.as_str(), "create-stack" | "delete-stack"))
        );
    }
}
#[test]
fn conflicting_or_incomplete_terminal_observations_are_not_pending() {
    for malformed in [
        "conflict",
        "missing-tags",
        "state",
        "missing-array",
        "continuation",
        "duplicates",
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        r.instance_id = Some("i-0123456789abcdef0".into());
        r.root_volume_id = Some("vol-0123456789abcdef0".into());
        store.save(&r).unwrap();
        let mut s = Script::default();
        s.push("get-caller-identity", json!({"Account":r.account}));
        s.verify("DELETE_COMPLETE");
        s.push("list-stack-resources", resources("DELETE_COMPLETE", true));
        s.push("describe-instances", json!({"Reservations":[]}));
        let mut volume =
            json!({"VolumeId":r.root_volume_id,"State":"deleting","Tags":owned_tags(&r)});
        match malformed {
            "conflict" => volume["VolumeId"] = json!("vol-unrelated"),
            "missing-tags" => {
                volume.as_object_mut().unwrap().remove("Tags");
            }
            "state" => volume["State"] = json!("unexpected"),
            _ => {}
        }
        s.push(
            "describe-volumes",
            match malformed {
                "missing-array" => json!({}),
                "continuation" => json!({"Volumes":[],"NextToken":"more"}),
                "duplicates" => json!({"Volumes":[volume,volume]}),
                _ => json!({"Volumes":[volume]}),
            },
        );
        assert!(aws(&mut s).cleanup(&mut r, &store).is_err(), "{malformed}");
        assert_eq!(
            s.now,
            Duration::ZERO,
            "must refuse immediately: {malformed}"
        );
        assert_eq!(r.cleanup, Cleanup::Pending);
        assert!(s.steps.is_empty());
    }
}
#[test]
fn first_interruption_allows_cleanup_second_preserves_pending() {
    for count in [1, 2] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut s = Script {
            cancel_count: count,
            ..Default::default()
        };
        if count == 1 {
            s.ready_cleanup();
        }
        let result = aws(&mut s).cleanup(&mut r, &store);
        assert_eq!(result.is_ok(), count == 1);
        if count == 2 {
            assert_eq!(r.cleanup, Cleanup::Pending);
            assert!(s.seen.is_empty());
        }
    }
}
#[test]
fn binding_conflicts_and_incomplete_template_refuse_deletion() {
    for field in [
        "StackName",
        "RoleARN",
        "DisableRollback",
        "EnableTerminationProtection",
        "TimeoutInMinutes",
        "Tags",
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut s = Script::default();
        s.push("get-caller-identity", json!({"Account":"123456789012"}));
        let mut st = stack("CREATE_COMPLETE");
        st["Stacks"][0].as_object_mut().unwrap().remove(field);
        s.push("describe-stacks", st);
        s.push("get-template", json!({"TemplateBody":template()}));
        assert!(aws(&mut s).cleanup(&mut r, &store).is_err(), "{field}");
        assert!(!s.seen.contains(&"delete-stack".into()));
    }
}

#[test]
fn termination_protection_omission_is_specific_to_deleted_unprotected_stack() {
    for (status, protection, accepted) in [
        ("CREATE_COMPLETE", Some(json!(false)), true),
        ("CREATE_COMPLETE", None, false),
        ("CREATE_IN_PROGRESS", None, false),
        ("DELETE_IN_PROGRESS", None, false),
        ("DELETE_FAILED", None, false),
        ("ROLLBACK_COMPLETE", None, false),
        ("DELETE_COMPLETE", Some(json!(false)), true),
        ("DELETE_COMPLETE", None, true),
        ("DELETE_COMPLETE", Some(json!(true)), false),
        ("DELETE_COMPLETE", Some(Value::Null), false),
        ("DELETE_COMPLETE", Some(json!("false")), false),
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut observed = stack(status);
        let fields = observed["Stacks"][0].as_object_mut().unwrap();
        fields.remove("EnableTerminationProtection");
        if let Some(value) = &protection {
            fields.insert("EnableTerminationProtection".into(), value.clone());
        }
        let mut s = Script::default();
        s.push("describe-stacks", observed);
        s.push("get-template", json!({"TemplateBody":template()}));
        assert_eq!(
            aws(&mut s).authorize(&mut r, &store).is_ok(),
            accepted,
            "{status} {protection:?}"
        );
        assert!(s.steps.is_empty());
    }
}

#[test]
fn deleted_stack_omission_does_not_relax_other_authority() {
    for field in [
        "StackId",
        "StackName",
        "RoleARN",
        "Tags",
        "DisableRollback",
        "TimeoutInMinutes",
    ] {
        let (_d, store) = store();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        store.save(&r).unwrap();
        let mut observed = stack("DELETE_COMPLETE");
        let fields = observed["Stacks"][0].as_object_mut().unwrap();
        fields.remove("EnableTerminationProtection");
        fields.remove(field);
        let mut s = Script::default();
        s.push("describe-stacks", observed);
        if field != "StackId" {
            s.push("get-template", json!({"TemplateBody":template()}));
        }
        assert!(aws(&mut s).authorize(&mut r, &store).is_err(), "{field}");
        assert!(s.steps.is_empty());
    }
}

#[test]
fn deleted_stack_recovery_confirms_resources_and_preserves_capture_failure() {
    let (_d, store) = store();
    let mut r = record();
    r.creation = Creation::Bound;
    r.stack_id = Some(stack_id());
    r.instance_id = Some("i-0123456789abcdef0".into());
    r.root_volume_id = Some("vol-0123456789abcdef0".into());
    r.capture = state::Capture::Failed;
    r.primary_failure = Some("AWS phase deadline exceeded".into());
    r.cleanup_failure = Some("stack operation binding mismatch".into());
    store.save(&r).unwrap();
    let mut observed = stack("DELETE_COMPLETE");
    observed["Stacks"][0]
        .as_object_mut()
        .unwrap()
        .remove("EnableTerminationProtection");
    let mut s = Script::default();
    s.push("get-caller-identity", json!({"Account":"123456789012"}));
    s.push("describe-stacks", observed);
    s.push("get-template", json!({"TemplateBody":template()}));
    s.push("list-stack-resources", resources("DELETE_COMPLETE", true));
    s.push(
        "describe-instances",
        json!({"Reservations":[{"Instances":[{
        "InstanceId":r.instance_id,"State":{"Name":"terminated"}}]}]}),
    );
    s.push("describe-volumes", json!({"Volumes":[]}));
    aws(&mut s).cleanup(&mut r, &store).unwrap();
    let saved = store.read().unwrap();
    assert_eq!(saved.cleanup, Cleanup::Confirmed);
    assert!(saved.cleanup_failure.is_none());
    assert_eq!(
        saved.primary_failure.as_deref(),
        Some("AWS phase deadline exceeded")
    );
    assert_eq!(saved.capture, state::Capture::Failed);
    assert!(s.steps.is_empty());
    assert!(
        !s.seen
            .iter()
            .any(|a| a == "create-stack" || a == "delete-stack")
    );
    assert!(
        s.arguments
            .last()
            .unwrap()
            .contains(&"Name=volume-id,Values=vol-0123456789abcdef0".into())
    );
}

#[test]
fn every_capture_failure_reaches_cleanup_and_preserves_both_failures() {
    for phase in [
        "deployment",
        "readiness",
        "upload",
        "execution",
        "retrieval",
        "validation",
        "cancellation",
    ] {
        for cleanup_fails in [false, true] {
            let (_d, store) = store();
            let mut r = record();
            r.creation = Creation::Bound;
            r.stack_id = Some(stack_id());
            store.save(&r).unwrap();
            let mut s = Script::default();
            s.ready_cleanup();
            if cleanup_fails {
                s.steps.back_mut().unwrap().1 =
                    Ok(json!({"Volumes":[{"VolumeId":"vol-remaining"}]}));
            }
            let (primary, cleanup) =
                capture_then_cleanup(&mut aws(&mut s), &mut r, &store, |_, _| Err(phase.into()));
            assert_eq!(primary.unwrap_err(), phase);
            assert_eq!(cleanup.is_err(), cleanup_fails);
            cloudformation::record_failure(
                &mut r,
                &store,
                Some(phase),
                cleanup.as_ref().err().map(String::as_str),
            )
            .unwrap();
            let saved = store.read().unwrap();
            assert_eq!(saved.primary_failure.as_deref(), Some(phase));
            assert_eq!(saved.cleanup_failure.is_some(), cleanup_fails);
            assert!(s.seen.contains(&"delete-stack".into()));
        }
    }
}

#[test]
fn no_observation_after_ambiguous_create_is_not_cleanup() {
    let (_d, store) = store();
    let mut r = record();
    r.creation = Creation::Submitted;
    store.save(&r).unwrap();
    let mut s = Script::default();
    s.push("get-caller-identity", json!({"Account":"123456789012"}));
    // Successful, complete but empty exact-name responses still cannot settle creation.
    for _ in 0..60 {
        s.push("describe-stacks", json!({"Stacks":[]}));
    }
    assert!(aws(&mut s).cleanup(&mut r, &store).is_err());
    assert_eq!(r.cleanup, Cleanup::Pending);
    assert_eq!(s.now, Duration::from_secs(300));
    assert!(!s.seen.contains(&"create-stack".into()));
    assert!(!s.seen.contains(&"delete-stack".into()));
}

#[test]
fn readiness_binds_the_cli_decoded_console_key_to_the_exact_instance_and_run() {
    let (_d, store) = store();
    let work = tempfile::tempdir().unwrap();
    let mut r = record();
    r.creation = Creation::Bound;
    r.stack_id = Some(stack_id());
    r.instance_id = Some("i-0123456789abcdef0".into());
    store.save(&r).unwrap();
    let mut s = Script::default();
    s.push(
        "describe-instances",
        json!({"Reservations":[{"Instances":[eice::tests::instance(&r)]}]}),
    );
    let public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEcm83UIknGkWMqM4JRfllStJlXXRhBszo9bo1ufsoTY";
    s.push("get-console-output",json!({"InstanceId":r.instance_id,"Output":format!("boot output\nBORROWSER_AG4_READY {} {public}\n",r.run_id)}));
    eice::tests::target_responses(&mut s, &r);
    let host = ssh::ready(
        &mut aws(&mut s),
        &mut r,
        &store,
        work.path(),
        work.path().join("key"),
        &eice::tests::endpoint(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(host.known_hosts).unwrap(),
        format!("i-0123456789abcdef0 {public}\n")
    );
    assert_eq!(
        store.read().unwrap().root_volume_id.as_deref(),
        Some("vol-0123456789abcdef0")
    );
}

fn readiness_poll(s: &mut Script, r: &Record, console: &str) {
    s.push(
        "describe-instances",
        json!({"Reservations":[{"Instances":[{
        "InstanceId":r.instance_id,"State":{"Name":"running"},"PublicIpAddress":"192.0.2.10",
        "RootDeviceName":"/dev/sda1","BlockDeviceMappings":[{"DeviceName":"/dev/sda1",
            "Ebs":{"VolumeId":"vol-0123456789abcdef0","DeleteOnTermination":true}}],
        "Tags":owned_tags(r)}]}]}),
    );
    s.push(
        "get-console-output",
        json!({"InstanceId":r.instance_id,"Output":console}),
    );
}

#[test]
fn bootstrap_failure_marker_is_immediate_primary_failure_and_still_cleans_up() {
    for (phase, status) in [
        ("apt_install", 124),
        ("ssh_prepare", 9),
        ("ssh_config", 1),
        ("ssh_validate", 1),
        ("ssh_validate", 255),
        ("ssh_restart", 5),
        ("ssh_probe_loopback", 1),
        ("ssh_probe_private", 1),
    ] {
        let (_d, store) = store();
        let work = tempfile::tempdir().unwrap();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        r.instance_id = Some("i-0123456789abcdef0".into());
        store.save(&r).unwrap();
        let mut s = Script::default();
        readiness_poll(
            &mut s,
            &r,
            &format!(
                "boot\nBORROWSER_AG4_BOOTSTRAP_FAILED {} {phase} {status}\r\n",
                r.run_id
            ),
        );
        s.ready_cleanup();
        let (primary, cleanup) =
            capture_then_cleanup(&mut aws(&mut s), &mut r, &store, |aws, r| {
                ssh::ready(
                    aws,
                    r,
                    &store,
                    work.path(),
                    work.path().join("key"),
                    &eice::tests::endpoint(),
                )
                .map(|_| vec![])
            });
        assert_eq!(
            primary.err().as_deref(),
            Some(format!("guest bootstrap failed: phase={phase}, exit-status={status}").as_str())
        );
        assert!(cleanup.is_ok());
        assert_eq!(
            s.seen.iter().filter(|a| *a == "get-console-output").count(),
            1
        );
        assert_eq!(store.read().unwrap().cleanup, Cleanup::Confirmed);
        assert!(!work.path().join("known_hosts").exists());
        assert!(s.steps.is_empty());
    }
}

#[test]
fn malformed_wrong_run_or_conflicting_bootstrap_failures_cannot_authorize_ssh() {
    for tail in [
        "apt_install 0",
        "apt_install -1",
        "apt_install 256",
        "apt_install 0124",
        "apt_install +1",
        "apt_install timeout",
        "unknown_phase 100",
        "apt_install 100 extra",
        "apt_install",
        "ssh 255",
        "ssh_validate 0",
        "ssh_validate 255 extra",
        "ssh_restart timeout",
        "ssh_probe 1",
        "ssh_probe_loopback 0",
        "ssh_probe_private 1 extra",
    ] {
        check_bad_bootstrap_console(&format!(
            "BORROWSER_AG4_BOOTSTRAP_FAILED 0123456789abcdef0123456789abcdef {tail}\n"
        ));
    }
    for phase in [
        "ssh_prepare",
        "ssh_config",
        "ssh_validate",
        "ssh_restart",
        "ssh_probe_loopback",
        "ssh_probe_private",
    ] {
        check_bad_bootstrap_console(&format!(
            "BORROWSER_AG4_BOOTSTRAP_FAILED fedcba9876543210fedcba9876543210 {phase} 255\n"
        ));
    }
    for text in [
        "BORROWSER_AG4_BOOTSTRAP_FAILED fedcba9876543210fedcba9876543210 apt_install 124\n",
        "BORROWSER_AG4_BOOTSTRAP_FAILED\n",
        "BORROWSER_AG4_BOOTSTRAP_FAILED 0123456789abcdef0123456789abcdef apt_install 124\nBORROWSER_AG4_BOOTSTRAP_FAILED 0123456789abcdef0123456789abcdef ssh_validate 1\n",
        "BORROWSER_AG4_READY 0123456789abcdef0123456789abcdef ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEcm83UIknGkWMqM4JRfllStJlXXRhBszo9bo1ufsoTY\nBORROWSER_AG4_BOOTSTRAP_FAILED 0123456789abcdef0123456789abcdef ssh_validate 1\n",
    ] {
        check_bad_bootstrap_console(text);
    }
}

fn check_bad_bootstrap_console(text: &str) {
    let (_d, store) = store();
    let work = tempfile::tempdir().unwrap();
    let mut r = record();
    r.creation = Creation::Bound;
    r.stack_id = Some(stack_id());
    r.instance_id = Some("i-0123456789abcdef0".into());
    store.save(&r).unwrap();
    let mut s = Script::default();
    readiness_poll(&mut s, &r, text);
    assert!(
        ssh::ready(
            &mut aws(&mut s),
            &mut r,
            &store,
            work.path(),
            work.path().join("key"),
            &eice::tests::endpoint(),
        )
        .is_err()
    );
    assert!(!work.path().join("known_hosts").exists());
    assert_eq!(s.now, Duration::ZERO);
    assert!(s.steps.is_empty());
}

#[test]
fn readiness_without_markers_times_out_and_cannot_borrow_capture_reserve() {
    for insufficient_budget in [false, true] {
        let (_d, store) = store();
        let work = tempfile::tempdir().unwrap();
        let mut r = record();
        r.creation = Creation::Bound;
        r.stack_id = Some(stack_id());
        r.instance_id = Some("i-0123456789abcdef0".into());
        store.save(&r).unwrap();
        let mut s = Script::default();
        if !insufficient_budget {
            for _ in 0..216 {
                readiness_poll(&mut s, &r, "ordinary boot output\n");
            }
        }
        let mut a = aws(&mut s);
        if insufficient_budget {
            a.deadline = Duration::from_secs(1619);
        }
        let error = ssh::ready(
            &mut a,
            &mut r,
            &store,
            work.path(),
            work.path().join("key"),
            &eice::tests::endpoint(),
        )
        .err()
        .unwrap();
        assert_eq!(
            error,
            if insufficient_budget {
                "insufficient work budget for full bootstrap readiness and capture reserve"
            } else {
                "host/bootstrap readiness deadline exceeded"
            }
        );
        assert_eq!(s.now.as_secs(), if insufficient_budget { 0 } else { 1080 });
        assert_eq!(s.seen.len(), if insufficient_budget { 0 } else { 432 });
        assert!(s.steps.is_empty());
        assert!(!work.path().join("known_hosts").exists());
    }
}

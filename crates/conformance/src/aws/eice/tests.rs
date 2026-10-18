use super::*;
use crate::aws::{
    self,
    ssh::Host,
    tests::{Script, resources, stack_id},
};
use std::os::unix::fs::{PermissionsExt, symlink};

pub(in crate::aws) fn config() -> Config {
    let mut c: Config =
        serde_json::from_slice(include_bytes!("../../../aws/operator.example.json")).unwrap();
    c.account = "123456789012".into();
    c.vpc_id = "vpc-0123456789abcdef0".into();
    c.subnet_id = "subnet-0123456789abcdef0".into();
    c.cloudformation_role_arn = "arn:aws:iam::123456789012:role/borrowser/reviewed-role".into();
    c.shared_template_sha256 = template::digest(&shared_template()).unwrap();
    c.aws_cli_identity = Some((1, 2, 3, "a".repeat(64)));
    c
}
pub(in crate::aws) fn endpoint() -> VerifiedEndpoint {
    let c = config();
    VerifiedEndpoint {
        account: c.account,
        stack_id: c.shared_stack_id,
        template_sha256: c.shared_template_sha256,
        endpoint_id: "eice-0123456789abcdef0".into(),
        endpoint_dns:
            "eice-0123456789abcdef0.b67b86ba.ec2-instance-connect-endpoint.eu-west-1.amazonaws.com"
                .into(),
        vpc_id: c.vpc_id,
        subnet_id: c.subnet_id,
        subnet_cidr: "10.77.0.0/26".into(),
        az: "eu-west-1a".into(),
        endpoint_sg_id: "sg-11111111111111111".into(),
        target_sg_id: "sg-22222222222222222".into(),
        resource_ids: BTreeMap::new(),
        endpoint_eni: "eni-11111111111111111".into(),
    }
}
fn target() -> VerifiedTarget {
    VerifiedTarget {
        instance_id: "i-0123456789abcdef0".into(),
        run_id: "0123456789abcdef0123456789abcdef".into(),
        stack_id: stack_id(),
        primary_eni: "eni-0123456789abcdef0".into(),
        private_ip: "10.77.0.10".parse().unwrap(),
        host_key:
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEcm83UIknGkWMqM4JRfllStJlXXRhBszo9bo1ufsoTY"
                .into(),
    }
}
pub(in crate::aws) fn host(work: &tempfile::TempDir) -> Host {
    fs::set_permissions(work.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = work.path().canonicalize().unwrap();
    Host {
        target: target(),
        endpoint: endpoint(),
        known_hosts: path.join("known_hosts"),
        key: path.join("key"),
    }
}
fn shared_template() -> Value {
    let mut resources = serde_json::Map::new();
    for (name, ty) in SHARED {
        resources.insert(name.into(), json!({"Type":ty}));
    }
    resources.get_mut("PublicSubnet").unwrap()["Properties"] =
        json!({"CidrBlock":"10.77.0.0/26","AvailabilityZone":"eu-west-1a"});
    json!({"Resources":resources})
}
fn owned_tags(r: &Record) -> Value {
    json!([{"Key":"borrowser:purpose","Value":"ag4"},{"Key":"borrowser:run-id","Value":r.run_id},{"Key":"borrowser:stack-id","Value":r.stack_id}])
}
fn groups() -> Value {
    json!([{"GroupId":"sg-0123456789abcdef0"},{"GroupId":"sg-22222222222222222"}])
}
pub(in crate::aws) fn instance(r: &Record) -> Value {
    json!({"InstanceId":r.instance_id,"State":{"Name":"running"},"PrivateIpAddress":"10.77.0.10",
        "VpcId":"vpc-0123456789abcdef0","SubnetId":"subnet-0123456789abcdef0","Placement":{"AvailabilityZone":"eu-west-1a"},
        "SecurityGroups":groups(),"NetworkInterfaces":[{"NetworkInterfaceId":"eni-0123456789abcdef0",
            "PrivateIpAddress":"10.77.0.10","Attachment":{"DeviceIndex":0},"Groups":groups()}],
        "RootDeviceName":"/dev/sda1","BlockDeviceMappings":[{"DeviceName":"/dev/sda1","Ebs":{"VolumeId":"vol-0123456789abcdef0","DeleteOnTermination":true}}],
        "Tags":owned_tags(r)})
}
fn rule(port: u32, group: Option<&str>) -> Value {
    json!([{"IpProtocol":"tcp","FromPort":port,"ToPort":port,"Ipv6Ranges":[],"PrefixListIds":[],
        "IpRanges":if group.is_none(){json!([{"CidrIp":"0.0.0.0/0"}])}else{json!([])},
        "UserIdGroupPairs":group.map(|g|json!([{"UserId":"123456789012","GroupId":g}])).unwrap_or(json!([]))}])
}
fn runner_group(r: &Record) -> Value {
    json!({"SecurityGroups":[{"GroupId":"sg-0123456789abcdef0","OwnerId":"123456789012","VpcId":"vpc-0123456789abcdef0",
        "Tags":owned_tags(r),"IpPermissions":rule(22,Some("sg-11111111111111111")),"IpPermissionsEgress":rule(443,None)}]})
}
fn runner_eni(r: &Record) -> Value {
    json!({"NetworkInterfaces":[{"NetworkInterfaceId":"eni-0123456789abcdef0","OwnerId":"123456789012",
        "VpcId":"vpc-0123456789abcdef0","SubnetId":"subnet-0123456789abcdef0","AvailabilityZone":"eu-west-1a",
        "Status":"in-use","RequesterManaged":false,"Groups":groups(),"PrivateIpAddress":"10.77.0.10",
        "PrivateIpAddresses":[{"PrivateIpAddress":"10.77.0.10","Primary":true}],
        "Attachment":{"InstanceId":r.instance_id,"DeviceIndex":0,"DeleteOnTermination":true,"Status":"attached"}}]})
}
pub(in crate::aws) fn target_responses(s: &mut Script, r: &Record) {
    s.push("list-stack-resources", resources("CREATE_COMPLETE", true));
    s.push("describe-security-groups", runner_group(r));
    s.push("describe-network-interfaces", runner_eni(r));
}

#[test]
fn descriptor_fixed_argv_and_private_lifecycle() {
    let work = tempfile::tempdir().unwrap();
    let host = host(&work);
    let path = TunnelDescriptor::create(
        &config(),
        &host.endpoint,
        &host.target,
        &work.path().canonicalize().unwrap(),
        Duration::from_secs(20),
    )
    .unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    let d = TunnelDescriptor::load(&path).unwrap();
    assert_eq!(
        d.argv(),
        vec![
            "--region",
            "eu-west-1",
            "--no-cli-pager",
            "--no-cli-auto-prompt",
            "ec2-instance-connect",
            "open-tunnel",
            "--instance-connect-endpoint-id",
            "eice-0123456789abcdef0",
            "--instance-connect-endpoint-dns-name",
            "eice-0123456789abcdef0.b67b86ba.ec2-instance-connect-endpoint.eu-west-1.amazonaws.com",
            "--private-ip-address",
            "10.77.0.10",
            "--remote-port",
            "22",
            "--max-tunnel-duration",
            "20"
        ]
    );
    assert_eq!(d.aws_cli_path, config().aws_cli);
    drop(work);
    assert!(!path.exists());
}

#[test]
fn executable_identity_rejects_links_replacement_and_mutation() {
    let work = tempfile::tempdir().unwrap();
    let _ = host(&work);
    let path = work.path().canonicalize().unwrap().join("aws");
    fs::write(&path, b"qualified executable").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let original = cli_identity(&path).unwrap();
    assert!(cli_identity(Path::new("aws")).is_err());
    let link = path.with_extension("link");
    symlink(&path, &link).unwrap();
    assert!(cli_identity(&link).is_err());
    fs::rename(&path, path.with_extension("old")).unwrap();
    fs::write(&path, b"qualified executable").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert_ne!(cli_identity(&path).unwrap(), original);
    let identity = cli_identity(&path).unwrap();
    fs::write(&path, b"substituted content!").unwrap();
    assert_ne!(cli_identity(&path).unwrap(), identity);
    let mut c = config();
    c.aws_cli = path.clone();
    c.aws_cli_identity = Some(original);
    let p = TunnelDescriptor::create(
        &c,
        &endpoint(),
        &target(),
        &work.path().canonicalize().unwrap(),
        Duration::from_secs(20),
    )
    .unwrap();
    assert!(TunnelDescriptor::load(&p).unwrap().verify_cli().is_err());
    fs::remove_file(&path).unwrap();
    symlink(path.with_extension("old"), &path).unwrap();
    assert!(TunnelDescriptor::load(&p).unwrap().verify_cli().is_err());
}

#[test]
fn descriptor_rejects_extra_fields_permissions_links_bounds_and_invalid_destinations() {
    let work = tempfile::tempdir().unwrap();
    let _ = host(&work);
    let root = work.path().canonicalize().unwrap();
    let path = TunnelDescriptor::create(
        &config(),
        &endpoint(),
        &target(),
        &root,
        Duration::from_secs(20),
    )
    .unwrap();
    let original = fs::read(&path).unwrap();
    for (field, value) in [
        ("program", json!("/bin/sh")),
        ("args", json!(["--debug"])),
        ("aws_cli_path", json!("aws")),
        ("aws_cli_path", json!("/bin/aws\n")),
        ("private_ip", json!("10.78.0.2")),
        ("max_tunnel_duration", json!(0)),
        ("max_tunnel_duration", json!(301)),
        ("endpoint_dns", json!("attacker.invalid")),
        ("instance_id", json!("i-*")),
    ] {
        let mut v: Value = serde_json::from_slice(&original).unwrap();
        v[field] = value;
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        assert!(TunnelDescriptor::load(&path).is_err(), "{field}");
    }
    fs::write(&path, &original).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(TunnelDescriptor::load(&path).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let link = root.join("link");
    symlink(&path, &link).unwrap();
    assert!(TunnelDescriptor::load(&link).is_err());
    fs::write(&path, vec![b' '; 16385]).unwrap();
    assert!(TunnelDescriptor::load(&path).is_err());
}

#[test]
fn target_is_private_interface_bound_and_rejects_substitutions() {
    for mutation in [
        "none",
        "public-only",
        "wrong-ip",
        "wrong-sg",
        "extra-eni",
        "wrong-vpc",
        "profile",
    ] {
        let (_dir, store) = aws::tests::store();
        let mut r = aws::tests::record();
        r.stack_id = Some(stack_id());
        r.instance_id = Some(target().instance_id);
        r.creation = aws::state::Creation::Bound;
        store.save(&r).unwrap();
        let mut i = instance(&r);
        match mutation {
            "public-only" => {
                i.as_object_mut().unwrap().remove("PrivateIpAddress");
                i["PublicIpAddress"] = json!("192.0.2.1");
            }
            "wrong-ip" => i["PrivateIpAddress"] = json!("10.77.0.11"),
            "wrong-sg" => i["SecurityGroups"][0]["GroupId"] = json!("sg-33333333333333333"),
            "extra-eni" => {
                let extra = i["NetworkInterfaces"][0].clone();
                i["NetworkInterfaces"].as_array_mut().unwrap().push(extra);
            }
            "wrong-vpc" => i["VpcId"] = json!("vpc-33333333333333333"),
            "profile" => i["IamInstanceProfile"] = json!({}),
            _ => (),
        }
        let mut s = Script::default();
        target_responses(&mut s, &r);
        let result = endpoint().verify_target(
            &mut aws::tests::aws(&mut s),
            &mut r,
            &store,
            &i,
            &target().host_key,
        );
        assert_eq!(result.is_ok(), mutation == "none", "{mutation}");
        if let Ok(t) = result {
            assert_eq!(t.private_ip.to_string(), "10.77.0.10");
            assert_eq!(t.instance_id, r.instance_id.unwrap());
        }
    }
}

#[test]
fn config_has_no_legacy_or_manual_transport_authority() {
    let original: Value =
        serde_json::from_slice(include_bytes!("../../../aws/operator.example.json")).unwrap();
    for key in [
        "controller_cidr",
        "shared_resource_ids",
        "endpoint_dns",
        "eice_id",
        "aws_cli_identity",
    ] {
        let mut v = original.clone();
        v[key] = json!("unreviewed");
        assert!(serde_json::from_value::<Config>(v).is_err(), "{key}");
    }
    let c = config();
    assert!(shared_authority(&c.account, &c.shared_stack_id, &c.shared_template_sha256).is_ok());
    for stack in [
        "borrowser-ag4-shared",
        &c.shared_stack_id.replace("eu-west-1", "us-east-1"),
        &c.shared_stack_id.replace("123456789012", "123456789013"),
    ] {
        assert!(shared_authority(&c.account, stack, &c.shared_template_sha256).is_err());
    }
}

#[test]
fn live_capture_rejects_authentic_historical_topologies() {
    for bytes in [
        include_bytes!("../../../aws/tests/legacy-direct-ssh-template.json").as_slice(),
        include_bytes!("../../../aws/tests/legacy-direct-ssh-deployed-template.json").as_slice(),
    ] {
        let t = template::json(bytes).unwrap();
        assert!(template::binding(&t).is_ok());
        assert!(validate_live_template(&t, &config(), &endpoint()).is_err());
    }
    assert!(
        validate_live_template(
            &template::json(include_bytes!("../../../aws/tests/synthesized.json")).unwrap(),
            &config(),
            &endpoint()
        )
        .is_ok()
    );
}

fn shared_responses() -> Vec<(&'static str, Value)> {
    let c = config();
    let e = endpoint();
    let tags = |logical: &str| {
        json!([{"Key":"borrowser:purpose","Value":"ag4-shared"},
        {"Key":"aws:cloudformation:stack-id","Value":c.shared_stack_id},
        {"Key":"aws:cloudformation:logical-id","Value":logical}])
    };
    let rows=SHARED.iter().map(|(name,ty)| {
        let physical=match *name {
            "Vpc"=>c.vpc_id.clone(),"PublicSubnet"=>c.subnet_id.clone(),"CaptureExecutionRole"=>"reviewed-role".into(),
            "EndpointSecurityGroup"=>e.endpoint_sg_id.clone(),"RunnerTransportTargetSecurityGroup"=>e.target_sg_id.clone(),
            "InstanceConnectEndpoint"=>e.endpoint_id.clone(),_=>format!("retained-{name}")};
        json!({"LogicalResourceId":name,"ResourceType":ty,"PhysicalResourceId":physical,"ResourceStatus":"CREATE_COMPLETE"})
    }).collect::<Vec<_>>();
    let group = |logical: &str, id: &str, rules: Value| {
        json!({"SecurityGroups":[{"GroupId":id,"OwnerId":c.account,"VpcId":c.vpc_id,
        "IpPermissions":[],"IpPermissionsEgress":rules,"Tags":tags(logical)}]})
    };
    vec![
        (
            "describe-stacks",
            json!({"Stacks":[{"StackId":c.shared_stack_id,"StackName":"borrowser-ag4-shared","EnableTerminationProtection":true,"StackStatus":"CREATE_COMPLETE"}]}),
        ),
        ("get-template", json!({"TemplateBody":shared_template()})),
        (
            "list-stack-resources",
            json!({"StackResourceSummaries":rows}),
        ),
        (
            "describe-subnets",
            json!({"Subnets":[{"SubnetId":c.subnet_id,"VpcId":c.vpc_id,"OwnerId":c.account,"State":"available",
            "CidrBlock":e.subnet_cidr,"AvailabilityZone":e.az,"Ipv6Native":false,"Ipv6CidrBlockAssociationSet":[]}]}),
        ),
        (
            "describe-instance-connect-endpoints",
            json!({"InstanceConnectEndpoints":[{"InstanceConnectEndpointId":e.endpoint_id,
            "InstanceConnectEndpointArn":format!("arn:aws:ec2:eu-west-1:{}:instance-connect-endpoint/{}",c.account,e.endpoint_id),
            "OwnerId":c.account,"VpcId":c.vpc_id,"SubnetId":c.subnet_id,"AvailabilityZone":e.az,"IpAddressType":"ipv4",
            "PreserveClientIp":false,"SecurityGroupIds":[e.endpoint_sg_id],"State":"create-complete","Tags":tags("InstanceConnectEndpoint"),
            "DnsName":e.endpoint_dns,"NetworkInterfaceIds":["eni-11111111111111111"]}]}),
        ),
        (
            "describe-network-interfaces",
            json!({"NetworkInterfaces":[{"NetworkInterfaceId":"eni-11111111111111111","OwnerId":c.account,
            "VpcId":c.vpc_id,"SubnetId":c.subnet_id,"AvailabilityZone":e.az,"RequesterManaged":false,
            "InterfaceType":"ec2_instance_connect_endpoint","Status":"in-use",
            "Ipv6Addresses":[],"PrivateIpAddress":"10.77.0.5","Groups":[{"GroupId":e.endpoint_sg_id}]}]}),
        ),
        (
            "describe-security-groups",
            group(
                "EndpointSecurityGroup",
                &e.endpoint_sg_id,
                rule(22, Some(&e.target_sg_id)),
            ),
        ),
        (
            "describe-security-groups",
            group(
                "RunnerTransportTargetSecurityGroup",
                &e.target_sg_id,
                rule(443, None),
            ),
        ),
    ]
}

#[test]
fn endpoint_reported_eice_interface_accepts_real_shape_without_requester_managed_authority() {
    // The deployed EICE reported RequesterManaged=false. Its authenticated ENI
    // relationship and EICE interface type bind it, not RequesterManaged.
    for requester_managed in [Some(false), Some(true), None] {
        let mut observations = shared_responses();
        let eni = &mut observations[5].1["NetworkInterfaces"][0];
        assert_eq!(eni["RequesterManaged"], false);
        assert_eq!(eni["InterfaceType"], "ec2_instance_connect_endpoint");
        if let Some(value) = requester_managed {
            eni["RequesterManaged"] = json!(value);
        } else {
            eni.as_object_mut().unwrap().remove("RequesterManaged");
        }
        let mut script = Script::default();
        for (action, value) in observations {
            script.push(action, value);
        }
        let verified = verify_shared(&mut aws::tests::aws(&mut script), &config()).unwrap();
        assert_eq!(verified.endpoint_eni, "eni-11111111111111111");
        assert!(
            script.arguments[5].windows(2).any(|pair| {
                pair == ["--network-interface-ids", verified.endpoint_eni.as_str()]
            })
        );
        assert!(script.steps.is_empty());
    }
}

#[test]
fn authenticated_regional_endpoint_dns_is_validated_and_carried_unchanged() {
    let c = config();
    let dns = "eice-0123456789abcdef0.opaque-7b67b86ba.ec2-instance-connect-endpoint.eu-west-1.amazonaws.com";
    let mut observations = shared_responses();
    observations[4].1["InstanceConnectEndpoints"][0]["DnsName"] = json!(dns);
    let mut script = Script::default();
    for (action, value) in observations {
        script.push(action, value);
    }
    let mut a = aws::tests::aws(&mut script);
    let endpoint = verify_shared(&mut a, &c).unwrap();
    assert_eq!(endpoint.endpoint_dns, dns);
    let work = tempfile::tempdir().unwrap();
    let _ = host(&work);
    let path = TunnelDescriptor::create(
        &c,
        &endpoint,
        &target(),
        &work.path().canonicalize().unwrap(),
        Duration::from_secs(20),
    )
    .unwrap();
    let descriptor = TunnelDescriptor::load(&path).unwrap();
    let argv = descriptor.argv();
    let at = argv
        .iter()
        .position(|s| s == "--instance-connect-endpoint-dns-name")
        .unwrap();
    assert_eq!(argv[at + 1], dns);
    for bad in [
        dns.replace("ec2-instance-connect-endpoint", "ec2-instance-connect"),
        dns.replace("eu-west-1", "eu-west-2"),
        dns.replace("amazonaws.com", "amazonaws.com.attacker.invalid"),
        dns.replace("amazonaws.com", "amazonaws-com.invalid"),
        dns.replace(
            "ec2-instance-connect-endpoint",
            "ec2-instance-connect-endpoint-lookalike",
        ),
        dns.replace("eice-0123456789abcdef0", "eice-11111111111111111"),
        dns.replace("opaque-7b67b86ba", ""),
        dns.replace("opaque-7b67b86ba", "-opaque"),
        dns.replace("opaque-7b67b86ba", "opaque-"),
        dns.replace("opaque-7b67b86ba", "opaque_label"),
        dns.replace("opaque-7b67b86ba", "opaque.label"),
        dns.replace("opaque-7b67b86ba", &"a".repeat(64)),
        dns.replace("opaque-7b67b86ba", &"a".repeat(254)),
        format!("https://{dns}"),
        format!("{dns}:443"),
        format!("{dns}."),
        "arbitrary.amazonaws.com".into(),
        "127.0.0.1".into(),
    ] {
        assert!(endpoint_dns(&bad, &endpoint.endpoint_id).is_err(), "{bad}");
        let mut observations = shared_responses();
        observations[4].1["InstanceConnectEndpoints"][0]["DnsName"] = json!(bad);
        let mut script = Script::default();
        for (action, value) in observations {
            script.push(action, value);
        }
        let mut a = aws::tests::aws(&mut script);
        assert!(verify_shared(&mut a, &c).is_err(), "{bad}");
        let mut descriptor = TunnelDescriptor::load(&path).unwrap();
        descriptor.endpoint_dns = bad;
        assert!(descriptor.validate().is_err());
    }
}

#[test]
fn shared_authority_derives_ids_and_rejects_conflicting_or_incomplete_observations() {
    let mut mutations: Vec<(usize, &str, Value)> = vec![
        (
            0,
            "/Stacks/0/StackId",
            json!(config().shared_stack_id.replace("abcdef", "abcdee")),
        ),
        (0, "/Stacks/0/EnableTerminationProtection", json!(false)),
        (0, "/Stacks/0/StackStatus", json!("UPDATE_IN_PROGRESS")),
        (
            1,
            "/TemplateBody/Resources/Vpc/Type",
            json!("AWS::EC2::Subnet"),
        ),
        (
            2,
            "/StackResourceSummaries/0/PhysicalResourceId",
            json!("vpc-33333333333333333"),
        ),
        (
            2,
            "/StackResourceSummaries/0/ResourceType",
            json!("AWS::EC2::Subnet"),
        ),
        (3, "/Subnets/0/OwnerId", json!("123456789013")),
        (3, "/Subnets/0/AvailabilityZone", json!("eu-west-1b")),
        (
            4,
            "/InstanceConnectEndpoints/0/PreserveClientIp",
            json!(true),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/IpAddressType",
            json!("dualstack"),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/State",
            json!("create-in-progress"),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/InstanceConnectEndpointId",
            json!("eice-33333333333333333"),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/DnsName",
            json!("attacker.invalid"),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/SecurityGroupIds",
            json!(["sg-33333333333333333"]),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/NetworkInterfaceIds",
            json!([]),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/NetworkInterfaceIds",
            json!(["eni-11111111111111111", "eni-33333333333333333"]),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/NetworkInterfaceIds",
            json!(["eni-invalid"]),
        ),
        (
            4,
            "/InstanceConnectEndpoints/0/NetworkInterfaceIds",
            json!(["eni-33333333333333333"]),
        ),
        (5, "/NetworkInterfaces/0/InterfaceType", json!("interface")),
        (5, "/NetworkInterfaces/0/InterfaceType", Value::Null),
        (
            5,
            "/NetworkInterfaces/0/NetworkInterfaceId",
            json!("eni-33333333333333333"),
        ),
        (5, "/NetworkInterfaces/0/OwnerId", json!("123456789013")),
        (
            5,
            "/NetworkInterfaces/0/VpcId",
            json!("vpc-33333333333333333"),
        ),
        (
            5,
            "/NetworkInterfaces/0/SubnetId",
            json!("subnet-33333333333333333"),
        ),
        (
            5,
            "/NetworkInterfaces/0/AvailabilityZone",
            json!("eu-west-1b"),
        ),
        (5, "/NetworkInterfaces/0/Status", json!("available")),
        (
            5,
            "/NetworkInterfaces/0/Ipv6Addresses",
            json!([{"Ipv6Address":"2001:db8::1"}]),
        ),
        (5, "/NetworkInterfaces/0/Groups", json!([])),
        (
            5,
            "/NetworkInterfaces/0/Groups",
            json!([{"GroupId":"sg-33333333333333333"}]),
        ),
        (
            5,
            "/NetworkInterfaces/0/Groups",
            json!([{"GroupId":"sg-11111111111111111"},{"GroupId":"sg-33333333333333333"}]),
        ),
        (
            5,
            "/NetworkInterfaces/0/PrivateIpAddress",
            json!("10.78.0.5"),
        ),
        (6, "/SecurityGroups/0/IpPermissions", rule(22, None)),
        (6, "/SecurityGroups/0/IpPermissionsEgress", rule(443, None)),
        (7, "/SecurityGroups/0/IpPermissionsEgress", rule(22, None)),
    ];
    mutations.push((0, "", Value::Null)); // Unmodified happy path.
    for (step, path, value) in mutations {
        let mut responses = shared_responses();
        if !path.is_empty() {
            *responses[step].1.pointer_mut(path).unwrap() = value;
        }
        let mut s = Script::default();
        for (action, value) in responses {
            s.push(action, value);
        }
        let result = verify_shared(&mut aws::tests::aws(&mut s), &config());
        assert_eq!(result.is_ok(), path.is_empty(), "{path}");
        if let Ok(e) = result {
            assert_eq!(e.endpoint_id, endpoint().endpoint_id);
            assert!(s.steps.is_empty());
        }
    }
    for index in 0..8 {
        let mut responses = shared_responses();
        responses[index].1["NextToken"] = json!("incomplete");
        let mut s = Script::default();
        for (action, value) in responses {
            s.push(action, value);
        }
        assert!(verify_shared(&mut aws::tests::aws(&mut s), &config()).is_err());
    }
    for change in ["missing", "extra", "duplicate"] {
        let mut responses = shared_responses();
        let rows = responses[2].1["StackResourceSummaries"]
            .as_array_mut()
            .unwrap();
        match change {
            "missing" => {
                rows.pop();
            }
            "extra" => rows.push(rows[0].clone()),
            _ => rows[1] = rows[0].clone(),
        }
        let mut s = Script::default();
        for (action, value) in responses {
            s.push(action, value);
        }
        assert!(verify_shared(&mut aws::tests::aws(&mut s), &config()).is_err());
    }
}

#[test]
fn frozen_historical_templates_keep_original_digest_and_deletion_authority() {
    // Byte-for-byte retained synthesis from source 86db597ba53b6023ba6a5f37eb9a250d2f13bc42,
    // and actual operation 862a8c18ca76026917f960088d0c721a, source
    // e8574e7bb47442dfb9417597fb901776f5a30ef7. The deployed digest agrees
    // with the retained GetTemplate Original response AND operation.json.
    for (bytes, raw, digest) in [
        (
            include_bytes!("../../../aws/tests/legacy-direct-ssh-template.json").as_slice(),
            "ef990db679e457a04d79b9d249e3401838c1df7144dfaab61e1e7c7c304d7875",
            "6912340031ac8362d8e3e0cd339db0f2588cc60d20008be0b3efb56f8ec1b164",
        ),
        (
            include_bytes!("../../../aws/tests/legacy-direct-ssh-deployed-template.json")
                .as_slice(),
            "76aeb2945cfd22aab4d49824ea79f62933816f6b4d1ba6d8b065fe2f322729bd",
            "bd467104e870245cddfeac1047a5cc0aa5afec10d20b5c94ba9fa36a281f9a9b",
        ),
    ] {
        assert_eq!(template::sha256(bytes), raw);
        let t = template::json(bytes).unwrap();
        assert_eq!(template::digest(&t).unwrap(), digest);
        for mutation in ["none", "userdata", "tag", "resource", "deletion"] {
            let binding = template::binding(&t).unwrap();
            let mut r = Record::new(&binding, digest.into());
            r.creation = aws::state::Creation::Bound;
            // Actual retained StackId suffix for the deployed operation; the
            // synthesis-only fixture never had a deployed stack identity.
            let suffix = if r.run_id == "862a8c18ca76026917f960088d0c721a" {
                "b4e59470-c88b-11f1-af7a-06d03175aea5"
            } else {
                "01234567-89ab-cdef-0123-456789abcdef"
            };
            r.stack_id = Some(format!(
                "arn:aws:cloudformation:eu-west-1:{}:stack/{}/{suffix}",
                r.account, r.stack_name
            ));
            let stack = |status: &str| {
                json!({"Stacks":[{"StackId":r.stack_id,"StackName":r.stack_name,"StackStatus":status,
                "RoleARN":binding.role_arn,"DisableRollback":false,"EnableTerminationProtection":false,"TimeoutInMinutes":10,
                "Tags":binding.tags.iter().map(|(k,v)| json!({"Key":k,"Value":v})).collect::<Vec<_>>()}]})
            };
            let mut observed = t.clone();
            match mutation {
                "userdata" => {
                    observed["Resources"]["Runner"]["Properties"]["UserData"] = json!("changed")
                }
                "tag" => {
                    observed["Metadata"]["BorrowserAg4Operation"]["tags"]["borrowser:purpose"] =
                        json!("other")
                }
                "resource" => {
                    observed["Resources"]
                        .as_object_mut()
                        .unwrap()
                        .remove("RunnerSecurityGroup");
                }
                "deletion" => observed["Resources"]["Runner"]["DeletionPolicy"] = json!("Retain"),
                _ => (),
            }
            let mut s = Script::default();
            s.push("get-caller-identity", json!({"Account":r.account}));
            s.push("describe-stacks", stack("CREATE_COMPLETE"));
            s.push("get-template", json!({"TemplateBody":observed}));
            s.push("list-stack-resources", resources("CREATE_COMPLETE", true));
            s.push("delete-stack", json!({}));
            s.push("describe-stacks", stack("DELETE_COMPLETE"));
            s.push("get-template", json!({"TemplateBody":t}));
            s.push("list-stack-resources", resources("DELETE_COMPLETE", true));
            s.push("describe-instances", json!({"Reservations":[]}));
            s.push("describe-volumes", json!({"Volumes":[]}));
            let (_dir, store) = aws::tests::store();
            store.save(&r).unwrap();
            let result = aws::tests::aws(&mut s).cleanup(&mut r, &store);
            assert_eq!(result.is_ok(), mutation == "none", "{mutation}: {result:?}");
            assert_eq!(s.seen.contains(&"delete-stack".into()), mutation == "none");
            assert!(
                s.seen
                    .iter()
                    .all(|s| !s.contains("endpoint") && !s.contains("tunnel"))
            );
        }
    }
}

#[test]
fn authentication_sanitization_precedes_every_excerpt_and_summary() {
    let env = aws::config::AwsEnvironment::from_lookup(|key| match key {
        "AWS_ACCESS_KEY_ID" => Some("AKIA_SYNTHETIC_ACCESS".into()),
        "AWS_SECRET_ACCESS_KEY" => Some("synthetic/secret+material".into()),
        "AWS_SESSION_TOKEN" => Some("synthetic/token+value=".into()),
        _ => None,
    })
    .unwrap();
    let secrets = [
        "AKIA_SYNTHETIC_ACCESS",
        "synthetic/secret+material",
        "synthetic/token+value=",
    ];
    for text in [
        secrets.join(" "),
        "AKIA_SYNTHETIC_\nACCESS synthetic/secr\net+material synthetic/token+\nvalue=".into(),
        "synthetic%2Fsecret%2Bmaterial synthetic%252Ftoken%252Bvalue%253D".into(),
        "TLS verification failure\nproxy connection failure\nbanner failure\nprocess-shutdown error: wss://endpoint/openTunnel?X-Amz-Credential=OTHER_ACCESS%2Fdate&X-Amz-Signature=SIG_SENTINEL&X-Amz-Security-Token=TOKEN_SENTINEL\nTOKEN_SENTINEL".into(),
        "Authorization: AWS4-HMAC-SHA256 Credential=OTHER_ACCESS, SignedHeaders=host, Signature=SIG_SENTINEL\nbanner failure".into(),
        "Proxy-Authorization: Basic AUTH_SENTINEL\nbanner failure".into(),
        "{\"X-Amz-Security-Token\": \"unterminated\nTOKEN_SENTINEL\n".into(),
        "X-Amz-Signature=first-part\nTOKEN_SENTINEL\n".into(),
        "X-\nAmz-Signature=SIG_SENTINEL".into(),
        "X\\u002dAmz-Signature=SIG_SENTINEL".into(),
    ] {
        let redacted=env.redact(&text);
        for secret in secrets.iter().copied().chain(["OTHER_ACCESS","SIG_SENTINEL","TOKEN_SENTINEL","AUTH_SENTINEL"]) {
            assert!(!redacted.contains(secret),"secret survived sanitization");
        }
        if text.starts_with("AKIA_SYNTHETIC_\n") {
            for fragment in ["AKIA_SYNTHETIC_", "ACCESS", "synthetic/secr", "et+material", "synthetic/token+", "value="] {
                assert!(!redacted.contains(fragment), "wrapped credential fragment survived");
            }
        }
        if text.starts_with("TLS") {
            for benign in ["TLS verification failure","proxy connection failure","banner failure","process-shutdown error"] {
                assert!(redacted.contains(benign),"missing benign evidence {benign}");
            }
        }
    }
    for boundary in [1024, 2048, 8192] {
        let raw = format!("{}{}\nbanner failure", "x".repeat(boundary - 3), secrets[1]);
        let safe = env.redact(&raw);
        let excerpt = safe.chars().take(boundary).collect::<String>();
        assert!(!excerpt.contains("syn"));
    }
    let raw = format!("{}{}", "x".repeat(256 * 1024 - 4), secrets[1]);
    assert_eq!(
        env.redact(&raw),
        "[transport diagnostics withheld: retention limit exceeded]"
    );
    let (_dir, store) = aws::tests::store();
    let mut record = aws::tests::record();
    let safe = env.redact(&format!("{}{}", secrets.join(" "), "é".repeat(10000)));
    aws::cloudformation::record_failure(&mut record, &store, Some(&safe), Some(&safe)).unwrap();
    let saved = store.read().unwrap();
    assert!(saved.primary_failure.unwrap().chars().count() <= 1024);
    assert!(saved.cleanup_failure.unwrap().chars().count() <= 1024);
    assert!(
        fs::metadata(store.path().join("operation.json"))
            .unwrap()
            .len()
            <= 16384
    );
}

#[test]
fn helper_uses_only_the_parent_bound_executable_and_never_reloads_config() {
    use std::process::{Command, Stdio};
    let work = tempfile::tempdir().unwrap();
    let _ = host(&work);
    let root = work.path().canonicalize().unwrap();
    // A regular local argv recorder exercises the exec boundary, not AWS CLI
    // version/network qualification. The separate native test uses the real pin.
    let program = root.join("qualified-aws");
    fs::copy("/bin/echo", &program).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
    let mut c = config();
    c.aws_cli = program.clone();
    c.aws_cli_identity = Some(cli_identity(&program).unwrap());
    let path = TunnelDescriptor::create(&c, &endpoint(), &target(), &root, Duration::from_secs(20))
        .unwrap();
    let expected = TunnelDescriptor::load(&path).unwrap().argv().join(" ");
    let hostile = root.join("aws");
    fs::write(&hostile, "#!/bin/sh\nprintf HOSTILE_EXECUTABLE\n").unwrap();
    fs::set_permissions(&hostile, fs::Permissions::from_mode(0o700)).unwrap();
    c.aws_cli = hostile.clone(); // Parent config changes cannot redirect the frozen handoff.
    fs::write(
        root.join("operator.json"),
        serde_json::to_vec(&json!({"aws_cli":hostile})).unwrap(),
    )
    .unwrap();
    for prompt in ["on", "on-partial"] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "aws::eice::tests::isolated_helper_exec",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("PATH", &root)
            .env("AWS_CLI_AUTO_PROMPT", prompt)
            .env("AG4_TEST_DESCRIPTOR", &path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("bounded helper exec timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains(&expected));
        assert!(!text.contains("HOSTILE_EXECUTABLE"));
        assert!(out.stderr.is_empty());
    }
}

#[test]
#[ignore = "exec-only child of the deterministic helper boundary test"]
fn isolated_helper_exec() {
    let path = std::env::var_os("AG4_TEST_DESCRIPTOR").expect("private test descriptor");
    let d = TunnelDescriptor::load(Path::new(&path)).unwrap();
    // The real helper uses helper_environment(). This isolated child tests the
    // same fixed exec with an adversarial PATH; argv still includes no-auto-prompt.
    d.execute(vec![
        ("PATH".into(), std::env::var_os("PATH").unwrap()),
        (
            "AWS_CLI_AUTO_PROMPT".into(),
            std::env::var_os("AWS_CLI_AUTO_PROMPT").unwrap(),
        ),
    ])
    .unwrap();
    unreachable!();
}

#[test]
fn percent_expansion_and_shell_quoting_preserve_only_two_helper_arguments() {
    use std::process::Command;
    let path = Path::new("/private/tmp/ag4 space ' quote %h $HOME `uname`/descriptor");
    let proxy = proxy_command(Path::new("/bin/echo"), path).unwrap();
    assert!(!proxy.contains("%h") || proxy.contains("%%h"));
    // OpenSSH expands %% before handing the fixed command to its shell.
    let expanded = proxy
        .strip_prefix("ProxyCommand=")
        .unwrap()
        .replace("%%", "%");
    let out = Command::new("/bin/sh")
        .args(["-c", &format!("exec {expanded}")])
        .env_clear()
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("--ag4-eice-proxy {}\n", path.display())
    );
    assert!(proxy_command(Path::new("relative"), path).is_err());
}

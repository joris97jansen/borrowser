//! The one reviewed AG4 EICE path. No discovery, key injection or fallback.
use super::{
    cloudformation::{Aws, array, string},
    command::Commands,
    config::Config,
    state::{Record, Store},
    template,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::Ipv4Addr,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Component, Path, PathBuf},
    process::Command,
    time::Duration,
};

const SHARED: [(&str, &str); 11] = [
    ("Vpc", "AWS::EC2::VPC"),
    ("PublicSubnet", "AWS::EC2::Subnet"),
    ("InternetGateway", "AWS::EC2::InternetGateway"),
    ("GatewayAttachment", "AWS::EC2::VPCGatewayAttachment"),
    ("PublicRoutes", "AWS::EC2::RouteTable"),
    ("InternetRoute", "AWS::EC2::Route"),
    (
        "PublicRouteAssociation",
        "AWS::EC2::SubnetRouteTableAssociation",
    ),
    ("CaptureExecutionRole", "AWS::IAM::Role"),
    (
        "InstanceConnectEndpoint",
        "AWS::EC2::InstanceConnectEndpoint",
    ),
    ("EndpointSecurityGroup", "AWS::EC2::SecurityGroup"),
    (
        "RunnerTransportTargetSecurityGroup",
        "AWS::EC2::SecurityGroup",
    ),
];

#[derive(Clone, PartialEq, Eq)]
pub(super) struct VerifiedEndpoint {
    account: String,
    stack_id: String,
    template_sha256: String,
    endpoint_id: String,
    endpoint_dns: String,
    vpc_id: String,
    subnet_id: String,
    subnet_cidr: String,
    az: String,
    pub endpoint_sg_id: String,
    pub target_sg_id: String,
    resource_ids: BTreeMap<String, String>,
    endpoint_eni: String,
}

#[derive(Clone)]
pub(super) struct VerifiedTarget {
    pub instance_id: String,
    run_id: String,
    stack_id: String,
    primary_eni: String,
    private_ip: Ipv4Addr,
    pub host_key: String,
}

fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn id(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix).is_some_and(|v| hex(v, 17))
}
fn uuid(s: &str) -> bool {
    s.len() == 36
        && s.split('-').zip([8, 4, 4, 4, 12]).all(|(v, n)| hex(v, n))
        && s.split('-').count() == 5
}
pub(super) fn shared_authority(account: &str, stack: &str, digest: &str) -> Result<(), String> {
    let prefix = format!("arn:aws:cloudformation:eu-west-1:{account}:stack/borrowser-ag4-shared/");
    if account.len() != 12
        || !account.bytes().all(|b| b.is_ascii_digit())
        || !stack.strip_prefix(&prefix).is_some_and(uuid)
        || !hex(digest, 64)
    {
        return Err("invalid shared StackId/account/template authority".into());
    }
    Ok(())
}
fn one<'a>(v: &'a Value, key: &str) -> Result<&'a Value, String> {
    let a = array(v, key)?;
    if a.len() != 1 {
        return Err(format!("missing or ambiguous {key}"));
    }
    Ok(&a[0])
}
fn tags(v: &Value) -> Result<BTreeMap<&str, &str>, String> {
    let mut result = BTreeMap::new();
    for t in array(v, "Tags")? {
        if result
            .insert(string(t, "Key")?, string(t, "Value")?)
            .is_some()
        {
            return Err("duplicate ownership tag".into());
        }
    }
    Ok(result)
}
fn shared_tags(v: &Value, stack: &str, logical: &str) -> Result<(), String> {
    let t = tags(v)?;
    for (k, want) in [
        ("borrowser:purpose", "ag4-shared"),
        ("aws:cloudformation:stack-id", stack),
        ("aws:cloudformation:logical-id", logical),
    ] {
        if t.get(k) != Some(&want) {
            return Err("shared ownership tags mismatch".into());
        }
    }
    Ok(())
}
fn cidr(value: &str) -> Result<(u32, u32), String> {
    let (ip, bits) = value.split_once('/').ok_or("invalid runner subnet CIDR")?;
    let ip: Ipv4Addr = ip.parse().map_err(|_| "invalid runner subnet IPv4")?;
    let bits: u32 = bits.parse().map_err(|_| "invalid runner subnet prefix")?;
    if !ip.is_private() || !(16..=28).contains(&bits) {
        return Err("unreviewed runner subnet range".into());
    }
    let mask = u32::MAX << (32 - bits);
    if u32::from(ip) & mask != u32::from(ip) {
        return Err("noncanonical subnet CIDR".into());
    }
    Ok((u32::from(ip), mask))
}
fn private_ip(ip: &str, subnet: &str) -> Result<Ipv4Addr, String> {
    let ip: Ipv4Addr = ip.parse().map_err(|_| "invalid private target IPv4")?;
    let (network, mask) = cidr(subnet)?;
    if !ip.is_private() || u32::from(ip) & mask != network {
        return Err("target outside reviewed subnet".into());
    }
    Ok(ip)
}
fn group_set(v: &Value, expected: &[&str]) -> Result<(), String> {
    let rows = v.as_array().ok_or("missing interface security groups")?;
    let actual: BTreeSet<_> = rows
        .iter()
        .map(|v| string(v, "GroupId"))
        .collect::<Result<_, _>>()?;
    if actual.len() != rows.len() || actual != expected.iter().copied().collect() {
        return Err("interface security-group set mismatch".into());
    }
    Ok(())
}

// Compare all effective permissions, rejecting extra ranges, groups and rules.
fn permission(v: &Value, port: u64, group: Option<(&str, &str)>) -> Result<(), String> {
    let rows = v.as_array().ok_or("missing security-group permissions")?;
    if rows.len() != 1 {
        return Err("unexpected security-group permission count".into());
    }
    let p = &rows[0];
    if p["IpProtocol"] != "tcp"
        || p["FromPort"].as_u64() != Some(port)
        || p["ToPort"].as_u64() != Some(port)
        || !array(p, "Ipv6Ranges")?.is_empty()
        || !array(p, "PrefixListIds")?.is_empty()
    {
        return Err("security-group protocol/port/range mismatch".into());
    }
    let ranges = array(p, "IpRanges")?;
    let groups = array(p, "UserIdGroupPairs")?;
    match group {
        Some((id, account))
            if ranges.is_empty()
                && groups.len() == 1
                && groups[0]["GroupId"] == id
                && groups[0]["UserId"] == account
                && groups[0].get("VpcPeeringConnectionId").is_none() =>
        {
            Ok(())
        }
        None if groups.is_empty() && ranges.len() == 1 && ranges[0]["CidrIp"] == "0.0.0.0/0" => {
            Ok(())
        }
        _ => Err("security-group destination/source mismatch".into()),
    }
}
fn observe_group<C: Commands>(
    aws: &mut Aws<'_, C>,
    group: &str,
    account: &str,
    vpc: &str,
) -> Result<Value, String> {
    let response = aws.call(
        "ec2",
        "describe-security-groups",
        &["--group-ids".into(), group.into()],
    )?;
    let g = one(&response, "SecurityGroups")?;
    if g["GroupId"] != group || g["OwnerId"] != account || g["VpcId"] != vpc {
        return Err("security-group identity mismatch".into());
    }
    Ok(g.clone())
}

pub(super) fn verify_shared<C: Commands>(
    aws: &mut Aws<'_, C>,
    c: &Config,
) -> Result<VerifiedEndpoint, String> {
    shared_authority(&c.account, &c.shared_stack_id, &c.shared_template_sha256)?;
    let selector = ["--stack-name".into(), c.shared_stack_id.clone()];
    let response = aws.call("cloudformation", "describe-stacks", &selector)?;
    let stack = one(&response, "Stacks")?;
    if stack["StackId"] != c.shared_stack_id
        || stack["StackName"] != "borrowser-ag4-shared"
        || stack["EnableTerminationProtection"] != true
        || !matches!(
            stack["StackStatus"].as_str(),
            Some("CREATE_COMPLETE" | "UPDATE_COMPLETE")
        )
    {
        return Err("shared stack identity/state/protection mismatch".into());
    }
    let response = aws.call(
        "cloudformation",
        "get-template",
        &[
            selector[0].clone(),
            selector[1].clone(),
            "--template-stage".into(),
            "Original".into(),
        ],
    )?;
    let t = template::original(
        &serde_json::to_vec(&response).map_err(|_| "invalid template envelope")?,
    )?;
    if template::digest(&t)? != c.shared_template_sha256 {
        return Err("shared template digest mismatch".into());
    }
    let definitions = t["Resources"]
        .as_object()
        .ok_or("missing shared resources")?;
    if definitions.len() != SHARED.len()
        || SHARED
            .iter()
            .any(|(name, ty)| definitions.get(*name).is_none_or(|r| r["Type"] != *ty))
    {
        return Err("shared template resource boundary mismatch".into());
    }
    let response = aws.call("cloudformation", "list-stack-resources", &selector)?;
    let rows = array(&response, "StackResourceSummaries")?;
    let mut ids = BTreeMap::new();
    let mut physical = BTreeSet::new();
    if rows.len() != SHARED.len() {
        return Err("shared physical resource boundary mismatch".into());
    }
    for row in rows {
        let logical = string(row, "LogicalResourceId")?;
        let expected = SHARED
            .iter()
            .find(|(name, _)| *name == logical)
            .ok_or("unrelated shared resource")?
            .1;
        let actual = string(row, "PhysicalResourceId")?;
        if row["ResourceType"] != expected
            || actual.is_empty()
            || actual.len() > 2048
            || actual.chars().any(char::is_control)
            || !matches!(
                row["ResourceStatus"].as_str(),
                Some("CREATE_COMPLETE" | "UPDATE_COMPLETE")
            )
            || ids.insert(logical, actual).is_some()
            || !physical.insert(actual)
        {
            return Err("conflicting shared physical resource identity".into());
        }
    }
    let get = |name: &str| ids.get(name).copied().ok_or("missing shared resource");
    if get("Vpc")? != c.vpc_id
        || get("PublicSubnet")? != c.subnet_id
        || c.cloudformation_role_arn
            != format!(
                "arn:aws:iam::{}:role/borrowser/{}",
                c.account,
                get("CaptureExecutionRole")?
            )
    {
        return Err("shared network/execution-role binding mismatch".into());
    }
    let endpoint_id = get("InstanceConnectEndpoint")?;
    let endpoint_sg = get("EndpointSecurityGroup")?;
    let target_sg = get("RunnerTransportTargetSecurityGroup")?;
    if !id(endpoint_id, "eice-") || !id(endpoint_sg, "sg-") || !id(target_sg, "sg-") {
        return Err("malformed shared transport identities".into());
    }
    let response = aws.call(
        "ec2",
        "describe-subnets",
        &["--subnet-ids".into(), c.subnet_id.clone()],
    )?;
    let subnet = one(&response, "Subnets")?;
    let subnet_cidr = string(subnet, "CidrBlock")?;
    cidr(subnet_cidr)?;
    let az = string(subnet, "AvailabilityZone")?;
    if subnet["SubnetId"] != c.subnet_id
        || subnet["VpcId"] != c.vpc_id
        || subnet["OwnerId"] != c.account
        || subnet["State"] != "available"
        || subnet["Ipv6Native"] != false
        || !array(subnet, "Ipv6CidrBlockAssociationSet")?.is_empty()
        || t["Resources"]["PublicSubnet"]["Properties"]["AvailabilityZone"] != az
        || t["Resources"]["PublicSubnet"]["Properties"]["CidrBlock"] != subnet_cidr
    {
        return Err("shared subnet/AZ/IPv4 mismatch".into());
    }
    let response = aws.call(
        "ec2",
        "describe-instance-connect-endpoints",
        &["--instance-connect-endpoint-ids".into(), endpoint_id.into()],
    )?;
    let endpoint = one(&response, "InstanceConnectEndpoints")?;
    let arn = format!(
        "arn:aws:ec2:eu-west-1:{}:instance-connect-endpoint/{endpoint_id}",
        c.account
    );
    if endpoint["InstanceConnectEndpointId"] != endpoint_id
        || endpoint["InstanceConnectEndpointArn"] != arn
        || endpoint["OwnerId"] != c.account
        || endpoint["VpcId"] != c.vpc_id
        || endpoint["SubnetId"] != c.subnet_id
        || endpoint["AvailabilityZone"] != az
        || endpoint["IpAddressType"] != "ipv4"
        || endpoint["PreserveClientIp"] != false
        || endpoint["SecurityGroupIds"] != json!([endpoint_sg])
        || !matches!(
            endpoint["State"].as_str(),
            Some("create-complete" | "update-complete")
        )
    {
        return Err("EICE identity/state/network mismatch".into());
    }
    shared_tags(endpoint, &c.shared_stack_id, "InstanceConnectEndpoint")?;
    let dns = string(endpoint, "DnsName")?;
    endpoint_dns(dns, endpoint_id)?;
    let enis = array(endpoint, "NetworkInterfaceIds")?;
    if enis.len() != 1 {
        return Err("unexpected EICE interface count".into());
    }
    let eni_id = enis[0]
        .as_str()
        .filter(|s| id(s, "eni-"))
        .ok_or("invalid EICE interface identity")?;
    let response = aws.call(
        "ec2",
        "describe-network-interfaces",
        &["--network-interface-ids".into(), eni_id.into()],
    )?;
    let eni = one(&response, "NetworkInterfaces")?;
    if eni["NetworkInterfaceId"] != eni_id
        || eni["OwnerId"] != c.account
        || eni["VpcId"] != c.vpc_id
        || eni["SubnetId"] != c.subnet_id
        || eni["AvailabilityZone"] != az
        || eni["InterfaceType"] != "ec2_instance_connect_endpoint"
        || eni["Status"] != "in-use"
        || !array(eni, "Ipv6Addresses")?.is_empty()
    {
        return Err("EICE interface binding mismatch".into());
    }
    private_ip(string(eni, "PrivateIpAddress")?, subnet_cidr)?;
    group_set(&eni["Groups"], &[endpoint_sg])?;
    for (group, logical) in [
        (endpoint_sg, "EndpointSecurityGroup"),
        (target_sg, "RunnerTransportTargetSecurityGroup"),
    ] {
        let g = observe_group(aws, group, &c.account, &c.vpc_id)?;
        shared_tags(&g, &c.shared_stack_id, logical)?;
        if !array(&g, "IpPermissions")?.is_empty() {
            return Err("shared group has ingress".into());
        }
        permission(
            &g["IpPermissionsEgress"],
            if group == endpoint_sg { 22 } else { 443 },
            (group == endpoint_sg).then_some((target_sg, c.account.as_str())),
        )?;
    }
    Ok(VerifiedEndpoint {
        account: c.account.clone(),
        stack_id: c.shared_stack_id.clone(),
        template_sha256: c.shared_template_sha256.clone(),
        endpoint_id: endpoint_id.into(),
        endpoint_dns: dns.into(),
        vpc_id: c.vpc_id.clone(),
        subnet_id: c.subnet_id.clone(),
        subnet_cidr: subnet_cidr.into(),
        az: az.into(),
        endpoint_sg_id: endpoint_sg.into(),
        target_sg_id: target_sg.into(),
        resource_ids: ids
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect(),
        endpoint_eni: eni_id.into(),
    })
}

impl VerifiedEndpoint {
    pub fn verify_target<C: Commands>(
        &self,
        aws: &mut Aws<'_, C>,
        r: &mut Record,
        store: &Store,
        instance: &Value,
        host_key: &str,
    ) -> Result<VerifiedTarget, String> {
        let instance_id = r.instance_id.as_deref().ok_or("unbound runner")?.to_owned();
        let stack_id = r.stack_id.clone().ok_or("unbound run stack")?;
        let rows = aws.resources(r, store)?;
        if rows.len() != 2
            || rows
                .iter()
                .any(|r| r["ResourceStatus"] != "CREATE_COMPLETE")
        {
            return Err("incomplete live run resources".into());
        }
        let sg = rows
            .iter()
            .find(|v| v["LogicalResourceId"] == "RunnerSecurityGroup")
            .and_then(|v| v["PhysicalResourceId"].as_str())
            .filter(|v| id(v, "sg-"))
            .ok_or("missing run security group")?;
        if !id(&instance_id, "i-")
            || r.account != self.account
            || instance["InstanceId"] != instance_id
            || instance["VpcId"] != self.vpc_id
            || instance["SubnetId"] != self.subnet_id
            || instance["Placement"]["AvailabilityZone"] != self.az
            || instance["State"]["Name"] != "running"
            || instance.get("IamInstanceProfile").is_some()
        {
            return Err("runner target identity mismatch".into());
        }
        let g = observe_group(aws, sg, &self.account, &self.vpc_id)?;
        for v in [instance, &g] {
            let t = tags(v)?;
            for (k, want) in [
                ("borrowser:purpose", "ag4"),
                ("borrowser:run-id", r.run_id.as_str()),
                ("borrowser:stack-id", stack_id.as_str()),
            ] {
                if t.get(k) != Some(&want) {
                    return Err("runner ownership mismatch".into());
                }
            }
        }
        permission(
            &g["IpPermissions"],
            22,
            Some((&self.endpoint_sg_id, &self.account)),
        )?;
        permission(&g["IpPermissionsEgress"], 443, None)?;
        group_set(&instance["SecurityGroups"], &[sg, &self.target_sg_id])?;
        let interface = one(instance, "NetworkInterfaces")?;
        let eni_id = string(interface, "NetworkInterfaceId")?;
        if !id(eni_id, "eni-") || interface["Attachment"]["DeviceIndex"] != 0 {
            return Err("runner primary interface mismatch".into());
        }
        let response = aws.call(
            "ec2",
            "describe-network-interfaces",
            &["--network-interface-ids".into(), eni_id.into()],
        )?;
        let eni = one(&response, "NetworkInterfaces")?;
        if eni["NetworkInterfaceId"] != eni_id
            || eni["OwnerId"] != self.account
            || eni["VpcId"] != self.vpc_id
            || eni["SubnetId"] != self.subnet_id
            || eni["AvailabilityZone"] != self.az
            || eni["Status"] != "in-use"
            || eni["Attachment"]["InstanceId"] != instance_id
            || eni["Attachment"]["DeviceIndex"] != 0
            || eni["Attachment"]["DeleteOnTermination"] != true
            || eni["Attachment"]["Status"] != "attached"
            || eni["RequesterManaged"] != false
        {
            return Err("runner interface attachment mismatch".into());
        }
        for v in [interface, eni] {
            group_set(&v["Groups"], &[sg, &self.target_sg_id])?;
        }
        let address = string(eni, "PrivateIpAddress")?;
        if interface["PrivateIpAddress"] != address || instance["PrivateIpAddress"] != address {
            return Err("conflicting runner private address".into());
        }
        let primary = array(eni, "PrivateIpAddresses")?
            .iter()
            .filter(|v| v["Primary"] == true)
            .collect::<Vec<_>>();
        if primary.len() != 1 || primary[0]["PrivateIpAddress"] != address {
            return Err("ambiguous primary IPv4".into());
        }
        Ok(VerifiedTarget {
            instance_id,
            run_id: r.run_id.clone(),
            stack_id,
            primary_eni: eni_id.into(),
            private_ip: private_ip(address, &self.subnet_cidr)?,
            host_key: host_key.into(),
        })
    }
}

fn endpoint_dns(s: &str, endpoint_id: &str) -> Result<(), String> {
    // Validate the authenticated value; never construct or select a hostname.
    // The ordinary EC2 Instance Connect API hostname is a different service.
    let prefix = s
        .strip_suffix(".ec2-instance-connect-endpoint.eu-west-1.amazonaws.com")
        .and_then(|prefix| prefix.split_once('.'));
    if s.len() > 253
        || !id(endpoint_id, "eice-")
        || !prefix.is_some_and(|(first, opaque)| first == endpoint_id && !opaque.contains('.'))
        || s.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    {
        return Err("invalid authenticated EICE DNS".into());
    }
    Ok(())
}

pub(super) fn validate_live_template(
    t: &Value,
    c: &Config,
    e: &VerifiedEndpoint,
) -> Result<(), String> {
    template::binding(t)?;
    let sg = &t["Resources"]["RunnerSecurityGroup"]["Properties"];
    let eni = &t["Resources"]["Runner"]["Properties"]["NetworkInterfaces"][0];
    if sg["VpcId"] != c.vpc_id
        || sg["SecurityGroupIngress"]
            != json!([{
        "IpProtocol":"tcp","FromPort":22,"ToPort":22,"SourceSecurityGroupId":e.endpoint_sg_id}])
        || sg["SecurityGroupEgress"]
            != json!([{"IpProtocol":"tcp","FromPort":443,"ToPort":443,"CidrIp":"0.0.0.0/0"}])
        || eni["SubnetId"] != c.subnet_id
        || eni["DeviceIndex"] != "0"
        || eni["GroupSet"]
            != json!([{"Fn::GetAtt":["RunnerSecurityGroup","GroupId"]},e.target_sg_id])
        || eni["AssociatePublicIpAddress"] != true
        || t["Resources"]["Runner"]["Properties"]
            .get("IamInstanceProfile")
            .is_some()
        || t["Resources"]["Runner"]["Properties"]
            .get("KeyName")
            .is_some()
    {
        return Err("live capture requires the exact EICE topology".into());
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<(), String> {
    let s = path.to_str().ok_or("non-UTF8 private execution path")?;
    if !path.is_absolute()
        || s.len() > 4096
        || s.chars().any(char::is_control)
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err("canonical absolute execution path required".into());
    }
    Ok(())
}
fn no_symlinks(path: &Path) -> Result<(), String> {
    absolute(path)?;
    // No canonicalization in the helper: a link is refused, never substituted.
    for p in path.ancestors() {
        if fs::symlink_metadata(p)
            .map_err(|_| "execution path unavailable")?
            .file_type()
            .is_symlink()
        {
            return Err("symlink in private execution path".into());
        }
    }
    if path
        .canonicalize()
        .map_err(|_| "execution path unavailable")?
        .as_os_str()
        != path.as_os_str()
    {
        return Err("noncanonical execution path".into());
    }
    Ok(())
}
pub(super) fn cli_identity(path: &Path) -> Result<(u64, u64, u64, String), String> {
    no_symlinks(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "AWS CLI executable unavailable")?;
    let before = file
        .metadata()
        .map_err(|_| "AWS CLI metadata unavailable")?;
    if !before.is_file()
        || before.mode() & 0o111 == 0
        || before.mode() & 0o022 != 0
        || ![0, unsafe { libc::geteuid() }].contains(&before.uid())
        || before.len() == 0
        || before.len() > 256 * 1024 * 1024
    {
        return Err("expected trusted regular AWS CLI executable".into());
    }
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    let mut count = 0;
    loop {
        let n = file.read(&mut bytes).map_err(|_| "AWS CLI read failed")?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > before.len() {
            return Err("AWS CLI changed while hashing".into());
        }
        hash.update(&bytes[..n]);
    }
    let after = file
        .metadata()
        .map_err(|_| "AWS CLI metadata unavailable")?;
    let named = fs::symlink_metadata(path).map_err(|_| "AWS CLI path changed")?;
    if !named.is_file()
        || named.dev() != before.dev()
        || named.ino() != before.ino()
        || count != before.len()
        || after.len() != before.len()
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
    {
        return Err("AWS CLI substituted during verification".into());
    }
    Ok((
        before.dev(),
        before.ino(),
        count,
        format!("{:x}", hash.finalize()),
    ))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TunnelDescriptor {
    aws_cli_path: PathBuf,
    aws_cli_device: u64,
    aws_cli_inode: u64,
    aws_cli_length: u64,
    aws_cli_sha256: String,
    account: String,
    shared_stack_id: String,
    shared_template_sha256: String,
    endpoint_id: String,
    endpoint_dns: String,
    run_id: String,
    run_stack_id: String,
    instance_id: String,
    primary_eni: String,
    private_ip: Ipv4Addr,
    subnet_cidr: String,
    max_tunnel_duration: u64,
}
impl TunnelDescriptor {
    pub fn create(
        config: &Config,
        endpoint: &VerifiedEndpoint,
        target: &VerifiedTarget,
        work: &Path,
        timeout: Duration,
    ) -> Result<PathBuf, String> {
        let (device, inode, length, sha256) = config
            .aws_cli_identity
            .as_ref()
            .ok_or("AWS CLI not parent-qualified")?;
        let d = Self {
            aws_cli_path: config.aws_cli.clone(),
            aws_cli_device: *device,
            aws_cli_inode: *inode,
            aws_cli_length: *length,
            aws_cli_sha256: sha256.clone(),
            account: endpoint.account.clone(),
            shared_stack_id: endpoint.stack_id.clone(),
            shared_template_sha256: endpoint.template_sha256.clone(),
            endpoint_id: endpoint.endpoint_id.clone(),
            endpoint_dns: endpoint.endpoint_dns.clone(),
            run_id: target.run_id.clone(),
            run_stack_id: target.stack_id.clone(),
            instance_id: target.instance_id.clone(),
            primary_eni: target.primary_eni.clone(),
            private_ip: target.private_ip,
            subnet_cidr: endpoint.subnet_cidr.clone(),
            max_tunnel_duration: timeout.as_secs().min(300),
        };
        d.validate()?;
        private_directory(work)?;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| "descriptor nonce unavailable")?;
        let path = work.join(format!("eice-{}.json", template::sha256(&nonce)));
        let bytes = serde_json::to_vec(&d).map_err(|_| "descriptor encoding failed")?;
        if bytes.len() > 16384 {
            return Err("descriptor too large".into());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| "descriptor creation failed")?;
        file.write_all(&bytes)
            .map_err(|_| "descriptor write failed")?;
        Ok(path)
    }
    fn validate(&self) -> Result<(), String> {
        absolute(&self.aws_cli_path)?;
        shared_authority(
            &self.account,
            &self.shared_stack_id,
            &self.shared_template_sha256,
        )?;
        endpoint_dns(&self.endpoint_dns, &self.endpoint_id)?;
        private_ip(&self.private_ip.to_string(), &self.subnet_cidr)?;
        let prefix = format!(
            "arn:aws:cloudformation:eu-west-1:{}:stack/borrowser-ag4-{}/",
            self.account, self.run_id
        );
        if !hex(&self.run_id, 32)
            || !self.run_stack_id.strip_prefix(&prefix).is_some_and(uuid)
            || !id(&self.instance_id, "i-")
            || !id(&self.primary_eni, "eni-")
            || !id(&self.endpoint_id, "eice-")
            || !hex(&self.aws_cli_sha256, 64)
            || self.aws_cli_length == 0
            || self.aws_cli_length > 256 * 1024 * 1024
            || !(1..=300).contains(&self.max_tunnel_duration)
        {
            return Err("invalid private EICE descriptor".into());
        }
        Ok(())
    }
    fn load(path: &Path) -> Result<Self, String> {
        no_symlinks(path)?;
        private_directory(path.parent().ok_or("descriptor directory required")?)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| "descriptor unavailable")?;
        let meta = file
            .metadata()
            .map_err(|_| "descriptor metadata unavailable")?;
        if !meta.is_file()
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o777 != 0o600
            || meta.nlink() != 1
        {
            return Err("private regular mode-0600 descriptor required".into());
        }
        let mut bytes = vec![];
        file.take(16385)
            .read_to_end(&mut bytes)
            .map_err(|_| "descriptor read failed")?;
        if bytes.len() > 16384 {
            return Err("descriptor too large".into());
        }
        let d: Self =
            serde_json::from_value(template::json(&bytes).map_err(|_| "invalid descriptor JSON")?)
                .map_err(|_| "invalid descriptor fields")?;
        d.validate()?;
        Ok(d)
    }
    fn argv(&self) -> Vec<String> {
        vec![
            "--region".into(),
            "eu-west-1".into(),
            "--no-cli-pager".into(),
            "--no-cli-auto-prompt".into(),
            "ec2-instance-connect".into(),
            "open-tunnel".into(),
            "--instance-connect-endpoint-id".into(),
            self.endpoint_id.clone(),
            "--instance-connect-endpoint-dns-name".into(),
            self.endpoint_dns.clone(),
            "--private-ip-address".into(),
            self.private_ip.to_string(),
            "--remote-port".into(),
            "22".into(),
            "--max-tunnel-duration".into(),
            self.max_tunnel_duration.to_string(),
        ]
    }
    fn verify_cli(&self) -> Result<(), String> {
        if cli_identity(&self.aws_cli_path)?
            != (
                self.aws_cli_device,
                self.aws_cli_inode,
                self.aws_cli_length,
                self.aws_cli_sha256.clone(),
            )
        {
            return Err("AWS CLI differs from parent-qualified executable".into());
        }
        Ok(())
    }
    fn execute(&self, env: Vec<(std::ffi::OsString, std::ffi::OsString)>) -> Result<(), String> {
        self.verify_cli()?;
        // Pathname exec assumes a trusted, stable local CLI installation
        // including its bundle. No same-UID/root writer may mutate it between
        // this check and exec; this is not an atomic pathname guarantee.
        let error = Command::new(&self.aws_cli_path)
            .args(self.argv())
            .env_clear()
            .envs(env)
            .exec();
        Err(format!("EICE AWS CLI exec failed: {error}"))
    }
}
fn private_directory(path: &Path) -> Result<(), String> {
    no_symlinks(path)?;
    let m = fs::symlink_metadata(path).map_err(|_| "private directory unavailable")?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o777 != 0o700 {
        return Err("owned mode-0700 invocation directory required".into());
    }
    Ok(())
}
pub(super) fn proxy_command(controller: &Path, descriptor: &Path) -> Result<String, String> {
    fn quote(p: &Path) -> Result<String, String> {
        absolute(p)?;
        Ok(format!(
            "'{}'",
            p.to_str()
                .unwrap()
                .replace('%', "%%")
                .replace('\'', "'\\''")
        ))
    }
    Ok(format!(
        "ProxyCommand={} --ag4-eice-proxy {}",
        quote(controller)?,
        quote(descriptor)?
    ))
}
pub(super) fn proxy(args: &[String]) -> Result<(), String> {
    if args.len() != 1
        || super::artifact::identity().source_tree.is_none()
        || [0, 1, 2].iter().any(|fd| unsafe { libc::isatty(*fd) } != 0)
    {
        return Err("EICE helper requires stamped controller and owned nonterminal streams".into());
    }
    let d = TunnelDescriptor::load(Path::new(&args[0]))?;
    let env = super::config::AwsEnvironment::helper_environment()?;
    d.execute(env)
}

#[cfg(test)]
pub(super) mod tests;

//! Invocation-scoped EC2 structural integrity. The SDK alone decodes semantic XML values.
use super::response_limits::ObservationRound;
use crate::{
    Error, Result,
    provider::{
        coverage::ReadOperationV1,
        limits::{LimitKind, RECORDS, RESPONSE_BYTES},
    },
    require,
};
use aws_sdk_ec2::operation::{
    describe_availability_zones as zones, describe_dhcp_options as dhcp,
    describe_network_acls as acls, describe_prefix_lists as prefixes, describe_regions as regions,
    describe_route_tables as tables, describe_security_groups as groups,
    describe_subnets as subnets, describe_vpc_attribute as attribute,
    describe_vpc_endpoints as endpoints, describe_vpcs as vpcs,
};
use aws_smithy_runtime_api::client::{
    interceptors::{
        Intercept,
        context::{
            AfterDeserializationInterceptorContextRef, BeforeDeserializationInterceptorContextRef,
            BeforeSerializationInterceptorContextRef, FinalizerInterceptorContextRef,
        },
    },
    runtime_components::RuntimeComponents,
};
use aws_smithy_types::config_bag::ConfigBag;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use xmlparser::{ElementEnd, Token};

#[derive(Clone)]
enum Shape {
    Scalar,
    Object(Vec<(&'static str, Shape)>),
    List(Box<Shape>),
}
#[derive(Debug, PartialEq, Eq)]
enum Presence {
    Scalar(bool),
    Object(BTreeMap<&'static str, Presence>),
    List(Vec<Presence>),
}
impl Shape {
    fn empty(&self) -> Presence {
        match self {
            Self::Scalar => Presence::Scalar(false),
            Self::Object(_) => Presence::Object(BTreeMap::new()),
            Self::List(_) => Presence::List(Vec::new()),
        }
    }
}
trait WireEvidence {
    fn shape() -> Shape;
    fn presence(&self) -> Presence;
}
impl WireEvidence for String {
    fn shape() -> Shape {
        Shape::Scalar
    }
    fn presence(&self) -> Presence {
        Presence::Scalar(!self.is_empty())
    }
}
macro_rules! wire_numbers { ($($ty:ty),*) => {$(impl WireEvidence for $ty {
    fn shape() -> Shape { Shape::Scalar }
    fn presence(&self) -> Presence { Presence::Scalar(true) }
})*}; }
wire_numbers!(bool, i32);
impl<T: WireEvidence> WireEvidence for Vec<T> {
    fn shape() -> Shape {
        Shape::List(Box::new(T::shape()))
    }
    fn presence(&self) -> Presence {
        Presence::List(self.iter().map(WireEvidence::presence).collect())
    }
}
macro_rules! wire_enums { ($($ty:ty),*) => {$(impl WireEvidence for $ty {
    fn shape() -> Shape { Shape::Scalar }
    fn presence(&self) -> Presence { Presence::Scalar(!self.as_str().is_empty()) }
})*}; }
macro_rules! wire_object {
    ($ty:ty { $($field:ident: $member:ty => $wire:literal),* $(,)? }) => {
        impl WireEvidence for $ty {
            fn shape() -> Shape { Shape::Object(vec![$(($wire, <$member>::shape())),*]) }
            fn presence(&self) -> Presence {
                let mut fields = BTreeMap::new();
                $(if let Some(value) = &self.$field { fields.insert($wire, value.presence()); })*
                Presence::Object(fields)
            }
        }
    };
}
wire_object!(aws_sdk_ec2::types::Region {
    region_name: ::std::string::String => "regionName" ,
    opt_in_status: ::std::string::String => "optInStatus" ,
});
wire_object!(aws_sdk_ec2::types::AvailabilityZone {
    zone_name: ::std::string::String => "zoneName" ,
    zone_id: ::std::string::String => "zoneId" ,
    region_name: ::std::string::String => "regionName" ,
    state: aws_sdk_ec2::types::AvailabilityZoneState => "zoneState" ,
});
wire_object!(aws_sdk_ec2::types::Subnet {
    subnet_id: ::std::string::String => "subnetId" ,
    owner_id: ::std::string::String => "ownerId" ,
    vpc_id: ::std::string::String => "vpcId" ,
    availability_zone: ::std::string::String => "availabilityZone" ,
    availability_zone_id: ::std::string::String => "availabilityZoneId" ,
    cidr_block: ::std::string::String => "cidrBlock" ,
    ipv6_native: bool => "ipv6Native" ,
    assign_ipv6_address_on_creation: bool => "assignIpv6AddressOnCreation" ,
    map_public_ip_on_launch: bool => "mapPublicIpOnLaunch" ,
    outpost_arn: ::std::string::String => "outpostArn" ,
    customer_owned_ipv4_pool: ::std::string::String => "customerOwnedIpv4Pool" ,
});
wire_object!(aws_sdk_ec2::types::Vpc {
    vpc_id: ::std::string::String => "vpcId" ,
    owner_id: ::std::string::String => "ownerId" ,
    instance_tenancy: aws_sdk_ec2::types::Tenancy => "instanceTenancy" ,
    dhcp_options_id: ::std::string::String => "dhcpOptionsId" ,
});
wire_object!(aws_sdk_ec2::types::SecurityGroup {
    group_id: ::std::string::String => "groupId" ,
    owner_id: ::std::string::String => "ownerId" ,
    vpc_id: ::std::string::String => "vpcId" ,
    ip_permissions: ::std::vec::Vec<aws_sdk_ec2::types::IpPermission> => "ipPermissions" ,
    ip_permissions_egress: ::std::vec::Vec<aws_sdk_ec2::types::IpPermission> => "ipPermissionsEgress" ,
});
wire_object!(aws_sdk_ec2::types::IpPermission {
    ip_protocol: ::std::string::String => "ipProtocol" ,
    from_port: i32 => "fromPort" ,
    to_port: i32 => "toPort" ,
    user_id_group_pairs: ::std::vec::Vec<aws_sdk_ec2::types::UserIdGroupPair> => "groups" ,
    ip_ranges: ::std::vec::Vec<aws_sdk_ec2::types::IpRange> => "ipRanges" ,
    ipv6_ranges: ::std::vec::Vec<aws_sdk_ec2::types::Ipv6Range> => "ipv6Ranges" ,
    prefix_list_ids: ::std::vec::Vec<aws_sdk_ec2::types::PrefixListId> => "prefixListIds" ,
});
wire_object!(aws_sdk_ec2::types::UserIdGroupPair {
    user_id: ::std::string::String => "userId" ,
    vpc_id: ::std::string::String => "vpcId" ,
    group_id: ::std::string::String => "groupId" ,
    group_name: ::std::string::String => "groupName" ,
    description: ::std::string::String => "description" ,
    vpc_peering_connection_id: ::std::string::String => "vpcPeeringConnectionId" ,
    peering_status: ::std::string::String => "peeringStatus" ,
});
wire_object!(aws_sdk_ec2::types::IpRange {
    cidr_ip: ::std::string::String => "cidrIp" ,
    description: ::std::string::String => "description" ,
});
wire_object!(aws_sdk_ec2::types::Ipv6Range {
    cidr_ipv6: ::std::string::String => "cidrIpv6" ,
    description: ::std::string::String => "description" ,
});
wire_object!(aws_sdk_ec2::types::PrefixListId {
    prefix_list_id: ::std::string::String => "prefixListId" ,
    description: ::std::string::String => "description" ,
});
wire_object!(aws_sdk_ec2::types::RouteTable {
    route_table_id: ::std::string::String => "routeTableId" ,
    owner_id: ::std::string::String => "ownerId" ,
    vpc_id: ::std::string::String => "vpcId" ,
    associations: ::std::vec::Vec<aws_sdk_ec2::types::RouteTableAssociation> => "associationSet" ,
    routes: ::std::vec::Vec<aws_sdk_ec2::types::Route> => "routeSet" ,
});
wire_object!(aws_sdk_ec2::types::Route {
    destination_cidr_block: ::std::string::String => "destinationCidrBlock" ,
    destination_ipv6_cidr_block: ::std::string::String => "destinationIpv6CidrBlock" ,
    destination_prefix_list_id: ::std::string::String => "destinationPrefixListId" ,
    gateway_id: ::std::string::String => "gatewayId" ,
    egress_only_internet_gateway_id: ::std::string::String => "egressOnlyInternetGatewayId" ,
    nat_gateway_id: ::std::string::String => "natGatewayId" ,
    transit_gateway_id: ::std::string::String => "transitGatewayId" ,
    local_gateway_id: ::std::string::String => "localGatewayId" ,
    carrier_gateway_id: ::std::string::String => "carrierGatewayId" ,
    instance_id: ::std::string::String => "instanceId" ,
    network_interface_id: ::std::string::String => "networkInterfaceId" ,
    vpc_peering_connection_id: ::std::string::String => "vpcPeeringConnectionId" ,
    core_network_arn: ::std::string::String => "coreNetworkArn" ,
    odb_network_arn: ::std::string::String => "odbNetworkArn" ,
    ip_address: ::std::string::String => "ipAddress" ,
    instance_owner_id: ::std::string::String => "instanceOwnerId" ,
    state: aws_sdk_ec2::types::RouteState => "state" ,
    origin: aws_sdk_ec2::types::RouteOrigin => "origin" ,
});
wire_object!(aws_sdk_ec2::types::RouteTableAssociation {
    route_table_association_id: ::std::string::String => "routeTableAssociationId" ,
    route_table_id: ::std::string::String => "routeTableId" ,
    subnet_id: ::std::string::String => "subnetId" ,
    gateway_id: ::std::string::String => "gatewayId" ,
    main: bool => "main" ,
    public_ipv4_pool: ::std::string::String => "publicIpv4Pool" ,
    association_state: aws_sdk_ec2::types::RouteTableAssociationState => "associationState" ,
});
wire_object!(aws_sdk_ec2::types::RouteTableAssociationState {
    state: aws_sdk_ec2::types::RouteTableAssociationStateCode => "state" ,
    status_message: ::std::string::String => "statusMessage" ,
});
wire_object!(aws_sdk_ec2::types::VpcEndpoint {
    vpc_endpoint_id: ::std::string::String => "vpcEndpointId" ,
    owner_id: ::std::string::String => "ownerId" ,
    vpc_id: ::std::string::String => "vpcId" ,
    service_name: ::std::string::String => "serviceName" ,
    vpc_endpoint_type: aws_sdk_ec2::types::VpcEndpointType => "vpcEndpointType" ,
    state: aws_sdk_ec2::types::State => "state" ,
    route_table_ids: ::std::vec::Vec<::std::string::String> => "routeTableIdSet" ,
    policy_document: ::std::string::String => "policyDocument" ,
});
wire_object!(aws_sdk_ec2::types::PrefixList {
    prefix_list_id: ::std::string::String => "prefixListId" ,
    prefix_list_name: ::std::string::String => "prefixListName" ,
    cidrs: ::std::vec::Vec<::std::string::String> => "cidrSet" ,
});
wire_object!(aws_sdk_ec2::types::AttributeBooleanValue {
    value: bool => "value" ,
});
wire_object!(aws_sdk_ec2::types::DhcpOptions {
    dhcp_options_id: ::std::string::String => "dhcpOptionsId" ,
    owner_id: ::std::string::String => "ownerId" ,
    dhcp_configurations: ::std::vec::Vec<aws_sdk_ec2::types::DhcpConfiguration> => "dhcpConfigurationSet" ,
});
wire_object!(aws_sdk_ec2::types::DhcpConfiguration {
    key: ::std::string::String => "key" ,
    values: ::std::vec::Vec<aws_sdk_ec2::types::AttributeValue> => "valueSet" ,
});
wire_object!(aws_sdk_ec2::types::AttributeValue {
    value: ::std::string::String => "value" ,
});
wire_object!(aws_sdk_ec2::types::NetworkAcl {
    network_acl_id: ::std::string::String => "networkAclId" ,
    owner_id: ::std::string::String => "ownerId" ,
    vpc_id: ::std::string::String => "vpcId" ,
    associations: ::std::vec::Vec<aws_sdk_ec2::types::NetworkAclAssociation> => "associationSet" ,
    entries: ::std::vec::Vec<aws_sdk_ec2::types::NetworkAclEntry> => "entrySet" ,
});
wire_object!(aws_sdk_ec2::types::NetworkAclAssociation {
    network_acl_association_id: ::std::string::String => "networkAclAssociationId" ,
    network_acl_id: ::std::string::String => "networkAclId" ,
    subnet_id: ::std::string::String => "subnetId" ,
});
wire_object!(aws_sdk_ec2::types::NetworkAclEntry {
    rule_number: i32 => "ruleNumber" ,
    egress: bool => "egress" ,
    rule_action: aws_sdk_ec2::types::RuleAction => "ruleAction" ,
    cidr_block: ::std::string::String => "cidrBlock" ,
    ipv6_cidr_block: ::std::string::String => "ipv6CidrBlock" ,
    protocol: ::std::string::String => "protocol" ,
    port_range: aws_sdk_ec2::types::PortRange => "portRange" ,
    icmp_type_code: aws_sdk_ec2::types::IcmpTypeCode => "icmpTypeCode" ,
});
wire_object!(aws_sdk_ec2::types::PortRange {
    from: i32 => "from" ,
    to: i32 => "to" ,
});
wire_object!(aws_sdk_ec2::types::IcmpTypeCode {
    r#type: i32 => "type" ,
    code: i32 => "code" ,
});
wire_object!(regions::DescribeRegionsOutput {
    regions: ::std::vec::Vec<aws_sdk_ec2::types::Region> => "regionInfo" ,
});
wire_object!(zones::DescribeAvailabilityZonesOutput {
    availability_zones: ::std::vec::Vec<aws_sdk_ec2::types::AvailabilityZone> => "availabilityZoneInfo" ,
});
wire_object!(subnets::DescribeSubnetsOutput {
    subnets: ::std::vec::Vec<aws_sdk_ec2::types::Subnet> => "subnetSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(vpcs::DescribeVpcsOutput {
    vpcs: ::std::vec::Vec<aws_sdk_ec2::types::Vpc> => "vpcSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(groups::DescribeSecurityGroupsOutput {
    security_groups: ::std::vec::Vec<aws_sdk_ec2::types::SecurityGroup> => "securityGroupInfo" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(tables::DescribeRouteTablesOutput {
    route_tables: ::std::vec::Vec<aws_sdk_ec2::types::RouteTable> => "routeTableSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(endpoints::DescribeVpcEndpointsOutput {
    vpc_endpoints: ::std::vec::Vec<aws_sdk_ec2::types::VpcEndpoint> => "vpcEndpointSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(prefixes::DescribePrefixListsOutput {
    prefix_lists: ::std::vec::Vec<aws_sdk_ec2::types::PrefixList> => "prefixListSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(attribute::DescribeVpcAttributeOutput {
    vpc_id: ::std::string::String => "vpcId" ,
    enable_dns_support: aws_sdk_ec2::types::AttributeBooleanValue => "enableDnsSupport" ,
    enable_dns_hostnames: aws_sdk_ec2::types::AttributeBooleanValue => "enableDnsHostnames" ,
});
wire_object!(dhcp::DescribeDhcpOptionsOutput {
    dhcp_options: ::std::vec::Vec<aws_sdk_ec2::types::DhcpOptions> => "dhcpOptionsSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_object!(acls::DescribeNetworkAclsOutput {
    network_acls: ::std::vec::Vec<aws_sdk_ec2::types::NetworkAcl> => "networkAclSet" ,
    next_token: ::std::string::String => "nextToken" ,
});
wire_enums!(
    aws_sdk_ec2::types::AvailabilityZoneState,
    aws_sdk_ec2::types::RouteOrigin,
    aws_sdk_ec2::types::RouteState,
    aws_sdk_ec2::types::RouteTableAssociationStateCode,
    aws_sdk_ec2::types::RuleAction,
    aws_sdk_ec2::types::State,
    aws_sdk_ec2::types::Tenancy,
    aws_sdk_ec2::types::VpcEndpointType
);

struct Frame<'a> {
    prefix: &'a str,
    local: &'a str,
    key: Option<&'static str>,
    shape: Option<&'a Shape>,
    presence: Option<Presence>,
    opened: bool,
    text_seen: bool,
    attributes: Vec<(&'a str, &'a str)>,
}
fn finish_frame(stack: &mut Vec<Frame<'_>>, root: &mut Option<Presence>) -> Result<()> {
    let frame = stack.pop().ok_or(Error("EC2 XML close"))?;
    if let Some(parent) = stack.last_mut() {
        if let Some(value) = frame.presence {
            match parent.presence.as_mut() {
                Some(Presence::Object(fields)) => {
                    require(
                        fields
                            .insert(frame.key.ok_or(Error("EC2 field path"))?, value)
                            .is_none(),
                        "duplicate EC2 field",
                    )?;
                }
                Some(Presence::List(items)) => items.push(value),
                _ => return Err(Error("EC2 structural correlation")),
            }
        }
    } else {
        require(root.is_none(), "duplicate EC2 root")?;
        *root = frame.presence;
    }
    Ok(())
}
fn scan(
    bytes: &[u8],
    root_name: &str,
    shape: &Shape,
    round: &ObservationRound,
) -> Result<(Presence, u64)> {
    require(bytes.len() as u64 <= RESPONSE_BYTES, "EC2 response bound")?;
    round.remaining()?;
    let xml = std::str::from_utf8(bytes).map_err(|_| Error("EC2 XML UTF-8"))?;
    let mut stack: Vec<Frame<'_>> = Vec::new();
    let mut root = None;
    let mut occurrences = 0;
    for (index, token) in xmlparser::Tokenizer::from(xml).enumerate() {
        if index % 64 == 0 {
            round.remaining()?;
        }
        match token.map_err(|_| Error("EC2 XML syntax"))? {
            Token::Declaration { encoding, .. } => require(
                encoding.is_none_or(|v| v.as_str().eq_ignore_ascii_case("utf-8")),
                "EC2 XML encoding",
            )?,
            Token::ElementStart { prefix, local, .. } => {
                require(stack.len() < 128, "EC2 XML depth")?;
                let (key, child) = match stack.last() {
                    None => {
                        require(
                            root.is_none() && local.as_str() == root_name,
                            "EC2 response root",
                        )?;
                        (None, Some(shape))
                    }
                    Some(parent) => {
                        require(parent.opened, "EC2 XML start")?;
                        match parent.shape {
                            Some(Shape::Scalar) => return Err(Error("EC2 scalar child")),
                            Some(Shape::Object(fields))
                                if parent.local == root_name
                                    && local.as_str() == "nextToken"
                                    && !fields.iter().any(|(name, _)| *name == "nextToken") =>
                            {
                                return Err(Error("singleton EC2 continuation"));
                            }
                            Some(Shape::Object(fields)) => fields
                                .iter()
                                .find(|(name, _)| *name == local.as_str())
                                .map_or((None, None), |(name, shape)| (Some(*name), Some(shape))),
                            Some(Shape::List(item)) if local.as_str() == "item" => {
                                occurrences += 1;
                                if occurrences > RECORDS {
                                    round.fail(LimitKind::Records);
                                    return Err(Error("EC2 occurrence bound"));
                                }
                                (None, Some(item.as_ref()))
                            }
                            Some(Shape::List(_)) => return Err(Error("EC2 list member")),
                            None => (None, None),
                        }
                    }
                };
                stack.push(Frame {
                    prefix: prefix.as_str(),
                    local: local.as_str(),
                    key,
                    shape: child,
                    presence: child.map(Shape::empty),
                    opened: false,
                    text_seen: false,
                    attributes: Vec::new(),
                });
            }
            Token::Attribute {
                prefix,
                local,
                value,
                ..
            } => {
                let frame = stack.last_mut().ok_or(Error("EC2 attribute location"))?;
                require(
                    !frame.opened && frame.attributes.len() < 128,
                    "EC2 attributes",
                )?;
                let name = (prefix.as_str(), local.as_str());
                require(
                    !frame.attributes.contains(&name) && !value.as_str().contains('&'),
                    "EC2 attribute syntax",
                )?;
                frame.attributes.push(name);
                if frame.shape.is_some() {
                    require(
                        prefix.as_str() == "xmlns"
                            || (prefix.as_str().is_empty() && local.as_str() == "xmlns"),
                        "EC2 monitored attribute",
                    )?;
                }
            }
            Token::ElementEnd { end, .. } => match end {
                ElementEnd::Open => {
                    let frame = stack.last_mut().ok_or(Error("EC2 open"))?;
                    require(!frame.opened, "EC2 repeated open")?;
                    frame.opened = true;
                }
                ElementEnd::Empty => {
                    require(stack.last().is_some_and(|f| !f.opened), "EC2 empty close")?;
                    finish_frame(&mut stack, &mut root)?;
                }
                ElementEnd::Close(prefix, local) => {
                    require(
                        stack.last().is_some_and(|f| {
                            f.opened && f.prefix == prefix.as_str() && f.local == local.as_str()
                        }),
                        "EC2 mismatched close",
                    )?;
                    finish_frame(&mut stack, &mut root)?;
                }
            },
            Token::Text { text } => match stack.last_mut() {
                Some(frame) => {
                    require(frame.opened, "EC2 scalar position")?;
                    match frame.presence.as_mut() {
                        Some(Presence::Scalar(nonempty)) => {
                            require(!frame.text_seen, "EC2 fragmented scalar")?;
                            frame.text_seen = true;
                            *nonempty = !text.as_str().is_empty();
                        }
                        Some(_) => require(text.as_str().trim().is_empty(), "EC2 container text")?,
                        None => (),
                    }
                }
                None => require(text.as_str().trim().is_empty(), "EC2 outside text")?,
            },
            Token::Comment { .. } => require(
                !stack
                    .last()
                    .is_some_and(|f| matches!(f.shape, Some(Shape::Scalar))),
                "EC2 scalar comment",
            )?,
            Token::Cdata { .. } if stack.last().is_some_and(|f| f.shape.is_none()) => (),
            _ => return Err(Error("unsupported EC2 XML structure")),
        }
    }
    round.remaining()?;
    require(stack.is_empty(), "EC2 incomplete XML")?;
    Ok((root.ok_or(Error("EC2 missing root"))?, occurrences))
}

pub(super) enum Ec2Output {
    Regions(Box<regions::DescribeRegionsOutput>),
    AvailabilityZones(Box<zones::DescribeAvailabilityZonesOutput>),
    Subnets(Box<subnets::DescribeSubnetsOutput>),
    Vpcs(Box<vpcs::DescribeVpcsOutput>),
    SecurityGroups(Box<groups::DescribeSecurityGroupsOutput>),
    RouteTables(Box<tables::DescribeRouteTablesOutput>),
    VpcEndpoints(Box<endpoints::DescribeVpcEndpointsOutput>),
    PrefixLists(Box<prefixes::DescribePrefixListsOutput>),
    VpcAttribute(Box<attribute::DescribeVpcAttributeOutput>),
    DhcpOptions(Box<dhcp::DescribeDhcpOptionsOutput>),
    NetworkAcls(Box<acls::DescribeNetworkAclsOutput>),
}
impl Ec2Output {
    fn presence(&self) -> Presence {
        match self {
            Self::Regions(value) => value.presence(),
            Self::AvailabilityZones(value) => value.presence(),
            Self::Subnets(value) => value.presence(),
            Self::Vpcs(value) => value.presence(),
            Self::SecurityGroups(value) => value.presence(),
            Self::RouteTables(value) => value.presence(),
            Self::VpcEndpoints(value) => value.presence(),
            Self::PrefixLists(value) => value.presence(),
            Self::VpcAttribute(value) => value.presence(),
            Self::DhcpOptions(value) => value.presence(),
            Self::NetworkAcls(value) => value.presence(),
        }
    }
}
enum State {
    Fresh,
    Started,
    ServiceError,
    Captured(Presence, u64),
    Normalized(Ec2Output, u64),
    Complete(Ec2Output, u64),
    Failed,
}
#[derive(Clone)]
pub(super) struct Ec2Integrity {
    operation: ReadOperationV1,
    state: Arc<Mutex<State>>,
    round: ObservationRound,
}
impl std::fmt::Debug for Ec2Integrity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never inspect capture state or delegate to SDK Debug: even a completed
        // guard still owns raw continuation and policy evidence until consumed.
        f.debug_struct("Ec2Integrity")
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}
pub(super) struct Ec2Receiver {
    state: Arc<Mutex<State>>,
    round: ObservationRound,
}
pub(super) fn capture_ec2(
    operation: ReadOperationV1,
    round: ObservationRound,
) -> (Ec2Integrity, Ec2Receiver) {
    let state = Arc::new(Mutex::new(State::Fresh));
    (
        Ec2Integrity {
            operation,
            state: state.clone(),
            round: round.clone(),
        },
        Ec2Receiver { state, round },
    )
}
impl Ec2Receiver {
    pub(super) fn take(self) -> Result<(Ec2Output, u64)> {
        self.round.remaining()?;
        match std::mem::replace(
            &mut *self.state.lock().map_err(|_| Error("EC2 capture state"))?,
            State::Failed,
        ) {
            State::Complete(output, count) => Ok((output, count)),
            _ => Err(Error("EC2 capture unavailable")),
        }
    }
}
impl Ec2Integrity {
    fn transition(&self, f: impl FnOnce(State) -> Result<State>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error("EC2 capture state"))?;
        let previous = std::mem::replace(&mut *state, State::Failed);
        self.round.remaining()?;
        *state = f(previous)?;
        Ok(())
    }
}
type HookResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;
impl Intercept for Ec2Integrity {
    fn name(&self) -> &'static str {
        "Ec2Integrity"
    }
    fn read_before_execution(
        &self,
        ctx: &BeforeSerializationInterceptorContextRef<'_>,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            require(matches!(state, State::Fresh), "EC2 capture reuse")?;
            let compatible = match self.operation {
                ReadOperationV1::DescribeRegions => ctx
                    .input()
                    .downcast_ref::<regions::DescribeRegionsInput>()
                    .is_some(),
                ReadOperationV1::DescribeAvailabilityZones => ctx
                    .input()
                    .downcast_ref::<zones::DescribeAvailabilityZonesInput>()
                    .is_some(),
                ReadOperationV1::DescribeSubnets => ctx
                    .input()
                    .downcast_ref::<subnets::DescribeSubnetsInput>()
                    .is_some(),
                ReadOperationV1::DescribeVpcs => ctx
                    .input()
                    .downcast_ref::<vpcs::DescribeVpcsInput>()
                    .is_some(),
                ReadOperationV1::DescribeSecurityGroups => ctx
                    .input()
                    .downcast_ref::<groups::DescribeSecurityGroupsInput>()
                    .is_some(),
                ReadOperationV1::DescribeRouteTables => ctx
                    .input()
                    .downcast_ref::<tables::DescribeRouteTablesInput>()
                    .is_some(),
                ReadOperationV1::DescribeVpcEndpoints => ctx
                    .input()
                    .downcast_ref::<endpoints::DescribeVpcEndpointsInput>()
                    .is_some(),
                ReadOperationV1::DescribePrefixLists => ctx
                    .input()
                    .downcast_ref::<prefixes::DescribePrefixListsInput>()
                    .is_some(),
                ReadOperationV1::DescribeVpcAttribute => ctx
                    .input()
                    .downcast_ref::<attribute::DescribeVpcAttributeInput>()
                    .is_some(),
                ReadOperationV1::DescribeDhcpOptions => ctx
                    .input()
                    .downcast_ref::<dhcp::DescribeDhcpOptionsInput>()
                    .is_some(),
                ReadOperationV1::DescribeNetworkAcls => ctx
                    .input()
                    .downcast_ref::<acls::DescribeNetworkAclsInput>()
                    .is_some(),
                _ => false,
            };
            require(compatible, "EC2 capture operation")?;
            Ok(State::Started)
        })?;
        Ok(())
    }
    fn read_before_deserialization(
        &self,
        ctx: &BeforeDeserializationInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            require(matches!(state, State::Started), "EC2 repeated response")?;
            // Ordinary service errors never enter success-schema scanning.
            if !ctx.response().status().is_success() {
                return Ok(State::ServiceError);
            }
            let (root, shape) = match self.operation {
                ReadOperationV1::DescribeRegions => (
                    "DescribeRegionsResponse",
                    regions::DescribeRegionsOutput::shape(),
                ),
                ReadOperationV1::DescribeAvailabilityZones => (
                    "DescribeAvailabilityZonesResponse",
                    zones::DescribeAvailabilityZonesOutput::shape(),
                ),
                ReadOperationV1::DescribeSubnets => (
                    "DescribeSubnetsResponse",
                    subnets::DescribeSubnetsOutput::shape(),
                ),
                ReadOperationV1::DescribeVpcs => {
                    ("DescribeVpcsResponse", vpcs::DescribeVpcsOutput::shape())
                }
                ReadOperationV1::DescribeSecurityGroups => (
                    "DescribeSecurityGroupsResponse",
                    groups::DescribeSecurityGroupsOutput::shape(),
                ),
                ReadOperationV1::DescribeRouteTables => (
                    "DescribeRouteTablesResponse",
                    tables::DescribeRouteTablesOutput::shape(),
                ),
                ReadOperationV1::DescribeVpcEndpoints => (
                    "DescribeVpcEndpointsResponse",
                    endpoints::DescribeVpcEndpointsOutput::shape(),
                ),
                ReadOperationV1::DescribePrefixLists => (
                    "DescribePrefixListsResponse",
                    prefixes::DescribePrefixListsOutput::shape(),
                ),
                ReadOperationV1::DescribeVpcAttribute => (
                    "DescribeVpcAttributeResponse",
                    attribute::DescribeVpcAttributeOutput::shape(),
                ),
                ReadOperationV1::DescribeDhcpOptions => (
                    "DescribeDhcpOptionsResponse",
                    dhcp::DescribeDhcpOptionsOutput::shape(),
                ),
                ReadOperationV1::DescribeNetworkAcls => (
                    "DescribeNetworkAclsResponse",
                    acls::DescribeNetworkAclsOutput::shape(),
                ),
                _ => return Err(Error("EC2 capture operation")),
            };
            let body = ctx
                .response()
                .body()
                .bytes()
                .ok_or(Error("EC2 completed body required"))?;
            let (presence, count) = scan(body, root, &shape, &self.round)?;
            Ok(State::Captured(presence, count))
        })?;
        Ok(())
    }
    fn read_after_deserialization(
        &self,
        ctx: &AfterDeserializationInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            let Ok(output) = ctx.output_or_error() else {
                return Ok(State::Failed);
            };
            let State::Captured(presence, count) = state else {
                return Err(Error("EC2 capture correlation"));
            };
            let value = match self.operation {
                ReadOperationV1::DescribeRegions => Ec2Output::Regions(Box::new(
                    output
                        .downcast_ref::<regions::DescribeRegionsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeAvailabilityZones => {
                    Ec2Output::AvailabilityZones(Box::new(
                        output
                            .downcast_ref::<zones::DescribeAvailabilityZonesOutput>()
                            .ok_or(Error("EC2 output type"))?
                            .clone(),
                    ))
                }
                ReadOperationV1::DescribeSubnets => Ec2Output::Subnets(Box::new(
                    output
                        .downcast_ref::<subnets::DescribeSubnetsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeVpcs => Ec2Output::Vpcs(Box::new(
                    output
                        .downcast_ref::<vpcs::DescribeVpcsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeSecurityGroups => Ec2Output::SecurityGroups(Box::new(
                    output
                        .downcast_ref::<groups::DescribeSecurityGroupsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeRouteTables => Ec2Output::RouteTables(Box::new(
                    output
                        .downcast_ref::<tables::DescribeRouteTablesOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeVpcEndpoints => Ec2Output::VpcEndpoints(Box::new(
                    output
                        .downcast_ref::<endpoints::DescribeVpcEndpointsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribePrefixLists => Ec2Output::PrefixLists(Box::new(
                    output
                        .downcast_ref::<prefixes::DescribePrefixListsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeVpcAttribute => Ec2Output::VpcAttribute(Box::new(
                    output
                        .downcast_ref::<attribute::DescribeVpcAttributeOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeDhcpOptions => Ec2Output::DhcpOptions(Box::new(
                    output
                        .downcast_ref::<dhcp::DescribeDhcpOptionsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                ReadOperationV1::DescribeNetworkAcls => Ec2Output::NetworkAcls(Box::new(
                    output
                        .downcast_ref::<acls::DescribeNetworkAclsOutput>()
                        .ok_or(Error("EC2 output type"))?
                        .clone(),
                )),
                _ => return Err(Error("EC2 capture operation")),
            };
            require(presence == value.presence(), "EC2 output correlation")?;
            Ok(State::Normalized(value, count))
        })?;
        Ok(())
    }
    fn read_after_execution(
        &self,
        ctx: &FinalizerInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            if !matches!(ctx.output_or_error(), Some(Ok(_))) {
                return Ok(State::Failed);
            }
            match state {
                State::Normalized(value, count) => Ok(State::Complete(value, count)),
                _ => Err(Error("EC2 incomplete lifecycle")),
            }
        })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "ec2_decode_integrity_tests.rs"]
mod tests;

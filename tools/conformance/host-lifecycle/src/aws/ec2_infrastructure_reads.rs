//! Eleven closed read-only adapters, using the identity observer's shared execution contract.
use super::{
    ec2_decode_integrity::capture_ec2,
    ec2_observation::{combine_failure, normalize},
    query_execution::{LogicalQuery, QueryResult, round_failure},
    response_limits::ObservationRound,
};
use crate::{
    Result,
    deployment::DeploymentV2,
    identity::*,
    provider::{
        coverage::*,
        ec2_observation_v4::ObservationDataV4,
        inventory::ResourceIdentity,
        management_observation_v2::ObservationValueV2,
        manifest::ReviewedInfrastructureV1,
        observation_v4::{ObservationEntryV4, ObservationRecordV4},
    },
};
use aws_smithy_runtime_api::client::{orchestrator::HttpResponse, result::SdkError};
use aws_smithy_types::error::metadata::ProvideErrorMetadata;

#[derive(Clone, Copy, Debug)]
pub(super) enum InfrastructureRead {
    Regions,
    AvailabilityZones,
    Subnets,
    Vpcs,
    SecurityGroups,
    RouteTables,
    VpcEndpoints,
    PrefixLists,
    VpcAttribute(VpcAttribute),
    DhcpOptions,
    NetworkAcls,
}
impl InfrastructureRead {
    pub(super) fn operation(self) -> ReadOperationV1 {
        match self {
            Self::Regions => ReadOperationV1::DescribeRegions,
            Self::AvailabilityZones => ReadOperationV1::DescribeAvailabilityZones,
            Self::Subnets => ReadOperationV1::DescribeSubnets,
            Self::Vpcs => ReadOperationV1::DescribeVpcs,
            Self::SecurityGroups => ReadOperationV1::DescribeSecurityGroups,
            Self::RouteTables => ReadOperationV1::DescribeRouteTables,
            Self::VpcEndpoints => ReadOperationV1::DescribeVpcEndpoints,
            Self::PrefixLists => ReadOperationV1::DescribePrefixLists,
            Self::VpcAttribute(_) => ReadOperationV1::DescribeVpcAttribute,
            Self::DhcpOptions => ReadOperationV1::DescribeDhcpOptions,
            Self::NetworkAcls => ReadOperationV1::DescribeNetworkAcls,
        }
    }
}
/// Request targets are constructed only after canonical manifest/deployment binding.
pub(super) struct InfrastructureTargets {
    account: AwsAccountId,
    region: Region,
    vpc: VpcId,
    subnet: SubnetId,
    groups: Vec<SecurityGroupId>,
    route_table: RouteTableId,
    endpoint: VpcEndpointId,
    prefix_list: PrefixListId,
    dhcp: DhcpOptionsId,
    nacl: NetworkAclId,
}
impl InfrastructureTargets {
    pub(super) fn parse(deployment: &DeploymentV2, bytes: &[u8]) -> Result<Self> {
        let manifest = ReviewedInfrastructureV1::parse_bound(bytes, deployment)?;
        Ok(Self {
            account: manifest.account,
            region: manifest.region,
            vpc: manifest.vpc,
            subnet: manifest.subnet,
            groups: manifest.security_groups.into_iter().map(|g| g.id).collect(),
            route_table: manifest.route_table,
            endpoint: manifest.s3_endpoint,
            prefix_list: manifest.s3_prefix_list,
            dhcp: manifest.dns.dhcp_options,
            nacl: manifest.nacl.id,
        })
    }
}
pub(super) struct InfrastructureObservations {
    targets: InfrastructureTargets,
    round: ObservationRound,
    ec2: aws_sdk_ec2::Client,
}
impl InfrastructureObservations {
    pub(super) fn from_config(
        targets: InfrastructureTargets,
        conf: &aws_types::SdkConfig,
        round: ObservationRound,
    ) -> Self {
        Self {
            targets,
            round,
            ec2: aws_sdk_ec2::Client::from_conf(
                aws_sdk_ec2::config::Builder::from(conf)
                    .use_fips(false)
                    .use_dual_stack(false)
                    .build(),
            ),
        }
    }
    fn query(&self, read: InfrastructureRead) -> QueryIdentityV1 {
        use InfrastructureRead as I;
        use ResourceIdentity as R;
        let ids = match read {
            I::Regions | I::AvailabilityZones | I::VpcAttribute(_) => None,
            I::Subnets => Some(vec![R::Subnet(self.targets.subnet.clone())]),
            I::Vpcs => Some(vec![R::Vpc(self.targets.vpc.clone())]),
            I::SecurityGroups => Some(
                self.targets
                    .groups
                    .iter()
                    .cloned()
                    .map(R::SecurityGroup)
                    .collect(),
            ),
            I::RouteTables => Some(vec![R::RouteTable(self.targets.route_table.clone())]),
            I::VpcEndpoints => Some(vec![R::Endpoint(self.targets.endpoint.clone())]),
            I::PrefixLists => Some(vec![R::PrefixList(self.targets.prefix_list.clone())]),
            I::DhcpOptions => Some(vec![R::Dhcp(self.targets.dhcp.clone())]),
            I::NetworkAcls => Some(vec![R::Nacl(self.targets.nacl.clone())]),
        };
        let scope = match (read, ids) {
            (I::VpcAttribute(attribute), _) => QueryScopeV1::VpcAttribute {
                vpc: self.targets.vpc.clone(),
                attribute,
            },
            (_, Some(identities)) => QueryScopeV1::Exact { identities },
            _ => QueryScopeV1::Regional,
        };
        QueryIdentityV1 {
            operation: read.operation(),
            account: self.targets.account.clone(),
            region: self.targets.region.clone(),
            scope,
        }
    }
    pub(super) async fn observe(
        &mut self,
        read: InfrastructureRead,
        required: bool,
    ) -> std::result::Result<QueryResult, ReadFailureV1> {
        let mut query = LogicalQuery::begin(self.round.clone(), self.query(read), required)?;
        while let Ok(returned) = query.start_page() {
            let token = returned.map(str::to_owned);
            let (guard, receiver) = capture_ec2(read.operation(), self.round.clone());
            let result = match read {
                InfrastructureRead::Regions => self
                    .ec2
                    .describe_regions()
                    .all_regions(true)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::AvailabilityZones => self
                    .ec2
                    .describe_availability_zones()
                    .all_availability_zones(true)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::Subnets => self
                    .ec2
                    .describe_subnets()
                    .subnet_ids(self.targets.subnet.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::Vpcs => self
                    .ec2
                    .describe_vpcs()
                    .vpc_ids(self.targets.vpc.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::SecurityGroups => self
                    .ec2
                    .describe_security_groups()
                    .set_group_ids(Some(
                        self.targets
                            .groups
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                    ))
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::RouteTables => self
                    .ec2
                    .describe_route_tables()
                    .route_table_ids(self.targets.route_table.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::VpcEndpoints => self
                    .ec2
                    .describe_vpc_endpoints()
                    .vpc_endpoint_ids(self.targets.endpoint.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::PrefixLists => self
                    .ec2
                    .describe_prefix_lists()
                    .prefix_list_ids(self.targets.prefix_list.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::DhcpOptions => self
                    .ec2
                    .describe_dhcp_options()
                    .dhcp_options_ids(self.targets.dhcp.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::NetworkAcls => self
                    .ec2
                    .describe_network_acls()
                    .network_acl_ids(self.targets.nacl.as_str())
                    .set_next_token(token)
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
                InfrastructureRead::VpcAttribute(attribute) => self
                    .ec2
                    .describe_vpc_attribute()
                    .vpc_id(self.targets.vpc.as_str())
                    .attribute(match attribute {
                        VpcAttribute::EnableDnsSupport => {
                            aws_sdk_ec2::types::VpcAttributeName::EnableDnsSupport
                        }
                        VpcAttribute::EnableDnsHostnames => {
                            aws_sdk_ec2::types::VpcAttributeName::EnableDnsHostnames
                        }
                    })
                    .customize()
                    .interceptor(guard)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| read_failure(&e)),
            };
            if let Err(reason) = result {
                query.failed_page(round_failure(&self.round, reason));
                break;
            }
            // No returned token or record is accepted before consuming the correlated invocation.
            let (output, occurrences) = match receiver.take() {
                Ok(v) => v,
                Err(_) => {
                    query.failed_page(round_failure(&self.round, ReadFailureV1::Malformed));
                    break;
                }
            };
            let mut page = normalize(output, occurrences, &self.round);
            if let InfrastructureRead::VpcAttribute(attribute) = read {
                let available=page.data.first().is_some_and(|data| {
                    let ObservationDataV4::Dns {support,hostnames,..}=data else {return false;};
                    let selected=match attribute {VpcAttribute::EnableDnsSupport=>support,VpcAttribute::EnableDnsHostnames=>hostnames};
                    matches!(selected,ObservationValueV2::Present(v) if matches!(v.value,ObservationValueV2::Present(_)))
                });
                if !available {
                    page.incomplete =
                        combine_failure(page.incomplete, Some(ReadFailureV1::Malformed));
                }
            }
            let continuation = if matches!(read, InfrastructureRead::VpcEndpoints) {
                vpc_endpoint_continuation(page.returned_token)
            } else {
                page.returned_token
            };
            let terminal = continuation.is_none();
            let records = page
                .data
                .into_iter()
                .map(|data| {
                    ObservationEntryV4::V4(Box::new(ObservationRecordV4 {
                        schema_version: 4,
                        query: query.query().clone(),
                        data,
                    }))
                })
                .collect();
            if query
                .page(records, page.occurrences, continuation, page.incomplete)
                .is_err()
                || terminal
            {
                break;
            }
        }
        Ok(query.finish())
    }
}
// DescribeVpcEndpoints documents empty string as exhaustion. This is not a shared token rule.
fn vpc_endpoint_continuation(returned: Option<String>) -> Option<String> {
    match returned {
        Some(token) if token.is_empty() => None,
        token => token,
    }
}
fn read_failure<E: ProvideErrorMetadata>(error: &SdkError<E, HttpResponse>) -> ReadFailureV1 {
    if let SdkError::ServiceError(e) = error
        && !e.raw().status().is_success()
        && matches!(
            e.err().code(),
            Some(
                "InvalidSubnetID.NotFound"
                    | "InvalidVpcID.NotFound"
                    | "InvalidGroup.NotFound"
                    | "InvalidRouteTableID.NotFound"
                    | "InvalidVpcEndpointId.NotFound"
                    | "InvalidPrefixListId.NotFound"
                    | "InvalidDhcpOptionID.NotFound"
                    | "InvalidNetworkAclID.NotFound"
            )
        )
    {
        return ReadFailureV1::NotFound;
    }
    super::identity_reads::read_failure(error)
}

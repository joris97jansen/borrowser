//! Closed allocation selectors and operation-specific SDK requests; no discovery coordination.
use super::{
    ec2_allocation_observation::project,
    ec2_decode_integrity::allocation::capture_allocation,
    query_execution::{LogicalQuery, QueryResult, round_failure},
    response_limits::ObservationRound,
};
use crate::{
    identity::*,
    launch::ClientToken,
    provider::{
        coverage::*, inventory::ResourceIdentity as R, observation_v5::validate_allocation_query,
    },
};
use aws_smithy_runtime_api::client::{orchestrator::HttpResponse, result::SdkError};
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use serde::Serialize;
type ReadResult<T> = std::result::Result<T, ReadFailureV1>;

pub(super) struct ExactIds<T>(Vec<T>);
impl<T: Serialize> TryFrom<Vec<T>> for ExactIds<T> {
    type Error = ReadFailureV1;
    fn try_from(v: Vec<T>) -> ReadResult<Self> {
        if v.is_empty() || crate::provider::canonical_set(&v, 128).is_err() {
            return Err(ReadFailureV1::Malformed);
        }
        Ok(Self(v))
    }
}
pub(super) struct ExactInstanceTypes(ExactIds<InstanceType>);
impl TryFrom<ExactIds<InstanceType>> for ExactInstanceTypes {
    type Error = ReadFailureV1;
    fn try_from(v: ExactIds<InstanceType>) -> ReadResult<Self> {
        if v.0.len() > 100 {
            return Err(ReadFailureV1::Malformed);
        }
        Ok(Self(v))
    }
}
pub(super) enum TagScope {
    Combined {
        authority: AuthorityId,
        operation: OperationId,
    },
    Authority(AuthorityId),
    Operation(OperationId),
}
impl TagScope {
    fn scope(&self) -> QueryScopeV1 {
        match self {
            Self::Combined {
                authority,
                operation,
            } => QueryScopeV1::OperationTags {
                authority: authority.clone(),
                operation: operation.clone(),
            },
            Self::Authority(authority) => QueryScopeV1::AuthorityTag {
                authority: authority.clone(),
            },
            Self::Operation(operation) => QueryScopeV1::OperationTag {
                operation: operation.clone(),
            },
        }
    }
}
pub(super) enum InstanceScope {
    Exact(ExactIds<InstanceId>),
    ClientToken(ClientToken),
    Tags(TagScope),
}
pub(super) enum ResourceScope<T> {
    Exact(ExactIds<T>),
    Tags(TagScope),
    AttachedTo(InstanceId),
}
pub(super) enum ProfileAssociationScope {
    Exact(ExactIds<ProfileAssociationId>),
    AttachedTo(InstanceId),
}
pub(super) enum AllocationRead {
    Images(ExactIds<AmiId>),
    InstanceTypes(ExactInstanceTypes),
    TypeOfferings {
        instance_type: InstanceType,
        zone: AvailabilityZone,
    },
    ProfileAssociations(ProfileAssociationScope),
    Instances(InstanceScope),
    NetworkInterfaces(ResourceScope<NetworkInterfaceId>),
    Volumes(ResourceScope<VolumeId>),
    InstanceAttribute {
        instance: InstanceId,
        attribute: InstanceAttribute,
    },
}
fn exact<T: Clone>(v: &ExactIds<T>, wrap: impl Fn(T) -> R) -> QueryScopeV1 {
    QueryScopeV1::Exact {
        identities: v.0.iter().cloned().map(wrap).collect(),
    }
}
fn resource_scope<T: Clone>(v: &ResourceScope<T>, wrap: impl Fn(T) -> R) -> QueryScopeV1 {
    match v {
        ResourceScope::Exact(v) => exact(v, wrap),
        ResourceScope::Tags(v) => v.scope(),
        ResourceScope::AttachedTo(instance) => QueryScopeV1::AttachedTo {
            instance: instance.clone(),
        },
    }
}
impl AllocationRead {
    fn selection(&self) -> (ReadOperationV1, QueryScopeV1) {
        use ReadOperationV1 as O;
        match self {
            Self::Images(v) => (O::DescribeImages, exact(v, R::Image)),
            Self::InstanceTypes(v) => (O::DescribeInstanceTypes, exact(&v.0, R::InstanceType)),
            Self::TypeOfferings {
                instance_type,
                zone,
            } => (
                O::DescribeInstanceTypeOfferings,
                QueryScopeV1::TypeOffering {
                    instance_type: instance_type.clone(),
                    zone: zone.clone(),
                },
            ),
            Self::ProfileAssociations(v) => (
                O::DescribeIamInstanceProfileAssociations,
                match v {
                    ProfileAssociationScope::Exact(v) => exact(v, R::ProfileAssociation),
                    ProfileAssociationScope::AttachedTo(instance) => QueryScopeV1::AttachedTo {
                        instance: instance.clone(),
                    },
                },
            ),
            Self::Instances(v) => (
                O::DescribeInstances,
                match v {
                    InstanceScope::Exact(v) => exact(v, R::Instance),
                    InstanceScope::ClientToken(token) => QueryScopeV1::ClientToken {
                        token: token.clone(),
                    },
                    InstanceScope::Tags(v) => v.scope(),
                },
            ),
            Self::NetworkInterfaces(v) => (
                O::DescribeNetworkInterfaces,
                resource_scope(v, R::NetworkInterface),
            ),
            Self::Volumes(v) => (O::DescribeVolumes, resource_scope(v, R::Volume)),
            Self::InstanceAttribute {
                instance,
                attribute,
            } => (
                O::DescribeInstanceAttribute,
                QueryScopeV1::InstanceAttribute {
                    instance: instance.clone(),
                    attribute: *attribute,
                },
            ),
        }
    }
}
/// Validate the closed selection before a query can reserve evidence or send a request.
impl TryFrom<(ReadOperationV1, QueryScopeV1)> for AllocationRead {
    type Error = ReadFailureV1;
    fn try_from((operation, scope): (ReadOperationV1, QueryScopeV1)) -> ReadResult<Self> {
        use QueryScopeV1 as S;
        use ReadOperationV1 as O;
        fn ids<T: Serialize>(
            v: Vec<R>,
            extract: impl Fn(R) -> Option<T>,
        ) -> ReadResult<ExactIds<T>> {
            v.into_iter()
                .map(|r| extract(r).ok_or(ReadFailureV1::Malformed))
                .collect::<ReadResult<Vec<T>>>()?
                .try_into()
        }
        macro_rules! get {
            ($v:expr,$variant:ident) => {
                ids($v, |r| {
                    if let R::$variant(v) = r {
                        Some(v)
                    } else {
                        None
                    }
                })?
            };
        }
        let tags = match &scope {
            S::OperationTags {
                authority,
                operation,
            } => Some(TagScope::Combined {
                authority: authority.clone(),
                operation: operation.clone(),
            }),
            S::AuthorityTag { authority } => Some(TagScope::Authority(authority.clone())),
            S::OperationTag { operation } => Some(TagScope::Operation(operation.clone())),
            _ => None,
        };
        Ok(match (operation, scope, tags) {
            (O::DescribeImages, S::Exact { identities }, _) => {
                Self::Images(get!(identities, Image))
            }
            (O::DescribeInstanceTypes, S::Exact { identities }, _) => Self::InstanceTypes(
                ExactInstanceTypes::try_from(get!(identities, InstanceType))?,
            ),
            (
                O::DescribeInstanceTypeOfferings,
                S::TypeOffering {
                    instance_type,
                    zone,
                },
                _,
            ) => Self::TypeOfferings {
                instance_type,
                zone,
            },
            (O::DescribeIamInstanceProfileAssociations, S::Exact { identities }, _) => {
                Self::ProfileAssociations(ProfileAssociationScope::Exact(get!(
                    identities,
                    ProfileAssociation
                )))
            }
            (O::DescribeIamInstanceProfileAssociations, S::AttachedTo { instance }, _) => {
                Self::ProfileAssociations(ProfileAssociationScope::AttachedTo(instance))
            }
            (O::DescribeInstances, S::Exact { identities }, _) => {
                Self::Instances(InstanceScope::Exact(get!(identities, Instance)))
            }
            (O::DescribeInstances, S::ClientToken { token }, _) => {
                Self::Instances(InstanceScope::ClientToken(token))
            }
            (O::DescribeNetworkInterfaces, S::Exact { identities }, _) => {
                Self::NetworkInterfaces(ResourceScope::Exact(get!(identities, NetworkInterface)))
            }
            (O::DescribeVolumes, S::Exact { identities }, _) => {
                Self::Volumes(ResourceScope::Exact(get!(identities, Volume)))
            }
            (O::DescribeNetworkInterfaces, S::AttachedTo { instance }, _) => {
                Self::NetworkInterfaces(ResourceScope::AttachedTo(instance))
            }
            (O::DescribeVolumes, S::AttachedTo { instance }, _) => {
                Self::Volumes(ResourceScope::AttachedTo(instance))
            }
            (
                O::DescribeInstanceAttribute,
                S::InstanceAttribute {
                    instance,
                    attribute,
                },
                _,
            ) => Self::InstanceAttribute {
                instance,
                attribute,
            },
            (O::DescribeInstances, _, Some(tags)) => Self::Instances(InstanceScope::Tags(tags)),
            (O::DescribeNetworkInterfaces, _, Some(tags)) => {
                Self::NetworkInterfaces(ResourceScope::Tags(tags))
            }
            (O::DescribeVolumes, _, Some(tags)) => Self::Volumes(ResourceScope::Tags(tags)),
            _ => return Err(ReadFailureV1::Unsupported),
        })
    }
}
pub(super) struct AllocationObservations {
    account: AwsAccountId,
    region: Region,
    round: ObservationRound,
    ec2: aws_sdk_ec2::Client,
}
impl AllocationObservations {
    pub(super) fn from_config(
        deployment: &crate::deployment::DeploymentV2,
        conf: &aws_types::SdkConfig,
        round: ObservationRound,
    ) -> Self {
        Self {
            account: deployment.identity.account_id.clone(),
            region: deployment.identity.region.clone(),
            round,
            ec2: aws_sdk_ec2::Client::from_conf(
                aws_sdk_ec2::config::Builder::from(conf)
                    .use_fips(false)
                    .use_dual_stack(false)
                    .build(),
            ),
        }
    }
    pub(super) async fn observe(
        &mut self,
        read: AllocationRead,
        required: bool,
    ) -> ReadResult<QueryResult> {
        let (operation, scope) = read.selection();
        let identity = QueryIdentityV1 {
            operation,
            scope,
            account: self.account.clone(),
            region: self.region.clone(),
        };
        validate_allocation_query(&identity).map_err(|_| ReadFailureV1::Malformed)?;
        let ids = match &identity.scope {
            QueryScopeV1::Exact { identities } => Some(
                identities
                    .iter()
                    .map(|r| match r {
                        R::Image(v) => Ok(v.as_str().to_owned()),
                        R::InstanceType(v) => Ok(v.as_str().to_owned()),
                        R::ProfileAssociation(v) => Ok(v.as_str().to_owned()),
                        R::Instance(v) => Ok(v.as_str().to_owned()),
                        R::NetworkInterface(v) => Ok(v.as_str().to_owned()),
                        R::Volume(v) => Ok(v.as_str().to_owned()),
                        _ => Err(ReadFailureV1::Malformed),
                    })
                    .collect::<ReadResult<Vec<_>>>()?,
            ),
            _ => None,
        };
        let filters = filters(&identity.scope, operation);
        let max = ids.is_none().then_some(10);
        let mut query = LogicalQuery::begin(self.round.clone(), identity, required)?;
        while let Ok((attempt, token)) = query.start_allocation_page() {
            let (guard, receiver) = capture_allocation(attempt, self.round.clone());
            macro_rules! send {
                ($builder:expr) => {
                    $builder
                        .customize()
                        .interceptor(guard)
                        .send()
                        .await
                        .map(|_| ())
                        .map_err(|e| read_failure(&e))
                };
            }
            let result = match &read {
                AllocationRead::Images(_) => send!(
                    self.ec2
                        .describe_images()
                        .set_image_ids(ids.clone())
                        .include_deprecated(true)
                        .include_disabled(true)
                        .set_next_token(token)
                ),
                AllocationRead::InstanceTypes(_) => send!(
                    self.ec2
                        .describe_instance_types()
                        .set_instance_types(ids.as_ref().map(|v| {
                            v.iter()
                                .map(|v| aws_sdk_ec2::types::InstanceType::from(v.as_str()))
                                .collect()
                        }))
                        .include_unsupported_in_region(true)
                        .set_next_token(token)
                ),
                AllocationRead::TypeOfferings { .. } => send!(
                    self.ec2
                        .describe_instance_type_offerings()
                        .location_type(aws_sdk_ec2::types::LocationType::AvailabilityZone)
                        .set_filters(filters.clone())
                        .max_results(10)
                        .set_next_token(token)
                ),
                AllocationRead::ProfileAssociations(_) => send!(
                    self.ec2
                        .describe_iam_instance_profile_associations()
                        .set_association_ids(ids.clone())
                        .set_filters(filters.clone())
                        .set_max_results(max)
                        .set_next_token(token)
                ),
                AllocationRead::Instances(_) => send!(
                    self.ec2
                        .describe_instances()
                        .set_instance_ids(ids.clone())
                        .set_filters(filters.clone())
                        .include_managed_resources(true)
                        .set_max_results(max)
                        .set_next_token(token)
                ),
                AllocationRead::NetworkInterfaces(_) => send!(
                    self.ec2
                        .describe_network_interfaces()
                        .set_network_interface_ids(ids.clone())
                        .set_filters(filters.clone())
                        .include_managed_resources(true)
                        .set_max_results(max)
                        .set_next_token(token)
                ),
                AllocationRead::Volumes(_) => send!(
                    self.ec2
                        .describe_volumes()
                        .set_volume_ids(ids.clone())
                        .set_filters(filters.clone())
                        .include_managed_resources(true)
                        .set_max_results(max)
                        .set_next_token(token)
                ),
                AllocationRead::InstanceAttribute {
                    instance,
                    attribute,
                } => {
                    use aws_sdk_ec2::types::InstanceAttributeName as A;
                    let attribute = match attribute {
                        InstanceAttribute::UserData => A::UserData,
                        InstanceAttribute::InstanceInitiatedShutdownBehavior => {
                            A::InstanceInitiatedShutdownBehavior
                        }
                        InstanceAttribute::DisableApiTermination => A::DisableApiTermination,
                        InstanceAttribute::DisableApiStop => A::DisableApiStop,
                    };
                    send!(
                        self.ec2
                            .describe_instance_attribute()
                            .instance_id(instance.as_str())
                            .attribute(attribute)
                    )
                }
            };
            if let Err(reason) = result {
                query.failed_page(round_failure(&self.round, reason));
                break;
            }
            let (receipt, output) = match receiver.take() {
                Ok(v) => v,
                Err(_) => {
                    query.failed_page(round_failure(&self.round, ReadFailureV1::Malformed));
                    break;
                }
            };
            let terminal = receipt.continuation().is_none();
            if query
                .allocation_page(receipt, |sink| project(&output, &self.round, sink))
                .is_err()
                || terminal
            {
                break;
            }
        }
        Ok(query.finish())
    }
}
fn filters(
    scope: &QueryScopeV1,
    operation: ReadOperationV1,
) -> Option<Vec<aws_sdk_ec2::types::Filter>> {
    fn f(name: &str, value: &str) -> aws_sdk_ec2::types::Filter {
        aws_sdk_ec2::types::Filter::builder()
            .name(name)
            .values(value)
            .build()
    }
    use QueryScopeV1 as S;
    Some(match scope {
        S::ClientToken { token } => vec![f("client-token", token.as_str())],
        S::OperationTags {
            authority,
            operation,
        } => vec![
            f("tag:borrowser:authority-id", authority.as_str()),
            f("tag:borrowser:operation-id", operation.as_str()),
        ],
        S::AuthorityTag { authority } => vec![f("tag:borrowser:authority-id", authority.as_str())],
        S::OperationTag { operation } => vec![f("tag:borrowser:operation-id", operation.as_str())],
        S::AttachedTo { instance } => vec![f(
            if operation == ReadOperationV1::DescribeIamInstanceProfileAssociations {
                "instance-id"
            } else {
                "attachment.instance-id"
            },
            instance.as_str(),
        )],
        S::TypeOffering {
            instance_type,
            zone,
        } => vec![
            f("instance-type", instance_type.as_str()),
            f("location", zone.as_str()),
        ],
        _ => return None,
    })
}
fn read_failure<E: ProvideErrorMetadata>(error: &SdkError<E, HttpResponse>) -> ReadFailureV1 {
    if let SdkError::ServiceError(e) = error
        && !e.raw().status().is_success()
        && matches!(
            e.err().code(),
            Some(
                "InvalidAMIID.NotFound"
                    | "InvalidInstanceID.NotFound"
                    | "InvalidNetworkInterfaceID.NotFound"
                    | "InvalidVolume.NotFound"
                    | "InvalidAssociationID.NotFound"
            )
        )
    {
        return ReadFailureV1::NotFound;
    }
    super::identity_reads::read_failure(error)
}

#[cfg(test)]
#[path = "ec2_instance_attribute_tests.rs"]
mod attribute_tests;
#[cfg(test)]
#[path = "ec2_allocation_boundary_tests.rs"]
mod boundary_tests;
#[cfg(test)]
#[path = "ec2_allocation_owned_fields_tests.rs"]
mod owned_fields_tests;
#[cfg(test)]
#[path = "ec2_allocation_reads_tests.rs"]
mod tests;

//! A01–A38 read obligations, never admission predicates or an A23 verdict.
use super::{relationships::*, *};
use crate::{
    Result,
    provider::{coverage::*, inventory::ResourceIdentity as R, reviewed_subnet_routes_v1::*},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FactSet(u64);
impl FactSet {
    pub fn contains(self, fact: AdmissionFactId) -> bool {
        self.0 & (1 << fact as u64) != 0
    }
    fn insert(&mut self, fact: AdmissionFactId) {
        self.0 |= 1 << fact as u64;
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequiredRead {
    pub query: DiscoveryQuery,
    pub facts: FactSet,
    /// Source occurrences that introduced this work; no provider payload copies.
    pub observations: Vec<usize>,
    pub retained_seed: bool,
    pub stage: scheduling::WorkStage,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequiredReads {
    pub reads: BTreeMap<Vec<u8>, RequiredRead>,
    pub exhausted: bool,
    key_bytes: usize,
    trigger_bytes: usize,
}
impl RequiredReads {
    fn seed(
        &mut self,
        budget: &mut report_budget::ReportBudget,
        query: DiscoveryQuery,
        facts: &[AdmissionFactId],
    ) -> Result<()> {
        let stage = scheduling::WorkStage::seed(&query);
        self.add(budget, query, facts, std::iter::empty(), stage)
    }
    fn add(
        &mut self,
        budget: &mut report_budget::ReportBudget,
        query: DiscoveryQuery,
        facts: &[AdmissionFactId],
        observations: impl Iterator<Item = usize>,
        stage: scheduling::WorkStage,
    ) -> Result<()> {
        let key = query.key()?;
        if !self.reads.contains_key(&key) {
            if self.reads.len() == 128
                || self.key_bytes + key.len() > 64 << 10
                || !budget.charge(2 * key.len() + 64)
            {
                self.exhausted = true;
                return Ok(());
            }
            self.key_bytes += key.len();
            self.reads.insert(
                key.clone(),
                RequiredRead {
                    query,
                    facts: FactSet::default(),
                    observations: Vec::new(),
                    retained_seed: !matches!(stage, scheduling::WorkStage::Relationships { .. }),
                    stage,
                },
            );
        }
        let r = self.reads.get_mut(&key).expect("required query");
        for f in facts {
            r.facts.insert(*f);
        }
        for index in observations {
            if !r.observations.contains(&index) {
                if self.trigger_bytes + 8 > 16 << 10 || !budget.charge(8) {
                    self.exhausted = true;
                    break;
                }
                r.observations.push(index);
                self.trigger_bytes += 8;
            }
        }
        r.observations.sort_unstable();
        r.retained_seed |= !matches!(stage, scheduling::WorkStage::Relationships { .. });
        r.stage = r.stage.min(stage);
        Ok(())
    }
}
fn exact(id: R) -> QueryScopeV1 {
    QueryScopeV1::Exact {
        identities: vec![id],
    }
}
pub fn derive_required_reads(
    inputs: &DiscoveryInputs<'_>,
    graph: &mut Relationships,
) -> Result<RequiredReads> {
    use AdmissionFactId::*;
    use QueryScopeV1 as S;
    use ReadOperationV1 as O;
    let binding = &inputs.context.fields().binding;
    let m = &inputs.manifest;
    let launch = &inputs.prepared.specification.launch;
    let mut work = RequiredReads::default();
    let query = |operation, scope| {
        DiscoveryQuery::Existing(QueryIdentityV1 {
            operation,
            scope,
            account: binding.account_id.clone(),
            region: binding.region.clone(),
        })
    };
    // Seeds are always independent, regardless of results from any earlier query.
    work.seed(
        &mut graph.budget,
        query(
            O::DescribeInstances,
            S::ClientToken {
                token: binding.client_token.clone(),
            },
        ),
        &[A22, A23],
    )?;
    for operation in [
        O::DescribeInstances,
        O::DescribeNetworkInterfaces,
        O::DescribeVolumes,
    ] {
        for scope in [
            S::AuthorityTag {
                authority: binding.authority_id.clone(),
            },
            S::OperationTag {
                operation: binding.operation_id.clone(),
            },
        ] {
            work.seed(&mut graph.budget, query(operation, scope), &[A23, A24, A38])?;
        }
    }
    let seeds: Vec<(O, S, &[AdmissionFactId])> = vec![
        (O::GetCallerIdentity, S::Regional, &[A01]),
        (O::DescribeRegions, S::Regional, &[A02]),
        (O::DescribeAvailabilityZones, S::Regional, &[A03]),
        (
            O::DescribeSubnets,
            exact(R::Subnet(m.subnet.clone())),
            &[A04, A22],
        ),
        (
            O::DescribeVpcs,
            exact(R::Vpc(m.vpc.clone())),
            &[A05, A09, A22],
        ),
        (
            O::DescribeSecurityGroups,
            S::Exact {
                identities: m
                    .security_groups
                    .iter()
                    .map(|g| R::SecurityGroup(g.id.clone()))
                    .collect(),
            },
            &[A06, A25],
        ),
        (
            O::DescribeRouteTables,
            exact(R::RouteTable(m.route_table.clone())),
            &[A07],
        ),
        (
            O::DescribeVpcEndpoints,
            exact(R::Endpoint(m.s3_endpoint.clone())),
            &[A08],
        ),
        (
            O::DescribePrefixLists,
            exact(R::PrefixList(m.s3_prefix_list.clone())),
            &[A08],
        ),
        (
            O::DescribeDhcpOptions,
            exact(R::Dhcp(m.dns.dhcp_options.clone())),
            &[A09],
        ),
        (
            O::DescribeNetworkAcls,
            exact(R::Nacl(m.nacl.id.clone())),
            &[A10],
        ),
        (O::HeadBucket, exact(R::Bucket(m.bucket.clone())), &[A11]),
        (
            O::DescribeKey,
            exact(R::Key(
                inputs.prepared.deployment.support()?.kms_key_arn.clone(),
            )),
            &[A12],
        ),
        (
            O::DescribeImages,
            exact(R::Image(launch.ami.image_id.clone())),
            &[A13, A14, A15, A16, A38],
        ),
        (
            O::DescribeInstanceTypes,
            exact(R::InstanceType(launch.instance_type.clone())),
            &[A17, A18, A36],
        ),
        (
            O::DescribeInstanceTypeOfferings,
            S::TypeOffering {
                instance_type: launch.instance_type.clone(),
                zone: launch.availability_zone.clone(),
            },
            &[A19],
        ),
        (
            O::GetInstanceProfile,
            exact(R::Profile(launch.instance_profile_arn.clone())),
            &[A20],
        ),
    ];
    for (op, scope, facts) in seeds {
        work.seed(&mut graph.budget, query(op, scope), facts)?;
    }
    for attribute in [
        VpcAttribute::EnableDnsSupport,
        VpcAttribute::EnableDnsHostnames,
    ] {
        work.seed(
            &mut graph.budget,
            query(
                O::DescribeVpcAttribute,
                S::VpcAttribute {
                    vpc: m.vpc.clone(),
                    attribute,
                },
            ),
            &[A09],
        )?;
    }
    work.seed(
        &mut graph.budget,
        DiscoveryQuery::ReviewedSubnetRoutes(ReviewedSubnetRouteQueryV1 {
            schema_version: 1,
            operation: O::DescribeRouteTables,
            account: m.account.clone(),
            region: m.region.clone(),
            association_subnet: m.subnet.clone(),
        }),
        &[A07],
    )?;
    for (id, resource) in &graph.resources {
        // A positional relation justifies work from both source perspectives.
        // Borrow the already bounded edges; no extra trigger/scratch index.
        let edges = &graph.relationships;
        let refs = || {
            resource.observations.iter().copied().chain(
                edges
                    .iter()
                    .filter_map(|edge| {
                        if (&edge.from == id || &edge.to == id)
                            && let RelationshipKind::PositionalParent { parent } = edge.kind
                            && let Some(child) = edge.observation
                        {
                            Some([parent, child])
                        } else {
                            None
                        }
                    })
                    .flatten(),
            )
        };
        let stage = if resource.prior {
            scheduling::WorkStage::PriorIdentities
        } else {
            scheduling::WorkStage::Relationships {
                depth: resource.discovery_depth.unwrap_or(u16::MAX),
            }
        };
        let (op, facts): (_, &[_]) = match id {
            AllocationIdentity::Instance(_) => (
                O::DescribeInstances,
                &[
                    A21, A22, A23, A24, A25, A26, A27, A28, A29, A30, A32, A33, A34, A35, A36, A38,
                ],
            ),
            AllocationIdentity::NetworkInterface(_) => (
                O::DescribeNetworkInterfaces,
                &[A23, A24, A25, A26, A27, A38],
            ),
            AllocationIdentity::Volume(_) => (O::DescribeVolumes, &[A16, A23, A24, A28, A29, A38]),
            AllocationIdentity::ProfileAssociation(_) => {
                (O::DescribeIamInstanceProfileAssociations, &[A21, A23])
            }
        };
        work.add(
            &mut graph.budget,
            query(op, exact(id.resource())),
            facts,
            refs(),
            stage,
        )?;
        if let AllocationIdentity::Instance(instance) = id {
            for (op, facts) in [
                (O::DescribeNetworkInterfaces, &[A23, A25, A26, A27][..]),
                (O::DescribeVolumes, &[A16, A23, A28, A29][..]),
                (O::DescribeIamInstanceProfileAssociations, &[A21, A23][..]),
            ] {
                work.add(
                    &mut graph.budget,
                    query(
                        op,
                        S::AttachedTo {
                            instance: instance.clone(),
                        },
                    ),
                    facts,
                    refs(),
                    stage,
                )?;
            }
            for attribute in [
                InstanceAttribute::UserData,
                InstanceAttribute::InstanceInitiatedShutdownBehavior,
                InstanceAttribute::DisableApiTermination,
                InstanceAttribute::DisableApiStop,
            ] {
                let fact = if attribute == InstanceAttribute::UserData {
                    A37
                } else {
                    A31
                };
                work.add(
                    &mut graph.budget,
                    query(
                        O::DescribeInstanceAttribute,
                        S::InstanceAttribute {
                            instance: instance.clone(),
                            attribute,
                        },
                    ),
                    &[fact],
                    refs(),
                    stage,
                )?;
            }
        }
    }
    Ok(work)
}

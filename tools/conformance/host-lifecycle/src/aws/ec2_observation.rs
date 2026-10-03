//! Pure returned-field projection. Reviewed/requested identities never enter normalization.
use super::{
    ec2_decode_integrity::Ec2Output, ec2_network_observation as network,
    response_limits::ObservationRound,
};
use crate::provider::{
    coverage::ReadFailureV1,
    ec2_observation_v4::*,
    identity_observation_v3::MemberRepresentationFailureV3,
    limits::LimitKind,
    management_observation_v2::{ObservationValueV2, UnavailableEvidenceV2},
    observation::EvidenceList,
};
pub(super) type ReadResult<T> = std::result::Result<T, ReadFailureV1>;
/// Keep the first pending limit; promote a limit over any query-local failure,
/// otherwise keep the first failure. This does not latch the round: e2a must
/// retain eligible evidence first, and its already-latched round/session/time
/// failure still takes precedence over this pending normalization disposition.
pub(super) fn combine_failure(
    pending: Option<ReadFailureV1>,
    next: Option<ReadFailureV1>,
) -> Option<ReadFailureV1> {
    match (pending, next) {
        (Some(ReadFailureV1::Limit(_)), _) => pending,
        (_, Some(ReadFailureV1::Limit(_))) => next,
        _ => pending.or(next),
    }
}
pub(super) fn member<T: Ec2LexicalV4>(value: Option<&str>) -> Ec2MemberV4<T> {
    member_bound(value, 2048)
}
pub(super) fn member_bound<T: Ec2LexicalV4>(value: Option<&str>, limit: usize) -> Ec2MemberV4<T> {
    match value {
        None => Ec2MemberV4::NotReturned,
        Some("") => Ec2MemberV4::Empty,
        Some(v) if v.len() > limit => {
            Ec2MemberV4::Unrepresentable(MemberRepresentationFailureV3::TextBytes)
        }
        Some(v) if v.contains('\0') => {
            Ec2MemberV4::Unrepresentable(MemberRepresentationFailureV3::ContainsNul)
        }
        Some(v) => match T::parse_literal(v) {
            Ok(v) => Ec2MemberV4::Present(v),
            Err(_) => Ec2MemberV4::Malformed(v.to_owned().try_into().expect("bounded member")),
        },
    }
}
pub(super) fn value<T>(value: Option<T>) -> ObservationValueV2<T> {
    value.map_or(
        ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotReturned),
        ObservationValueV2::Present,
    )
}
pub(super) struct Normalizer<'a> {
    pub round: &'a ObservationRound,
    pub policy_occurrences: u64,
}
impl Normalizer<'_> {
    pub fn list<T, U>(
        &mut self,
        values: Option<&Vec<T>>,
        mut map: impl FnMut(&mut Self, &T) -> ReadResult<U>,
    ) -> ReadResult<ObservationValueV2<EvidenceList<U>>> {
        let Some(values) = values else {
            return Ok(value(None));
        };
        if values.len() > 128 {
            return Err(ReadFailureV1::Limit(LimitKind::Records));
        }
        let mut result = Vec::with_capacity(values.len());
        for item in values {
            self.check()?;
            result.push(map(self, item)?);
        }
        Ok(ObservationValueV2::Present(
            result.try_into().expect("bounded list"),
        ))
    }
    pub fn check(&self) -> ReadResult<()> {
        self.round.remaining().map(|_| ()).map_err(|_| {
            super::query_execution::round_failure(self.round, ReadFailureV1::Malformed)
        })
    }
}
pub(super) struct NormalizedPage {
    pub data: Vec<ObservationDataV4>,
    pub occurrences: u64,
    pub returned_token: Option<String>,
    pub incomplete: Option<ReadFailureV1>,
}
pub(super) fn normalize(
    output: Ec2Output,
    wire_occurrences: u64,
    round: &ObservationRound,
) -> NormalizedPage {
    let mut n = Normalizer {
        round,
        policy_occurrences: 0,
    };
    let mut page = NormalizedPage {
        data: Vec::new(),
        occurrences: wire_occurrences,
        returned_token: None,
        incomplete: None,
    };
    macro_rules! records {
        ($output:ident,$field:ident,$convert:expr) => {
            if let Some(items) = &$output.$field {
                for item in items {
                    let converted = n.check().and_then(|_| $convert(&mut n, item));
                    match converted {
                        Ok(data) => match data.facts() {
                            Ok(facts) => {
                                page.incomplete = combine_failure(page.incomplete, facts.failure);
                                page.data.push(data);
                            }
                            Err(_) => {
                                page.incomplete = combine_failure(
                                    page.incomplete,
                                    Some(ReadFailureV1::Malformed),
                                );
                                break;
                            }
                        },
                        Err(reason) => {
                            page.incomplete = combine_failure(page.incomplete, Some(reason));
                            break;
                        }
                    }
                }
            } else {
                page.incomplete = combine_failure(page.incomplete, Some(ReadFailureV1::Malformed));
            }
        };
    }
    match output {
        Ec2Output::Regions(o) => records!(o, regions, region),
        Ec2Output::AvailabilityZones(o) => records!(o, availability_zones, zone),
        Ec2Output::Subnets(o) => {
            records!(o, subnets, subnet);
            page.returned_token = o.next_token;
        }
        Ec2Output::Vpcs(o) => {
            records!(o, vpcs, vpc);
            page.returned_token = o.next_token;
        }
        Ec2Output::SecurityGroups(o) => {
            records!(o, security_groups, network::security_group);
            page.returned_token = o.next_token;
        }
        Ec2Output::RouteTables(o) => {
            records!(o, route_tables, network::route_table);
            page.returned_token = o.next_token;
        }
        Ec2Output::VpcEndpoints(o) => {
            records!(o, vpc_endpoints, network::endpoint);
            page.returned_token = o.next_token;
        }
        Ec2Output::PrefixLists(o) => {
            records!(o, prefix_lists, network::prefix_list);
            page.returned_token = o.next_token;
        }
        Ec2Output::DhcpOptions(o) => {
            records!(o, dhcp_options, network::dhcp);
            page.returned_token = o.next_token;
        }
        Ec2Output::NetworkAcls(o) => {
            records!(o, network_acls, network::nacl);
            page.returned_token = o.next_token;
        }
        Ec2Output::VpcAttribute(o) => {
            let data = ObservationDataV4::Dns {
                vpc: member(o.vpc_id()),
                support: value(o.enable_dns_support().map(|v| {
                    crate::provider::network_observation_v4::BooleanAttributeV4 {
                        value: value(v.value()),
                    }
                })),
                hostnames: value(o.enable_dns_hostnames().map(|v| {
                    crate::provider::network_observation_v4::BooleanAttributeV4 {
                        value: value(v.value()),
                    }
                })),
            };
            page.occurrences += 1;
            match data.facts() {
                Ok(facts) => {
                    page.incomplete = combine_failure(page.incomplete, facts.failure);
                    page.data.push(data);
                }
                Err(_) => {
                    page.incomplete =
                        combine_failure(page.incomplete, Some(ReadFailureV1::Malformed));
                }
            }
        }
    }
    page.occurrences += n.policy_occurrences;
    page
}
fn region(_: &mut Normalizer<'_>, v: &aws_sdk_ec2::types::Region) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::Region {
        name: member(v.region_name()),
        opt_in_status: member(v.opt_in_status()),
    })
}
fn zone(
    _: &mut Normalizer<'_>,
    v: &aws_sdk_ec2::types::AvailabilityZone,
) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::AvailabilityZone {
        name: member(v.zone_name()),
        id: member(v.zone_id()),
        region: member(v.region_name()),
        state: member(v.state().map(|s| s.as_str())),
    })
}
fn subnet(_: &mut Normalizer<'_>, v: &aws_sdk_ec2::types::Subnet) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::Subnet {
        id: member(v.subnet_id()),
        owner: member(v.owner_id()),
        vpc: member(v.vpc_id()),
        zone: member(v.availability_zone()),
        zone_id: member(v.availability_zone_id()),
        ipv4: member(v.cidr_block()),
        ipv6_native: value(v.ipv6_native()),
        assign_ipv6: value(v.assign_ipv6_address_on_creation()),
        assign_public_ipv4: value(v.map_public_ip_on_launch()),
        outpost: member(v.outpost_arn()),
        customer_owned_pool: member(v.customer_owned_ipv4_pool()),
    })
}
fn vpc(_: &mut Normalizer<'_>, v: &aws_sdk_ec2::types::Vpc) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::Vpc {
        id: member(v.vpc_id()),
        owner: member(v.owner_id()),
        tenancy: member(v.instance_tenancy().map(|s| s.as_str())),
        dhcp: member(v.dhcp_options_id()),
    })
}

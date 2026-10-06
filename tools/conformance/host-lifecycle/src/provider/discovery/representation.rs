//! Discovery-specific representation audit. Historical minimum credit is not a proof.
use super::policy_count;
use crate::provider::{
    coverage::{CoverageStatus, ReadCoverageV1, ReadOperationV1},
    ec2_observation_v4::{FactsV4, ObservationDataV4},
    observation_v5::ObservationEntryV5,
    source_occurrence_v5::{
        CollectionShapeV5 as Shape, ProjectionKindV5 as P, SourceListV5 as L, SourceOccurrenceV5,
        SourcePathV5 as Path,
    },
};
use crate::{Result, require};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RepresentationGap {
    MissingSourcePosition,
    MissingParentProjection,
    MissingSiblingProjection,
    MissingChildProjection,
    UnrepresentedSourceOccurrences,
    UnverifiableSourceRepresentation,
    AuditBudgetExhausted,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationAudit {
    /// None when structure or a lossy historical marker prevents exact counting.
    pub represented_sources: Option<u64>,
    pub gaps: BTreeSet<RepresentationGap>,
}
impl RepresentationAudit {
    pub fn complete(&self) -> bool {
        self.gaps.is_empty()
    }
}
fn extracted(list: L) -> bool {
    matches!(
        list,
        L::Instances
            | L::InstanceNetworkInterfaces
            | L::InstanceEbsMappings
            | L::InstanceSecondaryInterfaces
            | L::VolumeAttachments
    )
}
fn projections(path: Path) -> &'static [P] {
    match path {
        Path::Image { .. } => &[P::Image, P::ExcludedFeatures],
        Path::InstanceType { .. } => &[P::InstanceType],
        Path::TypeOffering { .. } => &[P::TypeOffering],
        Path::ProfileAssociation { .. } => &[P::ProfileAssociation],
        Path::Reservation { .. } => &[P::Reservation],
        Path::Instance { .. } => &[P::Instance, P::InstanceOptions, P::ExcludedFeatures],
        Path::InstanceNetworkInterface { .. } => &[P::NetworkInterface, P::InstanceEniAttachment],
        Path::InstanceEbsMapping { .. } => &[P::InstanceEbsMapping],
        Path::InstanceSecondaryInterface { .. } => &[P::UnsupportedSecondaryInterface],
        Path::NetworkInterface { .. } => &[
            P::NetworkInterface,
            P::StandaloneEniAttachment,
            P::ExcludedFeatures,
        ],
        Path::Volume { .. } => &[P::Volume, P::ExcludedFeatures],
        Path::VolumeAttachment { .. } => &[P::VolumeAttachment],
        Path::InstanceAttribute => &[P::InstanceAttributes],
    }
}

/// Inputs must have passed bounded ingestion and their frozen validators. This
/// function neither changes coverage nor discards any evidence occurrence.
pub(crate) fn audit_query<'a>(
    coverage: &ReadCoverageV1,
    records: impl Iterator<Item = &'a ObservationEntryV5>,
) -> Result<RepresentationAudit> {
    let mut sources: BTreeMap<SourceOccurrenceV5, BTreeSet<P>> = BTreeMap::new();
    let mut lists = BTreeMap::<_, BTreeSet<u64>>::new();
    let mut claims = BTreeMap::new();
    let mut legacy = 0;
    let mut legacy_records = 0;
    let mut gaps = BTreeSet::new();
    let mut exact = true;
    let mut metadata_bytes = 0usize;
    for entry in records {
        require(
            entry.query() == &coverage.query,
            "representation query binding",
        )?;
        match entry {
            ObservationEntryV5::V5(v) => {
                let declared = v.data.claims();
                // Conservatively charge before inserting into any audit index.
                // Scratch is query-local and freed before the next audit.
                metadata_bytes += 128 + declared.len() * 64;
                if metadata_bytes > 64 << 10 {
                    gaps.insert(RepresentationGap::AuditBudgetExhausted);
                    return Ok(RepresentationAudit {
                        represented_sources: None,
                        gaps,
                    });
                }
                sources
                    .entry(v.source)
                    .or_default()
                    .insert(v.data.projection());
                if let Some((parent, list, index)) = v.source.path.containing_list() {
                    lists
                        .entry((v.source.page, parent, list))
                        .or_default()
                        .insert(index);
                }
                for (list, shape) in declared {
                    claims.insert((v.source.page, Some(v.source.path), list), shape);
                }
            }
            ObservationEntryV5::V4(v) => {
                legacy_records += 1;
                let facts = v.data.facts()?;
                require(
                    (facts.failure.is_none() && selected_dns_present(&v.query, &v.data))
                        || coverage.status != CoverageStatus::Complete,
                    "complete coverage with failed infrastructure evidence",
                )?;
                if let ObservationDataV4::Endpoint {
                    policy,
                    route_tables,
                    ..
                } = &v.data
                {
                    if let Some(work) = policy_count::count(policy) {
                        legacy += 1 + route_tables.facts()?.occurrences + work;
                    } else {
                        exact = false;
                    }
                } else {
                    legacy += facts.occurrences;
                }
            }
            ObservationEntryV5::V3(v) => {
                legacy_records += 1;
                require(
                    !identity_failed(&v.data) || coverage.status != CoverageStatus::Complete,
                    "complete coverage with failed identity evidence",
                )?;
                legacy += v.data.minimum_occurrences();
            }
            ObservationEntryV5::V2(v) => {
                use crate::provider::{observation::Observed, observation_v2::ObservationDataV2};
                let present = match &v.data {
                    ObservationDataV2::Caller {
                        account,
                        arn,
                        user_id,
                    } => {
                        matches!(account, Observed::Present(_))
                            && matches!(arn, Observed::Present(_))
                            && matches!(user_id, Observed::Present(_))
                    }
                    ObservationDataV2::Bucket { region, .. } => {
                        matches!(region, Observed::Present(_))
                    }
                    _ => false,
                };
                require(
                    present || coverage.status != CoverageStatus::Complete,
                    "complete coverage with failed singleton evidence",
                )?;
                legacy_records += 1;
                legacy += 1;
            }
        }
    }
    for (source, actual) in &sources {
        if projections(source.path).iter().any(|p| !actual.contains(p)) {
            gaps.insert(RepresentationGap::MissingSiblingProjection);
        }
        if let Some((Some(parent), _, _)) = source.path.containing_list() {
            let parent = SourceOccurrenceV5 {
                page: source.page,
                path: parent,
            };
            if !sources
                .get(&parent)
                .is_some_and(|p| p.contains(&projections(parent.path)[0]))
            {
                gaps.insert(RepresentationGap::MissingParentProjection);
            }
        }
    }
    for indices in lists.values() {
        if !indices.iter().copied().eq(0..indices.len() as u64) {
            gaps.insert(RepresentationGap::MissingSourcePosition);
        }
    }
    for (key, shape) in &claims {
        if extracted(key.2)
            && let Shape::Present { count } = shape
        {
            let actual = lists.get(key);
            if actual.map_or(0, BTreeSet::len) as u64 != u64::from(*count) {
                gaps.insert(RepresentationGap::MissingChildProjection);
            }
        }
    }
    let singleton = matches!(
        coverage.query.operation,
        ReadOperationV1::GetCallerIdentity
            | ReadOperationV1::HeadBucket
            | ReadOperationV1::GetInstanceProfile
            | ReadOperationV1::DescribeKey
            | ReadOperationV1::DescribeVpcAttribute
            | ReadOperationV1::DescribeInstanceAttribute
    );
    require(
        !singleton || legacy_records + sources.len() <= 1,
        "multiple singleton sources",
    )?;
    if singleton && legacy_records + sources.len() != 1 {
        gaps.insert(RepresentationGap::UnrepresentedSourceOccurrences);
    }
    let represented_sources = if gaps.is_empty() && exact {
        // Count actual source nodes, never prefixes inferred from source indices.
        // Inline lists are held by a single projection; extracted lists were
        // checked above and their actual children already belong to `sources`.
        let inline: u64 = claims
            .iter()
            .filter(|(key, _)| !extracted(key.2))
            .map(|(_, s)| match s {
                Shape::Present { count } => u64::from(*count),
                _ => 0,
            })
            .sum();
        let actual = legacy + sources.len() as u64 + inline;
        require(
            actual <= coverage.records,
            "represented sources exceed coverage",
        )?;
        if actual != coverage.records {
            gaps.insert(RepresentationGap::UnrepresentedSourceOccurrences);
        }
        Some(actual)
    } else {
        None
    };
    if !exact {
        gaps.insert(RepresentationGap::UnverifiableSourceRepresentation);
    }
    Ok(RepresentationAudit {
        represented_sources,
        gaps,
    })
}

// This requirement belongs to DescribeVpcAttribute's adapter, not the frozen
// generic V4 data validator. The unsolicited sibling cannot satisfy it.
fn selected_dns_present(
    query: &crate::provider::coverage::QueryIdentityV1,
    data: &ObservationDataV4,
) -> bool {
    use crate::provider::{
        coverage::{QueryScopeV1, VpcAttribute},
        management_observation_v2::ObservationValueV2 as V,
    };
    let QueryScopeV1::VpcAttribute { attribute, .. } = query.scope else {
        return true;
    };
    let ObservationDataV4::Dns {
        support, hostnames, ..
    } = data
    else {
        return false;
    };
    let selected = match attribute {
        VpcAttribute::EnableDnsSupport => support,
        VpcAttribute::EnableDnsHostnames => hostnames,
    };
    matches!(selected, V::Present(v) if matches!(v.value, V::Present(_)))
}

fn identity_failed(data: &crate::provider::observation_v3::ObservationDataV3) -> bool {
    use crate::provider::{
        identity_observation_v3::IdentityMemberV3 as M,
        management_observation_v2::ObservationValueV2 as V, observation_v3::ObservationDataV3 as D,
    };
    fn failed<T>(v: &M<T>) -> bool {
        matches!(v, M::Malformed(_) | M::Unrepresentable(_))
    }
    match data {
        D::Key {
            metadata: V::Present(v),
        } => {
            failed(&v.arn)
                || failed(&v.account)
                || failed(&v.manager)
                || failed(&v.spec)
                || failed(&v.usage)
                || failed(&v.state)
        }
        D::Profile {
            profile: V::Present(v),
        } => {
            failed(&v.arn)
                || failed(&v.id)
                || matches!(&v.roles,V::Present(roles) if roles.as_slice().iter().any(|r|failed(&r.arn)||failed(&r.id)))
        }
        _ => false,
    }
}

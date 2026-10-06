//! Allocation references only. Reviewed infrastructure is never traversed as an inventory.
use super::*;
use crate::{
    identity::*,
    provider::{
        allocation_value_v5::MemberV5 as M,
        coverage::QueryScopeV1 as S,
        ec2_allocation_observation_v5::{ObservationDataV5 as D, TagV5},
        management_observation_v2::ObservationValueV2 as V,
        observation::EvidenceList,
        observation_v5::ObservationEntryV5,
    },
};
use std::collections::{BTreeMap, BTreeSet};

#[path = "positional_relationships.rs"]
mod positional;
pub use positional::{ParentIdentityAgreement, PositionalParentFact};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationIdentity {
    Instance(InstanceId),
    NetworkInterface(NetworkInterfaceId),
    Volume(VolumeId),
    ProfileAssociation(ProfileAssociationId),
}
impl AllocationIdentity {
    pub(crate) fn logical_bytes(&self) -> usize {
        16 + match self {
            Self::Instance(v) => v.as_str().len(),
            Self::NetworkInterface(v) => v.as_str().len(),
            Self::Volume(v) => v.as_str().len(),
            Self::ProfileAssociation(v) => v.as_str().len(),
        }
    }
    pub(crate) fn resource(&self) -> crate::provider::inventory::ResourceIdentity {
        use crate::provider::inventory::ResourceIdentity as R;
        match self {
            Self::Instance(v) => R::Instance(v.clone()),
            Self::NetworkInterface(v) => R::NetworkInterface(v.clone()),
            Self::Volume(v) => R::Volume(v.clone()),
            Self::ProfileAssociation(v) => R::ProfileAssociation(v.clone()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribution {
    DiscoveryHit,
    PlausiblyLinked,
    ContradictoryLinkage,
    AffirmativelyUnrelated,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceAttribution {
    pub identity: AllocationIdentity,
    /// Indices in the canonical outer record order; duplicate observations remain separate.
    pub observations: Vec<usize>,
    pub attribution: Attribution,
    /// Monotone operation linkage, retained even when attribution is contradictory.
    pub positive_linkage: bool,
    pub prior: bool,
    pub query_references: Vec<usize>,
    /// None means supplied evidence has no retained/independent origin for this ID.
    pub discovery_depth: Option<u16>,
    pub agreement: relationship_agreement::RelationshipAgreement,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationshipOccurrence {
    pub from: AllocationIdentity,
    pub to: AllocationIdentity,
    pub observation: Option<usize>,
    pub kind: RelationshipKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipKind {
    Retained,
    RecordedReference,
    QueryReference,
    SameSource,
    /// `observation` is the child; `parent` is its actual positional projection.
    /// Structural containment is not a confirmed provider attachment.
    PositionalParent {
        parent: usize,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipGapKind {
    PriorEvidenceUnavailable,
    ReferenceUnavailable,
    CollectionUnavailable,
    UnsupportedReference,
    OpaqueManagedReference,
    ParentProjectionUnavailable,
    ParentIdentityUnavailable,
    EnclosingIdentityUnavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationshipGap {
    pub observation: Option<usize>,
    pub kind: RelationshipGapKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Relationships {
    pub resources: BTreeMap<AllocationIdentity, ResourceAttribution>,
    pub relationships: Vec<RelationshipOccurrence>,
    pub unresolved_references: bool,
    pub exhausted: bool,
    pub gaps: Vec<RelationshipGap>,
    pub facts: Vec<relationship_agreement::RelationshipFactRef>,
    pub positional_parents: Vec<PositionalParentFact>,
    pub(super) budget: report_budget::ReportBudget,
    bytes: usize,
}
impl Relationships {
    fn gap(&mut self, observation: Option<usize>, kind: RelationshipGapKind) {
        self.unresolved_references = true;
        if self.charge(16) {
            self.gaps.push(RelationshipGap { observation, kind });
        }
    }
    fn charge(&mut self, bytes: usize) -> bool {
        if self.exhausted || self.bytes + bytes > 32 << 10 || !self.budget.charge(bytes) {
            self.exhausted = true;
            return false;
        }
        self.bytes += bytes;
        true
    }
    fn add(
        &mut self,
        id: AllocationIdentity,
        occurrence: Option<usize>,
        positive: bool,
        prior: bool,
    ) {
        if !self.resources.contains_key(&id) {
            if self.resources.len() == 4096 || !self.charge(2 * id.logical_bytes() + 64) {
                self.exhausted = true;
                return;
            }
            self.resources.insert(
                id.clone(),
                ResourceAttribution {
                    identity: id.clone(),
                    observations: Vec::new(),
                    attribution: Attribution::DiscoveryHit,
                    positive_linkage: false,
                    prior,
                    query_references: Vec::new(),
                    discovery_depth: prior.then_some(0),
                    agreement: Default::default(),
                },
            );
        }
        if occurrence.is_some() && !self.charge(8) {
            return;
        }
        let r = self.resources.get_mut(&id).expect("inserted resource");
        if let Some(i) = occurrence
            && r.observations.last() != Some(&i)
        {
            r.observations.push(i);
        }
        if positive {
            r.attribution = Attribution::PlausiblyLinked;
            r.positive_linkage = true;
        }
        r.prior |= prior;
        if prior {
            r.discovery_depth = Some(0);
        }
    }
    fn relate(
        &mut self,
        a: AllocationIdentity,
        b: AllocationIdentity,
        record: Option<usize>,
        kind: RelationshipKind,
    ) {
        if a == b {
            return;
        }
        if self.relationships.len() == 4096
            || !self.charge(a.logical_bytes() + b.logical_bytes() + 24)
        {
            self.exhausted = true;
            return;
        }
        self.relationships.push(RelationshipOccurrence {
            from: a,
            to: b,
            observation: record,
            kind,
        });
    }
}
fn conflict(
    graph: &mut Relationships,
    conflicts: &mut BTreeSet<AllocationIdentity>,
    id: &AllocationIdentity,
) {
    if !conflicts.contains(id) && graph.charge(id.logical_bytes() + 16) {
        conflicts.insert(id.clone());
    }
}
fn member<T: Clone>(
    v: &M<T>,
    wrap: impl Fn(T) -> AllocationIdentity,
    ids: &mut Vec<AllocationIdentity>,
    unresolved: &mut bool,
) {
    match v {
        M::Present(v) => ids.push(wrap(v.clone())),
        _ => *unresolved = true,
    }
}
fn tag_matches(tags: &V<EvidenceList<TagV5>>, key: &str, expected: &str) -> bool {
    matches!(tags,V::Present(tags) if tags.as_slice().iter().any(|t|
        matches!((&t.key,&t.value),(M::Present(k),M::Present(v)) if k.as_str()==key && v.as_str()==expected)))
}
pub(crate) const AUTHORITY_TAG: &str = "borrowser:authority-id";
pub(crate) const OPERATION_TAG: &str = "borrowser:operation-id";

pub(super) fn derive(
    inputs: &DiscoveryInputs<'_>,
    evidence: &ValidatedDiscoveryEvidence,
) -> Relationships {
    use AllocationIdentity as I;
    let mut graph = Relationships::default();
    let binding = &inputs.context.fields().binding;
    if let Some(prior) = &inputs.context.fields().prior_provider {
        if let Some(id) = &prior.bound_instance {
            graph.add(I::Instance(id.clone()), None, true, true);
        }
        if inputs.prior.is_none() {
            graph.gap(None, RelationshipGapKind::PriorEvidenceUnavailable);
        }
    }
    if let Some(prior) = inputs.prior {
        if !prior.fully_supplied {
            graph.gap(None, RelationshipGapKind::PriorEvidenceUnavailable);
        }
        // Charge borrowed ordering scratch before constructing it. Counts are
        // preflighted at input ingestion; no hidden index grows outside the cap.
        if !graph.charge(8 * (prior.resources.len() + 3 * prior.relationships.len())) {
            return graph;
        }
        // Outer order never controls bounded selection.
        let mut ids: BTreeSet<_> = prior.resources.iter().collect();
        for (a, b) in &prior.relationships {
            ids.insert(a);
            ids.insert(b);
        }
        for id in ids {
            graph.add(id.clone(), None, true, true);
        }
        let mut edges: Vec<_> = prior.relationships.iter().collect();
        edges.sort();
        for (a, b) in edges {
            graph.relate(a.clone(), b.clone(), None, RelationshipKind::Retained);
        }
    }
    let mut source_ids: BTreeMap<_, Vec<AllocationIdentity>> = BTreeMap::new();
    let mut contradictory = BTreeSet::new();
    for (index, entry) in evidence.evidence.observations.records.iter().enumerate() {
        let ObservationEntryV5::V5(record) = entry else {
            continue;
        };
        if let Some(kind) = relationship_agreement::kind(&record.data)
            && graph.charge(16)
        {
            graph
                .facts
                .push(relationship_agreement::RelationshipFactRef {
                    observation: index,
                    kind,
                });
        }
        if relationship_agreement::opaque(&record.data) {
            graph.gap(Some(index), RelationshipGapKind::OpaqueManagedReference);
        }
        let mut ids = Vec::new();
        let mut unresolved = false;
        let mut positive = matches!(&record.query.scope,
            S::ClientToken {token} if token == &binding.client_token)
            || matches!(&record.query.scope,S::OperationTag {operation}|S::OperationTags{operation,..} if operation == &binding.operation_id);
        macro_rules! id {
            ($value:expr,$variant:ident) => {
                member($value, I::$variant, &mut ids, &mut unresolved)
            };
        }
        match &record.data {
            D::Reservation(v) => {
                unresolved |= !matches!(
                    v.instances,
                    crate::provider::source_occurrence_v5::CollectionShapeV5::Present { .. }
                );
            }
            D::Instance(v) => {
                id!(&v.id, Instance);
                positive |=
                    matches!(&v.token,M::Present(v) if v.as_str()==binding.client_token.as_str());
                positive |= tag_matches(&v.tags, OPERATION_TAG, binding.operation_id.as_str());
                unresolved |= [&v.interfaces, &v.ebs_mappings, &v.secondary_interfaces]
                    .iter()
                    .any(|s| {
                        !matches!(
                            s,
                            crate::provider::source_occurrence_v5::CollectionShapeV5::Present { .. }
                        )
                    });
            }
            D::InstanceOptions(v) => id!(&v.id, Instance),
            D::ExcludedFeatures(v) => {
                use crate::provider::source_occurrence_v5::SourcePathV5 as P;
                let parsed = match (&record.source.path, &v.resource_id) {
                    (P::Instance { .. }, M::Present(v)) => v.as_str().parse().ok().map(I::Instance),
                    (P::NetworkInterface { .. }, M::Present(v)) => {
                        v.as_str().parse().ok().map(I::NetworkInterface)
                    }
                    (P::Volume { .. }, M::Present(v)) => v.as_str().parse().ok().map(I::Volume),
                    (P::Image { .. }, _) => None,
                    _ => {
                        unresolved = true;
                        None
                    }
                };
                if let Some(id) = parsed {
                    ids.push(id);
                } else if !matches!(record.source.path, P::Image { .. }) {
                    unresolved = true;
                }
            }
            D::NetworkInterface(v) => {
                id!(&v.id, NetworkInterface);
                if matches!(
                    record.source.path,
                    crate::provider::source_occurrence_v5::SourcePathV5::InstanceNetworkInterface { .. }
                ) {
                    id!(&v.enclosing_instance, Instance);
                }
                positive |= tag_matches(&v.tags, OPERATION_TAG, binding.operation_id.as_str());
            }
            D::InstanceEniAttachment(v) => {
                id!(&v.enclosing_instance, Instance);
                id!(&v.enclosing_interface, NetworkInterface);
                unresolved |= !matches!(v.attachment, V::Present(_));
            }
            D::StandaloneEniAttachment(v) => {
                id!(&v.enclosing_interface, NetworkInterface);
                if let V::Present(v) = &v.attachment {
                    id!(&v.instance, Instance);
                } else {
                    unresolved = true;
                }
            }
            D::InstanceEbsMapping(v) => {
                id!(&v.enclosing_instance, Instance);
                if let V::Present(v) = &v.ebs {
                    id!(&v.volume, Volume);
                } else {
                    unresolved = true;
                }
            }
            D::Volume(v) => {
                id!(&v.id, Volume);
                positive |= tag_matches(&v.tags, OPERATION_TAG, binding.operation_id.as_str());
                unresolved |= !matches!(
                    v.attachments,
                    crate::provider::source_occurrence_v5::CollectionShapeV5::Present { .. }
                );
            }
            D::VolumeAttachment(v) => {
                id!(&v.enclosing_volume, Volume);
                id!(&v.volume, Volume);
                id!(&v.instance, Instance);
            }
            D::ProfileAssociation(v) => {
                id!(&v.association, ProfileAssociation);
                id!(&v.instance, Instance);
            }
            D::InstanceAttributes(v) => id!(&v.id, Instance),
            D::UnsupportedSecondaryInterface(v) => {
                id!(&v.enclosing_instance, Instance);
                unresolved = true;
            }
            _ => (),
        }
        if unresolved {
            let kind = match &record.data {
                D::UnsupportedSecondaryInterface(_) => RelationshipGapKind::UnsupportedReference,
                D::Reservation(_) => RelationshipGapKind::CollectionUnavailable,
                _ => RelationshipGapKind::ReferenceUnavailable,
            };
            graph.gap(Some(index), kind);
        }
        let requested = match &record.query.scope {
            S::AttachedTo { instance } | S::InstanceAttribute { instance, .. } => {
                Some(I::Instance(instance.clone()))
            }
            S::Exact { identities } if identities.len() == 1 => match &identities[0] {
                crate::provider::inventory::ResourceIdentity::Instance(v) => {
                    Some(I::Instance(v.clone()))
                }
                crate::provider::inventory::ResourceIdentity::NetworkInterface(v) => {
                    Some(I::NetworkInterface(v.clone()))
                }
                crate::provider::inventory::ResourceIdentity::Volume(v) => {
                    Some(I::Volume(v.clone()))
                }
                crate::provider::inventory::ResourceIdentity::ProfileAssociation(v) => {
                    Some(I::ProfileAssociation(v.clone()))
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(requested) = requested
            && !ids.is_empty()
        {
            // The requested identity is a query reference, never a substituted
            // returned field. A contradictory returned identity remains separate.
            graph.add(requested.clone(), None, false, false);
            if graph.charge(8)
                && let Some(node) = graph.resources.get_mut(&requested)
            {
                node.query_references.push(index);
            }
            for id in &ids {
                graph.relate(
                    requested.clone(),
                    id.clone(),
                    Some(index),
                    RelationshipKind::QueryReference,
                );
                if id != &requested
                    && std::mem::discriminant(id) == std::mem::discriminant(&requested)
                {
                    conflict(&mut graph, &mut contradictory, &requested);
                    conflict(&mut graph, &mut contradictory, id);
                }
            }
        }
        for a in &ids {
            for b in &ids {
                if a != b && std::mem::discriminant(a) == std::mem::discriminant(b) {
                    conflict(&mut graph, &mut contradictory, a);
                    conflict(&mut graph, &mut contradictory, b);
                }
            }
        }
        for id in &ids {
            graph.add(id.clone(), Some(index), positive, false);
        }
        for pair in ids.windows(2) {
            graph.relate(
                pair[0].clone(),
                pair[1].clone(),
                Some(index),
                RelationshipKind::RecordedReference,
            );
        }
        // Sibling projections may contradict one another. Connect their identities
        // without replacing either perspective, only within the exact query/source.
        if !ids.is_empty() && !graph.exhausted {
            if !graph.charge(
                64 + ids
                    .iter()
                    .map(AllocationIdentity::logical_bytes)
                    .sum::<usize>(),
            ) {
                continue;
            }
            let query_index = evidence
                .evidence
                .observations
                .coverage
                .iter()
                .position(|c| c.query == record.query)
                .expect("validated matching coverage");
            let key = (query_index, record.source);
            let siblings = source_ids.entry(key).or_default();
            for id in &ids {
                for sibling in siblings.iter() {
                    if sibling != id
                        && std::mem::discriminant(sibling) == std::mem::discriminant(id)
                    {
                        conflict(&mut graph, &mut contradictory, sibling);
                        conflict(&mut graph, &mut contradictory, id);
                    }
                    graph.relate(
                        sibling.clone(),
                        id.clone(),
                        Some(index),
                        RelationshipKind::SameSource,
                    );
                }
            }
            siblings.extend(ids);
        }
    }
    positional::derive(evidence, &mut graph, &mut contradictory);
    // Reachability is monotone, with at most N promotions. No query is issued here.
    loop {
        let mut changed = false;
        for edge in &graph.relationships {
            let positive = [&edge.from, &edge.to]
                .iter()
                .any(|id| graph.resources.get(*id).is_some_and(|r| r.positive_linkage));
            if positive {
                for id in [&edge.from, &edge.to] {
                    if let Some(r) = graph.resources.get_mut(id)
                        && !r.positive_linkage
                    {
                        r.positive_linkage = true;
                        r.attribution = Attribution::PlausiblyLinked;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Compare attachment endpoints without allocating another identity index.
    for edge in &graph.relationships {
        for (resource, instance) in [(&edge.from, &edge.to), (&edge.to, &edge.from)] {
            if matches!(resource, AllocationIdentity::Instance(_))
                || !matches!(instance, AllocationIdentity::Instance(_))
            {
                continue;
            }
            for other in &graph.relationships {
                let other_instance = if &other.from == resource {
                    &other.to
                } else if &other.to == resource {
                    &other.from
                } else {
                    continue;
                };
                if other_instance != instance
                    && matches!(other_instance, AllocationIdentity::Instance(_))
                {
                    for id in [resource, instance, other_instance] {
                        if let Some(r) = graph.resources.get_mut(id) {
                            r.attribution = Attribution::ContradictoryLinkage;
                        }
                    }
                }
            }
        }
    }
    for id in contradictory {
        if let Some(r) = graph.resources.get_mut(&id) {
            r.attribution = Attribution::ContradictoryLinkage;
        }
    }
    scheduling::derive_depths(inputs, evidence, &mut graph);
    for node in graph.resources.values_mut() {
        node.agreement = relationship_agreement::assess(inputs, evidence, node);
        if node.agreement.contradictory {
            node.attribution = Attribution::ContradictoryLinkage;
        }
    }
    graph
}

/// A closed foreign component is the only exclusion rule. Missing fields or an
/// incomplete discovery report never authorize exclusion. Evidence is retained.
pub(super) fn exclude_foreign(
    inputs: &DiscoveryInputs<'_>,
    evidence: &ValidatedDiscoveryEvidence,
    graph: &mut Relationships,
) -> BTreeSet<AllocationIdentity> {
    let mut excluded = BTreeSet::new();
    // Two borrowed-identity sets, reused across components. Charge their entire
    // possible occupancy before insertion; payload remains owned by the graph.
    if !graph.budget.charge(16 * graph.resources.len() + 32) {
        return excluded;
    }
    let mut visited = BTreeSet::new();
    for seed in graph.resources.keys() {
        if visited.contains(seed) {
            continue;
        }
        let mut component = BTreeSet::from([seed]);
        loop {
            let before = component.len();
            for e in &graph.relationships {
                if component.contains(&e.from) || component.contains(&e.to) {
                    component.insert(&e.from);
                    component.insert(&e.to);
                }
            }
            if component.len() == before {
                break;
            }
        }
        visited.extend(component.iter().copied());
        if component.iter().any(|id| {
            graph.resources.get(*id).is_none_or(|r| {
                r.prior
                    || r.attribution != Attribution::DiscoveryHit
                    || !r.agreement.permits_exclusion()
            })
        }) {
            continue;
        }
        let mut foreign = None;
        let mut eligible = true;
        for &id in &component {
            let resource = &graph.resources[id];
            let mut standalone = false;
            let mut token: Option<&str> = None;
            for index in &resource.observations {
                let ObservationEntryV5::V5(record) =
                    &evidence.evidence.observations.records[*index]
                else {
                    continue;
                };
                let tags=match (&record.data,id) {
                    (D::Instance(v),AllocationIdentity::Instance(expected)) if matches!(&v.id,M::Present(id) if id==expected) => {
                        standalone=true;
                        eligible &= matches!(&v.reservation_owner,M::Present(owner) if owner==&inputs.context.fields().binding.account_id);
                        if let M::Present(t)=&v.token {
                            if token.is_some_and(|old|old!=t.as_str()) {eligible=false;}
                            token=Some(t.as_str());
                        }
                        eligible &= matches!(&v.token,M::Present(t) if !t.as_str().is_empty() && t.as_str()!=inputs.context.fields().binding.client_token.as_str());
                        Some(&v.tags)
                    }
                    (D::NetworkInterface(v),AllocationIdentity::NetworkInterface(expected)) if matches!(&v.id,M::Present(id) if id==expected)
                        && matches!(record.source.path,crate::provider::source_occurrence_v5::SourcePathV5::NetworkInterface{..}) => {
                            standalone=true;
                            eligible &= matches!(&v.owner,M::Present(owner) if owner==&inputs.context.fields().binding.account_id);
                            Some(&v.tags)
                        }
                    (D::Volume(v),AllocationIdentity::Volume(expected)) if matches!(&v.id,M::Present(id) if id==expected) => {standalone=true;Some(&v.tags)}
                    (D::ProfileAssociation(_),AllocationIdentity::ProfileAssociation(_)) => {standalone=true;None}
                    _=>None,
                };
                if let Some(tags) = tags {
                    let pair = foreign_tags(tags, inputs);
                    if pair.is_none() || foreign.as_ref().is_some_and(|f| Some(f) != pair.as_ref())
                    {
                        eligible = false;
                    }
                    if foreign.is_none() {
                        foreign = pair;
                    }
                }
            }
            eligible &= standalone;
        }
        if eligible && foreign.is_some() {
            for id in component {
                if !graph.budget.charge(id.logical_bytes() + 16) {
                    return excluded;
                }
                excluded.insert(id.clone());
            }
        }
    }
    excluded
}
fn foreign_tags(
    tags: &V<EvidenceList<TagV5>>,
    inputs: &DiscoveryInputs<'_>,
) -> Option<(AuthorityId, OperationId)> {
    let V::Present(tags) = tags else { return None };
    let (mut authority, mut operation) = (None, None);
    for tag in tags.as_slice() {
        let (M::Present(key), M::Present(value)) = (&tag.key, &tag.value) else {
            return None;
        };
        match key.as_str() {
            AUTHORITY_TAG => {
                if authority.is_some() {
                    return None;
                }
                authority = Some(value.as_str().parse().ok()?);
            }
            OPERATION_TAG => {
                if operation.is_some() {
                    return None;
                }
                operation = Some(value.as_str().parse().ok()?);
            }
            _ => (),
        }
    }
    let pair = (authority?, operation?);
    (pair.1 != inputs.context.fields().binding.operation_id).then_some(pair)
}

//! Source-aware prerequisites for exclusion. No launch-property admission checks.
use super::{relationships::*, *};
use crate::provider::{
    allocation_value_v5::MemberV5 as M,
    ec2_allocation_observation_v5::{ObservationDataV5 as D, OperatorV5, ProfileV5},
    management_observation_v2::ObservationValueV2 as V,
    observation_v5::{ObservationEntryV5, ObservationRecordV5},
    source_occurrence_v5::SourcePathV5 as P,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipFactKind {
    ReservationOwner,
    Instance,
    NetworkInterface,
    InstanceEniAttachment,
    StandaloneEniAttachment,
    InstanceEbsMapping,
    Volume,
    VolumeAttachment,
    ProfileAssociation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationshipFactRef {
    pub observation: usize,
    pub kind: RelationshipFactKind,
}
pub(super) fn kind(data: &D) -> Option<RelationshipFactKind> {
    use RelationshipFactKind as K;
    Some(match data {
        D::Reservation(_) => K::ReservationOwner,
        D::Instance(_) => K::Instance,
        D::NetworkInterface(_) => K::NetworkInterface,
        D::InstanceEniAttachment(_) => K::InstanceEniAttachment,
        D::StandaloneEniAttachment(_) => K::StandaloneEniAttachment,
        D::InstanceEbsMapping(_) => K::InstanceEbsMapping,
        D::Volume(_) => K::Volume,
        D::VolumeAttachment(_) => K::VolumeAttachment,
        D::ProfileAssociation(_) => K::ProfileAssociation,
        _ => return None,
    })
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationshipAgreement {
    pub missing: bool,
    pub contradictory: bool,
    pub opaque: bool,
}
impl RelationshipAgreement {
    pub fn permits_exclusion(self) -> bool {
        !self.missing && !self.contradictory && !self.opaque
    }
    fn same<T: PartialEq>(&mut self, a: &M<T>, b: &M<T>) {
        match (a, b) {
            (M::Present(a), M::Present(b)) => self.contradictory |= a != b,
            _ => self.missing = true,
        }
    }
    fn owner(
        &mut self,
        value: &M<crate::identity::AwsAccountId>,
        expected: &crate::identity::AwsAccountId,
    ) {
        match value {
            M::Present(v) => self.contradictory |= v != expected,
            _ => self.missing = true,
        }
    }
    fn profile(&mut self, a: &V<ProfileV5>, b: &V<ProfileV5>) {
        if let (V::Present(a), V::Present(b)) = (a, b) {
            self.same(&a.arn, &b.arn);
            self.same(&a.id, &b.id);
        } else {
            self.missing = true;
        }
    }
}
fn same_present<T: PartialEq>(a: &M<T>, b: &M<T>) -> bool {
    matches!((a,b),(M::Present(a),M::Present(b)) if a==b)
}
fn records(e: &ValidatedDiscoveryEvidence) -> impl Iterator<Item = &ObservationRecordV5> {
    e.evidence.observations.records.iter().filter_map(|r| {
        if let ObservationEntryV5::V5(v) = r {
            Some(v.as_ref())
        } else {
            None
        }
    })
}
fn reference<T>(v: &M<T>) -> bool {
    !matches!(v, M::NotReturned | M::NotExposedBySource | M::Empty)
}
fn operator(v: &V<OperatorV5>) -> bool {
    matches!(v,V::Present(v) if reference(&v.principal)||matches!(v.managed,V::Present(true)))
}
pub(super) fn opaque(data: &D) -> bool {
    match data {
        D::Instance(v) => operator(&v.operator),
        D::NetworkInterface(v) => {
            operator(&v.operator)
                || reference(&v.requester.identity)
                || matches!(v.requester.managed, V::Present(true))
        }
        D::Volume(v) => operator(&v.operator),
        D::InstanceEbsMapping(v) => {
            matches!(&v.ebs,V::Present(v) if reference(&v.associated_resource)||operator(&v.operator))
        }
        D::VolumeAttachment(v) => {
            reference(&v.associated_resource) || reference(&v.instance_owning_service)
        }
        _ => false,
    }
}

pub(super) fn assess(
    inputs: &DiscoveryInputs<'_>,
    evidence: &ValidatedDiscoveryEvidence,
    node: &ResourceAttribution,
) -> RelationshipAgreement {
    let mut out = RelationshipAgreement::default();
    let account = &inputs.context.fields().binding.account_id;
    for &index in &node.observations {
        let ObservationEntryV5::V5(record) = &evidence.evidence.observations.records[index] else {
            continue;
        };
        out.opaque |= opaque(&record.data);
        match &record.data {
            D::Instance(v) => {
                out.owner(&v.reservation_owner, account);
                let mut parent = false;
                if let P::Instance { reservation, .. } = record.source.path {
                    for r in records(evidence).filter(|r| {
                        r.query == record.query
                            && r.source.page == record.source.page
                            && r.source.path == P::Reservation { reservation }
                    }) {
                        if let D::Reservation(p) = &r.data {
                            parent = true;
                            out.owner(&p.owner, account);
                            out.same(&p.owner, &v.reservation_owner);
                        }
                    }
                }
                out.missing |= !parent;
                // A returned profile reference needs the independently queried association.
                if matches!(v.profile, V::Present(_)) {
                    let mut found = false;
                    for r in records(evidence) {
                        if let D::ProfileAssociation(p) = &r.data
                            && same_present(&v.id, &p.instance)
                        {
                            found = true;
                            out.profile(&v.profile, &p.profile);
                        }
                    }
                    out.missing |= !found;
                }
            }
            D::NetworkInterface(v) => out.owner(&v.owner, account),
            D::InstanceEniAttachment(v) => {
                let mut found = false;
                if let V::Present(a) = &v.attachment {
                    if !matches!(a.id, M::Present(_)) {
                        out.missing = true;
                    }
                    for r in records(evidence) {
                        if let D::StandaloneEniAttachment(p) = &r.data
                            && same_present(&v.enclosing_interface, &p.enclosing_interface)
                        {
                            if let V::Present(b) = &p.attachment {
                                found = true;
                                out.same(&a.id, &b.fields.id);
                                out.same(&v.enclosing_instance, &b.instance);
                                out.owner(&b.instance_owner, account);
                            } else {
                                out.missing = true;
                            }
                        }
                    }
                } else {
                    out.missing = true;
                }
                out.missing |= !found;
            }
            D::StandaloneEniAttachment(v) => {
                let mut found = false;
                if let V::Present(a) = &v.attachment {
                    out.owner(&a.instance_owner, account);
                    if !matches!(a.fields.id, M::Present(_)) {
                        out.missing = true;
                    }
                    for r in records(evidence) {
                        if let D::InstanceEniAttachment(p) = &r.data
                            && same_present(&v.enclosing_interface, &p.enclosing_interface)
                        {
                            if let V::Present(b) = &p.attachment {
                                found = true;
                                out.same(&a.fields.id, &b.id);
                                out.same(&a.instance, &p.enclosing_instance);
                            } else {
                                out.missing = true;
                            }
                        }
                    }
                } else {
                    out.missing = true;
                }
                out.missing |= !found;
            }
            D::InstanceEbsMapping(v) => {
                let mut found = false;
                if let V::Present(a) = &v.ebs {
                    out.owner(&a.volume_owner, account);
                    for r in records(evidence) {
                        if let D::VolumeAttachment(p) = &r.data
                            && same_present(&a.volume, &p.enclosing_volume)
                        {
                            found = true;
                            out.same(&a.volume, &p.volume);
                            out.same(&v.enclosing_instance, &p.instance);
                            out.same(&v.device, &p.device);
                        }
                    }
                } else {
                    out.missing = true;
                }
                out.missing |= !found;
            }
            D::VolumeAttachment(v) => {
                out.same(&v.enclosing_volume, &v.volume);
                let mut found = false;
                for r in records(evidence) {
                    if let D::InstanceEbsMapping(p) = &r.data
                        && let V::Present(a) = &p.ebs
                        && same_present(&v.enclosing_volume, &a.volume)
                    {
                        found = true;
                        out.same(&v.instance, &p.enclosing_instance);
                        out.same(&v.device, &p.device);
                        out.owner(&a.volume_owner, account);
                    }
                }
                out.missing |= !found;
            }
            D::ProfileAssociation(v) => {
                let mut found = false;
                for r in records(evidence) {
                    if let D::Instance(p) = &r.data
                        && same_present(&v.instance, &p.id)
                    {
                        found = true;
                        out.profile(&v.profile, &p.profile);
                    }
                }
                out.missing |= !found;
            }
            _ => (),
        }
    }
    out
}

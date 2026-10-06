//! Structural containment from exact typed provenance, not provider attachment proof.
use super::*;
use crate::provider::{
    observation_v5::ObservationRecordV5, source_occurrence_v5::SourcePathV5 as P,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentIdentityAgreement {
    Consistent,
    Contradictory,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionalParentFact {
    pub parent: Option<usize>,
    pub child: usize,
    pub agreement: ParentIdentityAgreement,
}

fn present<T: Clone>(
    member: &M<T>,
    wrap: impl Fn(T) -> AllocationIdentity,
) -> Option<AllocationIdentity> {
    if let M::Present(id) = member {
        Some(wrap(id.clone()))
    } else {
        None
    }
}

/// Fixed-size borrowed-input extraction; never an index or an unbounded child list.
/// The first reference is the child's recorded enclosing identity. Remaining
/// references are independently recorded child/attachment endpoints.
fn references(record: &ObservationRecordV5) -> Option<[Option<AllocationIdentity>; 3]> {
    use AllocationIdentity as I;
    Some(match (&record.source.path, &record.data) {
        (P::InstanceNetworkInterface { .. }, D::NetworkInterface(v)) => [
            present(&v.enclosing_instance, I::Instance),
            present(&v.id, I::NetworkInterface),
            None,
        ],
        (P::InstanceNetworkInterface { .. }, D::InstanceEniAttachment(v)) => [
            present(&v.enclosing_instance, I::Instance),
            present(&v.enclosing_interface, I::NetworkInterface),
            None,
        ],
        (P::InstanceEbsMapping { .. }, D::InstanceEbsMapping(v)) => [
            present(&v.enclosing_instance, I::Instance),
            if let V::Present(ebs) = &v.ebs {
                present(&ebs.volume, I::Volume)
            } else {
                None
            },
            None,
        ],
        (P::VolumeAttachment { .. }, D::VolumeAttachment(v)) => [
            present(&v.enclosing_volume, I::Volume),
            present(&v.volume, I::Volume),
            present(&v.instance, I::Instance),
        ],
        (P::InstanceSecondaryInterface { .. }, D::UnsupportedSecondaryInterface(v)) => {
            [present(&v.enclosing_instance, I::Instance), None, None]
        }
        // Reservations carry ownership, not an AllocationIdentity. Their actual
        // presence/owner is audited separately; do not join sibling instances.
        // Root paths have no parent. Inline lists introduce no allocation nodes.
        _ => return None,
    })
}

pub(super) fn derive(
    evidence: &ValidatedDiscoveryEvidence,
    graph: &mut Relationships,
    contradictory: &mut BTreeSet<AllocationIdentity>,
) {
    for (child, entry) in evidence.evidence.observations.records.iter().enumerate() {
        if graph.exhausted {
            break;
        }
        let ObservationEntryV5::V5(record) = entry else {
            continue;
        };
        let Some(references) = references(record) else {
            continue;
        };
        let Some((Some(parent_path), _, _)) = record.source.path.containing_list() else {
            continue;
        };
        // Bounded linear lookup avoids another source index. Match the actual
        // parent projection, never a sibling projection or a copied enclosing ID.
        let parent = evidence
            .evidence
            .observations
            .records
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                let ObservationEntryV5::V5(p) = entry else {
                    return None;
                };
                if p.query != record.query
                    || p.source.page != record.source.page
                    || p.source.path != parent_path
                {
                    return None;
                }
                match &p.data {
                    D::Instance(v) => Some((index, present(&v.id, AllocationIdentity::Instance))),
                    D::Volume(v) => Some((index, present(&v.id, AllocationIdentity::Volume))),
                    _ => None,
                }
            });
        let actual = parent.as_ref().and_then(|(_, id)| id.as_ref());
        let agreement = match (actual, &references[0]) {
            (Some(a), Some(b)) if a != b => ParentIdentityAgreement::Contradictory,
            (Some(_), Some(_)) => ParentIdentityAgreement::Consistent,
            _ => ParentIdentityAgreement::Unavailable,
        };
        if !graph.charge(32) {
            break;
        }
        graph.positional_parents.push(PositionalParentFact {
            parent: parent.as_ref().map(|p| p.0),
            child,
            agreement,
        });
        if parent.is_none() {
            graph.gap(
                Some(child),
                RelationshipGapKind::ParentProjectionUnavailable,
            );
        } else if actual.is_none() {
            graph.gap(Some(child), RelationshipGapKind::ParentIdentityUnavailable);
        }
        if references[0].is_none() {
            graph.gap(
                Some(child),
                RelationshipGapKind::EnclosingIdentityUnavailable,
            );
        }
        if let Some((parent, Some(actual))) = parent {
            for id in references.iter().flatten() {
                // Includes an independently returned VolumeId as well as the
                // enclosing volume. All valid perspectives remain connected.
                if id != &actual && std::mem::discriminant(id) == std::mem::discriminant(&actual) {
                    conflict(graph, contradictory, &actual);
                    conflict(graph, contradictory, id);
                }
                graph.relate(
                    actual.clone(),
                    id.clone(),
                    Some(child),
                    RelationshipKind::PositionalParent { parent },
                );
            }
        }
    }
}

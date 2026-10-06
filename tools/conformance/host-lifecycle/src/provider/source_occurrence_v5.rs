//! Positional provenance and minimum source credit. Never resource matching or discovery.
use super::{coverage::ReadOperationV1, limits::RECORDS};
use crate::{Error, Result, require};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

macro_rules! bounded_integer {
    ($name:ident, $min:literal, $max:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "u64", into = "u64")]
        pub struct $name(u64);
        impl TryFrom<u64> for $name {
            type Error = Error;
            fn try_from(value: u64) -> Result<Self> {
                require(
                    ($min..=$max).contains(&value),
                    "source position/count bound",
                )?;
                Ok(Self(value))
            }
        }
        impl From<$name> for u64 {
            fn from(value: $name) -> u64 {
                value.0
            }
        }
    };
}
bounded_integer!(SourcePageV5, 1, 16);
bounded_integer!(RootIndexV5, 0, 4095);
bounded_integer!(NestedIndexV5, 0, 127);
bounded_integer!(SourceCountV5, 0, 4096);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceOccurrenceV5 {
    pub page: SourcePageV5,
    pub path: SourcePathV5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SourcePathV5 {
    Image {
        image: RootIndexV5,
    },
    InstanceType {
        instance_type: RootIndexV5,
    },
    TypeOffering {
        offering: RootIndexV5,
    },
    ProfileAssociation {
        association: RootIndexV5,
    },
    Reservation {
        reservation: RootIndexV5,
    },
    Instance {
        reservation: RootIndexV5,
        instance: NestedIndexV5,
    },
    InstanceNetworkInterface {
        reservation: RootIndexV5,
        instance: NestedIndexV5,
        interface: NestedIndexV5,
    },
    InstanceEbsMapping {
        reservation: RootIndexV5,
        instance: NestedIndexV5,
        mapping: NestedIndexV5,
    },
    InstanceSecondaryInterface {
        reservation: RootIndexV5,
        instance: NestedIndexV5,
        interface: NestedIndexV5,
    },
    NetworkInterface {
        interface: RootIndexV5,
    },
    Volume {
        volume: RootIndexV5,
    },
    VolumeAttachment {
        volume: RootIndexV5,
        attachment: NestedIndexV5,
    },
    InstanceAttribute,
}

/// Derived from the data variant, never independently supplied serialized evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProjectionKindV5 {
    Image,
    InstanceType,
    TypeOffering,
    ProfileAssociation,
    Reservation,
    Instance,
    InstanceOptions,
    ExcludedFeatures,
    NetworkInterface,
    InstanceEniAttachment,
    StandaloneEniAttachment,
    InstanceEbsMapping,
    UnsupportedSecondaryInterface,
    Volume,
    VolumeAttachment,
    InstanceAttributes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CollectionShapeV5 {
    NotReturned,
    NotExposedBySource,
    Present { count: SourceCountV5 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SourceListV5 {
    Images,
    InstanceTypes,
    TypeOfferings,
    ProfileAssociations,
    Reservations,
    Instances,
    InstanceNetworkInterfaces,
    InstanceEbsMappings,
    InstanceSecondaryInterfaces,
    NetworkInterfaces,
    Volumes,
    VolumeAttachments,
    ImageMappings,
    ImageProductCodes,
    TypeArchitectures,
    TypeVirtualization,
    TypeRootDevices,
    TypeUsageClasses,
    GpuDevices,
    FpgaDevices,
    InferenceDevices,
    MediaDevices,
    NeuronDevices,
    Tags,
    SecurityGroups,
    PrivateIpv4,
    Ipv6,
    Ipv4Prefixes,
    Ipv6Prefixes,
    Licenses,
    ElasticGpuAssociations,
    ElasticInferenceAssociations,
}
impl SourcePathV5 {
    pub(crate) fn operation(self) -> ReadOperationV1 {
        use ReadOperationV1 as O;
        match self {
            Self::Image { .. } => O::DescribeImages,
            Self::InstanceType { .. } => O::DescribeInstanceTypes,
            Self::TypeOffering { .. } => O::DescribeInstanceTypeOfferings,
            Self::ProfileAssociation { .. } => O::DescribeIamInstanceProfileAssociations,
            Self::Reservation { .. }
            | Self::Instance { .. }
            | Self::InstanceNetworkInterface { .. }
            | Self::InstanceEbsMapping { .. }
            | Self::InstanceSecondaryInterface { .. } => O::DescribeInstances,
            Self::NetworkInterface { .. } => O::DescribeNetworkInterfaces,
            Self::Volume { .. } | Self::VolumeAttachment { .. } => O::DescribeVolumes,
            Self::InstanceAttribute => O::DescribeInstanceAttribute,
        }
    }
    pub(crate) fn permits(self, projection: ProjectionKindV5) -> bool {
        use ProjectionKindV5 as P;
        matches!(
            (self, projection),
            (Self::Image { .. }, P::Image | P::ExcludedFeatures)
                | (Self::InstanceType { .. }, P::InstanceType)
                | (Self::TypeOffering { .. }, P::TypeOffering)
                | (Self::ProfileAssociation { .. }, P::ProfileAssociation)
                | (Self::Reservation { .. }, P::Reservation)
                | (
                    Self::Instance { .. },
                    P::Instance | P::InstanceOptions | P::ExcludedFeatures
                )
                | (
                    Self::InstanceNetworkInterface { .. },
                    P::NetworkInterface | P::InstanceEniAttachment
                )
                | (Self::InstanceEbsMapping { .. }, P::InstanceEbsMapping)
                | (
                    Self::InstanceSecondaryInterface { .. },
                    P::UnsupportedSecondaryInterface
                )
                | (
                    Self::NetworkInterface { .. },
                    P::NetworkInterface | P::StandaloneEniAttachment | P::ExcludedFeatures
                )
                | (Self::Volume { .. }, P::Volume | P::ExcludedFeatures)
                | (Self::VolumeAttachment { .. }, P::VolumeAttachment)
                | (Self::InstanceAttribute, P::InstanceAttributes)
        )
    }
    pub(crate) fn containing_list(self) -> Option<(Option<Self>, SourceListV5, u64)> {
        use SourceListV5 as L;
        Some(match self {
            Self::Image { image } => (None, L::Images, image.into()),
            Self::InstanceType { instance_type } => (None, L::InstanceTypes, instance_type.into()),
            Self::TypeOffering { offering } => (None, L::TypeOfferings, offering.into()),
            Self::ProfileAssociation { association } => {
                (None, L::ProfileAssociations, association.into())
            }
            Self::Reservation { reservation } => (None, L::Reservations, reservation.into()),
            Self::Instance {
                reservation,
                instance,
            } => (
                Some(Self::Reservation { reservation }),
                L::Instances,
                instance.into(),
            ),
            Self::InstanceNetworkInterface {
                reservation,
                instance,
                interface,
            } => (
                Some(Self::Instance {
                    reservation,
                    instance,
                }),
                L::InstanceNetworkInterfaces,
                interface.into(),
            ),
            Self::InstanceEbsMapping {
                reservation,
                instance,
                mapping,
            } => (
                Some(Self::Instance {
                    reservation,
                    instance,
                }),
                L::InstanceEbsMappings,
                mapping.into(),
            ),
            Self::InstanceSecondaryInterface {
                reservation,
                instance,
                interface,
            } => (
                Some(Self::Instance {
                    reservation,
                    instance,
                }),
                L::InstanceSecondaryInterfaces,
                interface.into(),
            ),
            Self::NetworkInterface { interface } => (None, L::NetworkInterfaces, interface.into()),
            Self::Volume { volume } => (None, L::Volumes, volume.into()),
            Self::VolumeAttachment { volume, attachment } => (
                Some(Self::Volume { volume }),
                L::VolumeAttachments,
                attachment.into(),
            ),
            Self::InstanceAttribute => return None,
        })
    }
    pub(crate) fn permits_list(self, list: SourceListV5) -> bool {
        use SourceListV5 as L;
        matches!(
            (self, list),
            (Self::Image { .. }, L::ImageMappings | L::ImageProductCodes)
                | (
                    Self::InstanceType { .. },
                    L::TypeArchitectures
                        | L::TypeVirtualization
                        | L::TypeRootDevices
                        | L::TypeUsageClasses
                        | L::GpuDevices
                        | L::FpgaDevices
                        | L::InferenceDevices
                        | L::MediaDevices
                        | L::NeuronDevices
                )
                | (Self::Reservation { .. }, L::Instances)
                | (
                    Self::Instance { .. },
                    L::InstanceNetworkInterfaces
                        | L::InstanceEbsMappings
                        | L::InstanceSecondaryInterfaces
                        | L::Tags
                        | L::Licenses
                        | L::ElasticGpuAssociations
                        | L::ElasticInferenceAssociations
                )
                | (
                    Self::InstanceNetworkInterface { .. } | Self::NetworkInterface { .. },
                    L::SecurityGroups
                        | L::PrivateIpv4
                        | L::Ipv6
                        | L::Ipv4Prefixes
                        | L::Ipv6Prefixes
                )
                | (Self::NetworkInterface { .. } | Self::Volume { .. }, L::Tags)
                | (Self::Volume { .. }, L::VolumeAttachments)
        )
    }
}

#[derive(Clone, Copy, Default)]
struct Demand {
    minimum: u64,
    shape: Option<CollectionShapeV5>,
}
type CollectionKey = (SourcePageV5, Option<SourcePathV5>, SourceListV5);

/// One exact query only. The owner supplies matching coverage; no cross-query lookup here.
#[derive(Default)]
pub(crate) struct SourceCreditV5 {
    demands: BTreeMap<CollectionKey, Demand>,
    projections: BTreeSet<(SourceOccurrenceV5, ProjectionKindV5)>,
    singletons: BTreeSet<SourcePageV5>,
}
impl SourceCreditV5 {
    pub(crate) fn record(
        &mut self,
        source: SourceOccurrenceV5,
        kind: ProjectionKindV5,
        claims: impl IntoIterator<Item = (SourceListV5, CollectionShapeV5)>,
    ) -> Result<()> {
        require(source.path.permits(kind), "source/projection mismatch")?;
        require(
            self.projections.len() < RECORDS as usize,
            "projection count bound",
        )?;
        require(
            self.projections.insert((source, kind)),
            "duplicate source projection",
        )?;
        if source.path == SourcePathV5::InstanceAttribute {
            require(u64::from(source.page) == 1, "singleton source page")?;
            self.singletons.insert(source.page);
        }
        let mut path = Some(source.path);
        while let Some(current) = path {
            if let Some((parent, list, index)) = current.containing_list() {
                self.demand((source.page, parent, list), index + 1, None)?;
                path = parent;
            } else {
                path = None;
            }
        }
        for (list, shape) in claims {
            require(source.path.permits_list(list), "source/list mismatch")?;
            let count = match shape {
                CollectionShapeV5::Present { count } => count.into(),
                _ => 0,
            };
            self.demand((source.page, Some(source.path), list), count, Some(shape))?;
        }
        self.minimum().map(|_| ())
    }
    fn demand(
        &mut self,
        key: CollectionKey,
        minimum: u64,
        shape: Option<CollectionShapeV5>,
    ) -> Result<()> {
        let demand = self.demands.entry(key).or_default();
        if let Some(shape) = shape {
            require(
                demand.shape.is_none_or(|old| old == shape),
                "conflicting collection shape",
            )?;
            demand.shape = Some(shape);
        }
        demand.minimum = demand.minimum.max(minimum);
        if let Some(shape) = demand.shape {
            let count = match shape {
                CollectionShapeV5::Present { count } => count.into(),
                _ => 0,
            };
            require(demand.minimum <= count, "child outside parent collection")?;
        }
        Ok(())
    }
    pub(crate) fn minimum(&self) -> Result<u64> {
        self.demands
            .values()
            .try_fold(self.singletons.len() as u64, |total, d| {
                total
                    .checked_add(d.minimum)
                    .filter(|v| *v <= RECORDS)
                    .ok_or(Error("source occurrence bound"))
            })
    }
}

#[cfg(test)]
#[path = "source_occurrence_v5_tests.rs"]
mod tests;

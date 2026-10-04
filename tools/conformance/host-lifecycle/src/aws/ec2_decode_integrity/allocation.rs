//! Invocation-bound qualification receipts. No independently constructible page provenance.
#[cfg(test)]
#[path = "allocation_tests.rs"]
mod tests;
use super::*;
use crate::aws::query_execution::AllocationAttempt;
use crate::provider::{ec2_allocation_observation_v5::ObservationDataV5, source_occurrence_v5::*};
use std::collections::BTreeSet;
pub(in crate::aws) enum AllocationOutput {
    Images(Box<aws_sdk_ec2::operation::describe_images::DescribeImagesOutput>),
InstanceTypes(Box<aws_sdk_ec2::operation::describe_instance_types::DescribeInstanceTypesOutput>),
InstanceTypeOfferings(Box<aws_sdk_ec2::operation::describe_instance_type_offerings::DescribeInstanceTypeOfferingsOutput>),
IamInstanceProfileAssociations(Box<aws_sdk_ec2::operation::describe_iam_instance_profile_associations::DescribeIamInstanceProfileAssociationsOutput>),
Instances(Box<aws_sdk_ec2::operation::describe_instances::DescribeInstancesOutput>),
NetworkInterfaces(Box<aws_sdk_ec2::operation::describe_network_interfaces::DescribeNetworkInterfacesOutput>),
Volumes(Box<aws_sdk_ec2::operation::describe_volumes::DescribeVolumesOutput>),
InstanceAttribute(Box<aws_sdk_ec2::operation::describe_instance_attribute::DescribeInstanceAttributeOutput>),
}
impl AllocationOutput {
    pub(super) fn presence(&self) -> Presence {
        match self {
            Self::Images(v) => v.presence(),
            Self::InstanceTypes(v) => v.presence(),
            Self::InstanceTypeOfferings(v) => v.presence(),
            Self::IamInstanceProfileAssociations(v) => v.presence(),
            Self::Instances(v) => v.presence(),
            Self::NetworkInterfaces(v) => v.presence(),
            Self::Volumes(v) => v.presence(),
            Self::InstanceAttribute(v) => v.presence(),
        }
    }
    fn continuation(&self) -> Option<&str> {
        match self {
            Self::Images(v) => v.next_token.as_deref(),
            Self::InstanceTypes(v) => v.next_token.as_deref(),
            Self::InstanceTypeOfferings(v) => v.next_token.as_deref(),
            Self::IamInstanceProfileAssociations(v) => v.next_token.as_deref(),
            Self::Instances(v) => v.next_token.as_deref(),
            Self::NetworkInterfaces(v) => v.next_token.as_deref(),
            Self::Volumes(v) => v.next_token.as_deref(),
            Self::InstanceAttribute(_) => None,
        }
    }
}

pub(in crate::aws) struct AllocationReceiver {
    receiver: Ec2Receiver,
    attempt: AllocationAttempt,
}
pub(in crate::aws) struct QualifiedAllocationReceipt {
    attempt: AllocationAttempt,
    layout: SourceLayout,
    occurrences: u64,
    continuation: Option<String>,
}
pub(in crate::aws) fn capture_allocation(
    attempt: AllocationAttempt,
    round: ObservationRound,
) -> (Ec2Integrity, AllocationReceiver) {
    let (guard, receiver) = capture_ec2(attempt.operation(), round);
    (guard, AllocationReceiver { receiver, attempt })
}
impl AllocationReceiver {
    pub(in crate::aws) fn take(self) -> Result<(QualifiedAllocationReceipt, AllocationOutput)> {
        let round = self.receiver.round.clone();
        let (output, count) = self.receiver.take()?;
        let Ec2Output::Allocation(output) = output else {
            return Err(Error("allocation invocation output"));
        };
        let layout = SourceLayout::build(&output, &round)?;
        let occurrences =
            count + u64::from(matches!(output, AllocationOutput::InstanceAttribute(_)));
        let continuation = output.continuation().map(str::to_owned);
        Ok((
            QualifiedAllocationReceipt {
                attempt: self.attempt,
                layout,
                occurrences,
                continuation,
            },
            output,
        ))
    }
}
impl QualifiedAllocationReceipt {
    pub(in crate::aws) fn matches(&self, stamp: &Arc<()>) -> bool {
        self.attempt.matches(stamp)
    }
    pub(in crate::aws) fn occurrences(&self) -> u64 {
        self.occurrences
    }
    pub(in crate::aws) fn continuation(&self) -> Option<&str> {
        self.continuation.as_deref()
    }
    pub(in crate::aws) fn validate_source(
        &self,
        path: SourcePathV5,
        data: &ObservationDataV5,
    ) -> Result<()> {
        require(
            self.layout.paths.contains(&path),
            "source outside qualified SDK page",
        )?;
        for (list, shape) in data.claims() {
            require(
                self.layout.collections.get(&(path, list)) == Some(&shape),
                "source collection correlation",
            )?;
        }
        Ok(())
    }
}
#[derive(Default)]
struct SourceLayout {
    paths: BTreeSet<SourcePathV5>,
    collections: BTreeMap<(SourcePathV5, SourceListV5), CollectionShapeV5>,
}
impl SourceLayout {
    fn list<T>(
        &mut self,
        path: SourcePathV5,
        list: SourceListV5,
        values: Option<&Vec<T>>,
    ) -> Result<()> {
        require(path.permits_list(list), "qualified list family")?;
        let shape = match values {
            Some(v) => CollectionShapeV5::Present {
                count: (v.len() as u64).try_into()?,
            },
            None => CollectionShapeV5::NotReturned,
        };
        require(
            self.collections.insert((path, list), shape).is_none(),
            "duplicate qualified collection",
        )?;
        Ok(())
    }
    fn path(&mut self, path: SourcePathV5, round: &ObservationRound) -> Result<()> {
        round.remaining()?;
        require(
            self.paths.len() < RECORDS as usize && self.paths.insert(path),
            "qualified source bound/duplicate",
        )
    }
    fn eni_instance(
        &mut self,
        p: SourcePathV5,
        v: &aws_sdk_ec2::types::InstanceNetworkInterface,
    ) -> Result<()> {
        use SourceListV5 as L;
        self.list(p, L::SecurityGroups, v.groups.as_ref())?;
        self.list(p, L::PrivateIpv4, v.private_ip_addresses.as_ref())?;
        self.list(p, L::Ipv6, v.ipv6_addresses.as_ref())?;
        self.list(p, L::Ipv4Prefixes, v.ipv4_prefixes.as_ref())?;
        self.list(p, L::Ipv6Prefixes, v.ipv6_prefixes.as_ref())
    }
    fn build(output: &AllocationOutput, round: &ObservationRound) -> Result<Self> {
        use SourceListV5 as L;
        use SourcePathV5 as P;
        let mut s = Self::default();
        macro_rules! roots {
            ($items:expr,$index:ident,$item:ident,$path:expr,$body:block) => {
                if let Some(items) = $items {
                    for ($index, $item) in items.iter().enumerate() {
                        let $index = ($index as u64).try_into()?;
                        let p = $path;
                        s.path(p, round)?;
                        $body
                    }
                }
            };
        }
        match output {
            AllocationOutput::Images(v) => {
                roots!(&v.images, i, x, P::Image { image: i }, {
                    let p = P::Image { image: i };
                    s.list(p, L::ImageMappings, x.block_device_mappings.as_ref())?;
                    s.list(p, L::ImageProductCodes, x.product_codes.as_ref())?;
                });
            }
            AllocationOutput::InstanceTypes(v) => {
                roots!(
                    &v.instance_types,
                    i,
                    x,
                    P::InstanceType { instance_type: i },
                    {
                        let p = P::InstanceType { instance_type: i };
                        if let Some(v) = &x.processor_info {
                            s.list(p, L::TypeArchitectures, v.supported_architectures.as_ref())?;
                        }
                        s.list(
                            p,
                            L::TypeVirtualization,
                            x.supported_virtualization_types.as_ref(),
                        )?;
                        s.list(
                            p,
                            L::TypeRootDevices,
                            x.supported_root_device_types.as_ref(),
                        )?;
                        s.list(p, L::TypeUsageClasses, x.supported_usage_classes.as_ref())?;
                        if let Some(v) = &x.gpu_info {
                            s.list(p, L::GpuDevices, v.gpus.as_ref())?;
                        }
                        if let Some(v) = &x.fpga_info {
                            s.list(p, L::FpgaDevices, v.fpgas.as_ref())?;
                        }
                        if let Some(v) = &x.inference_accelerator_info {
                            s.list(p, L::InferenceDevices, v.accelerators.as_ref())?;
                        }
                        if let Some(v) = &x.media_accelerator_info {
                            s.list(p, L::MediaDevices, v.accelerators.as_ref())?;
                        }
                        if let Some(v) = &x.neuron_info {
                            s.list(p, L::NeuronDevices, v.neuron_devices.as_ref())?;
                        }
                    }
                );
            }
            AllocationOutput::InstanceTypeOfferings(v) => {
                roots!(
                    &v.instance_type_offerings,
                    i,
                    _x,
                    P::TypeOffering { offering: i },
                    {}
                );
            }
            AllocationOutput::IamInstanceProfileAssociations(v) => {
                roots!(
                    &v.iam_instance_profile_associations,
                    i,
                    _x,
                    P::ProfileAssociation { association: i },
                    {}
                );
            }
            AllocationOutput::Instances(v) => {
                roots!(&v.reservations, r, x, P::Reservation { reservation: r }, {
                    s.list(
                        P::Reservation { reservation: r },
                        L::Instances,
                        x.instances.as_ref(),
                    )?;
                    if let Some(items) = &x.instances
                        && items.len() <= 128
                    {
                        for (i, v) in items.iter().enumerate() {
                            let i = (i as u64).try_into()?;
                            let p = P::Instance {
                                reservation: r,
                                instance: i,
                            };
                            s.path(p, round)?;
                            s.list(p, L::Tags, v.tags.as_ref())?;
                            s.list(p, L::Licenses, v.licenses.as_ref())?;
                            s.list(
                                p,
                                L::ElasticGpuAssociations,
                                v.elastic_gpu_associations.as_ref(),
                            )?;
                            s.list(
                                p,
                                L::ElasticInferenceAssociations,
                                v.elastic_inference_accelerator_associations.as_ref(),
                            )?;
                            s.list(
                                p,
                                L::InstanceNetworkInterfaces,
                                v.network_interfaces.as_ref(),
                            )?;
                            s.list(p, L::InstanceEbsMappings, v.block_device_mappings.as_ref())?;
                            s.list(
                                p,
                                L::InstanceSecondaryInterfaces,
                                v.secondary_interfaces.as_ref(),
                            )?;
                            if let Some(items) = &v.network_interfaces
                                && items.len() <= 128
                            {
                                for (j, eni) in items.iter().enumerate() {
                                    let p = P::InstanceNetworkInterface {
                                        reservation: r,
                                        instance: i,
                                        interface: (j as u64).try_into()?,
                                    };
                                    s.path(p, round)?;
                                    s.eni_instance(p, eni)?;
                                }
                            }
                            if let Some(items) = &v.block_device_mappings
                                && items.len() <= 128
                            {
                                for j in 0..items.len() {
                                    s.path(
                                        P::InstanceEbsMapping {
                                            reservation: r,
                                            instance: i,
                                            mapping: (j as u64).try_into()?,
                                        },
                                        round,
                                    )?;
                                }
                            }
                            if let Some(items) = &v.secondary_interfaces
                                && items.len() <= 128
                            {
                                for j in 0..items.len() {
                                    s.path(
                                        P::InstanceSecondaryInterface {
                                            reservation: r,
                                            instance: i,
                                            interface: (j as u64).try_into()?,
                                        },
                                        round,
                                    )?;
                                }
                            }
                        }
                    }
                });
            }
            AllocationOutput::NetworkInterfaces(v) => {
                roots!(
                    &v.network_interfaces,
                    i,
                    x,
                    P::NetworkInterface { interface: i },
                    {
                        let p = P::NetworkInterface { interface: i };
                        s.list(p, L::SecurityGroups, x.groups.as_ref())?;
                        s.list(p, L::PrivateIpv4, x.private_ip_addresses.as_ref())?;
                        s.list(p, L::Ipv6, x.ipv6_addresses.as_ref())?;
                        s.list(p, L::Ipv4Prefixes, x.ipv4_prefixes.as_ref())?;
                        s.list(p, L::Ipv6Prefixes, x.ipv6_prefixes.as_ref())?;
                        s.list(p, L::Tags, x.tag_set.as_ref())?;
                    }
                );
            }
            AllocationOutput::Volumes(v) => {
                roots!(&v.volumes, i, x, P::Volume { volume: i }, {
                    let p = P::Volume { volume: i };
                    s.list(p, L::Tags, x.tags.as_ref())?;
                    s.list(p, L::VolumeAttachments, x.attachments.as_ref())?;
                    if let Some(items) = &x.attachments
                        && items.len() <= 128
                    {
                        for j in 0..items.len() {
                            s.path(
                                P::VolumeAttachment {
                                    volume: i,
                                    attachment: (j as u64).try_into()?,
                                },
                                round,
                            )?;
                        }
                    }
                });
            }
            AllocationOutput::InstanceAttribute(_) => {
                s.path(P::InstanceAttribute, round)?;
            }
        }
        Ok(s)
    }
}

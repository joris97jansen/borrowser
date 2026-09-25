# AG9g0d V2 RunInstances field-disposition contract

This is the closed **Borrowser logical policy**, reviewed against the
[AWS RunInstances API reference](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_RunInstances.html).
It is not an SDK mapping or proof of AWS wire completeness. Pass 4 must pin an SDK
version, audit every builder/nested member against this policy and prove one-to-one
projection. Newly exposed members require review; they cannot become passthrough.
An SDK field without a reviewed disposition must block projection.

`RunInstancesRequestV2` contains the complete logical `LaunchSpecV2`, its fingerprint,
exact deterministic token and explicit counts. Only validated requests are usable
by a future transport. Its nested reviewed/binding metadata is not sent as AWS
request parameters. `LaunchPolicyV2::fixed()` is protocol policy, never capacity
selection. All capacity comes from `LaunchApprovalV2`.

| Category / AWS field | Disposition | V2 meaning / future projection obligation |
| --- | --- | --- |
| ImageId | Explicitly populated | Exact approved AMI; no lookup or substitution. |
| InstanceType | Explicitly populated | Exact reviewed lexical identifier; no catalogue guessed in Rust. |
| MinCount, MaxCount | Explicitly populated | Both exactly 1. |
| ClientToken | Explicitly populated | Exact retained 64-character deterministic token. Never SDK-generated. |
| Placement.AvailabilityZone | Explicitly populated | Exact reviewed AZ; bind AZ ID, subnet and VPC in admission. |
| SubnetId, SecurityGroupId, SecurityGroup (top level) | Explicitly fixed omission with defined semantics | Subnet and SG IDs are populated in the one ENI. No group names or default VPC/SG fallback. |
| NetworkInterface | Explicitly populated | One newly created primary ENI, device/card indices 0, exact subnet/SGs, delete-on-termination true. Existing ENI reuse unsupported. |
| AssociatePublicIpAddress | Explicitly populated | False regardless of subnet public-address defaults; EIP allocation unsupported. |
| IPv6 count / primary IPv6 | Explicitly populated | Zero/false on supported interface fields; reject IPv6-enabled/IPv6-native subnet policy at future admission if API omission could inherit addresses. |
| IPv6 addresses/prefixes, delegated prefixes, secondary private addresses | Unsupported | No extra addressing. No top-level IPv6 fields combined with an ENI. |
| PrivateIpAddress | Explicitly fixed omission with defined semantics | AWS assigns one primary IPv4 from the exact subnet. Actual address is a future observation. No requested fixed address or pool. |
| IamInstanceProfile | Explicitly populated | Exact ARN, with reviewed profile unique ID and role ARN/unique ID binding. |
| BlockDeviceMapping | Explicitly populated | One reviewed root device, gp3 size/IOPS/throughput, encryption true, exact key ARN, delete-on-termination true. |
| SnapshotId | Explicitly fixed omission with defined semantics | Immutable approved AMI supplies its root snapshot; future AMI admission must reject extra mappings, billing products, instance storage and incompatible root facts. No independent snapshot override. |
| EbsCardIndex | Explicitly fixed omission with defined semantics | Standard primary EBS card, no alternate card selection; future type admission must establish compatible topology. |
| EBS AvailabilityZone / AvailabilityZoneId | Unsupported | Separate volume placement is not part of RunInstances; root resides with the instance. |
| EBS VolumeInitializationRate, OutpostArn, VirtualName, NoDevice | Unsupported | No provisioned initialization rate, Outposts placement, instance-store mapping or inherited-device suppression. |
| MetadataOptions | Explicitly populated | Endpoint enabled, tokens required, hop limit 1, IPv6 endpoint disabled, metadata tags disabled. |
| Monitoring | Explicitly populated | Detailed monitoring false. |
| EbsOptimized | Explicitly populated | True; approved type must support it, with cost review. |
| InstanceInitiatedShutdownBehavior | Explicitly populated | Stop, never terminate. OS shutdown cannot grant destruction authority. |
| DisableApiTermination | Explicitly populated | False, allowing a later separately authorized exact-instance termination. |
| DisableApiStop | Explicitly populated | True; no stop API authority is introduced. Does not prevent OS shutdown. |
| CapacityReservationSpecification | Explicitly populated | Preference none; no open/targeted reservation consumption. |
| HibernationOptions, EnclaveOptions | Explicitly populated | Both false. |
| Placement.Tenancy | Explicitly populated | Default/shared. Future admission must reject a dedicated-tenancy VPC. |
| Placement group, partition, host ID/resource group, affinity, spread domain | Unsupported | No placement groups, dedicated hosts or alternate scheduling. |
| KeyName | Explicitly fixed omission with defined semantics | No EC2 key pair. Approved AMI must provide only the separately reviewed collector capability. |
| InstanceMarketOptions | Explicitly fixed omission with defined semantics | On-Demand; all Spot/market options unsupported. |
| LaunchTemplate | Unsupported | No inherited configuration. |
| TagSpecification | Explicitly populated | Explicit sorted unique tags on instance, volume and ENI; reserved authority/operation tags derived by the spec constructor. No Spot-request tags. |
| UserData | Explicitly populated | Exact canonical bounded collector JSON, including LF; encode once as required by pinned SDK. No scripts or cloud-init. |
| PrivateDnsNameOptions | Explicitly populated | ip-name; resource-name A/AAAA records false. VPC DNS support remains reviewed static infrastructure. |
| MaintenanceOptions.AutoRecovery | Explicitly populated | Disabled. No automatic replacement/recovery permission. |
| RebootMigration | Unsupported | No Pass-2 configuration authority; pinned request-model audit required as described below. |
| CpuOptions | Explicitly fixed omission with defined semantics | Standard topology of exact reviewed type; future admission must corroborate its full vCPU/memory ceilings. No CPU customization or nested virtualization. |
| CreditSpecification | Unsupported | Burstable CPU-credit types rejected by future provider admission; no default unlimited-credit billing. |
| KernelId, RamdiskId | Unsupported | Linux HVM/EBS only; no paravirtual boot overrides. |
| AdditionalInfo | Unsupported | Reserved field never supplied. |
| LicenseSpecification | Unsupported | No separately billable license configuration. |
| ElasticGpuSpecification, ElasticInferenceAccelerator | Unsupported | No accelerators. |
| SecondaryInterface, additional ENIs/disks, network performance overrides | Unsupported | One primary ENI and one root EBS only. No instance-store-capable type. |
| Operator / managed-resource delegation | Unsupported | No alternate lifecycle controller. |
| DryRun | Explicitly fixed omission with defined semantics | This is a future real request contract, not a permission probe; Pass 2 never transmits it. |
| Fleet, Auto Scaling, CloudFormation host replacement | Unsupported | No alternate allocator, orchestration entry point or implicit replacement. |

Account/region, AZ ID/VPC, AMI provenance, role/profile IDs, trust and deployment
digests, ceilings and review metadata are **Borrowser binding/admission facts**.
Some have no RunInstances field; they must be validated before dispatch, not dropped
as unimportant. Exact type properties (native x86_64, nonburstable, EBS-only, HVM,
capacity/cost fit), immutable AMI mappings, profile-role binding, subnet/AZ/VPC,
DNS/endpoint policy and absence of incompatible defaults remain future provider
corroboration. Lexical acceptance and these documents alone do not establish them.

The subnet fixes zonal idempotency: account + region + approved AZ identity + exact
request/token are retained together. The token is not global AWS authority and
must not be reused in another region/AZ. Lost-response retry/reconciliation and
provider mismatch handling remain later passes.


## Reboot-migration boundary

Borrowser grants no reboot-migration configuration authority in Pass 2. Pass 4 must inspect
the pinned AWS Rust SDK RunInstances input. If that exact request shape exposes
`RebootMigration`, a reviewed contract update may fix it to disabled. If it does not,
Borrowser must not add an unplanned post-launch `ModifyInstanceMaintenanceOptions`
mutation to emulate it. No qualification conclusion may depend on reboot-migration
state until such a contract exists. The V2 data model rejects `reboot_migration` as
an unknown field rather than promising an unproven wire projection.

# AG9g0d / #1396 pinned AWS SDK boundary audit

Scope: non-mutating SDK execution and pure request projection. No production RunInstances transmission.

## Source and dependency identity

Audited EC2 crate: `aws-sdk-ec2 = 1.237.0`, archive SHA-256 `bfe29481f63a118c80f6bbded678711367bfc2b23f59b4351e7dfa6f439c3881`. STS `1.107.0`; S3 `1.137.0`. The lockfile freezes the resolved graph; Rust 1.92 validation is required.

Source paths are relative to that exact EC2 crate. The request input/builder is `src/operation/run_instances/_run_instances_input.rs`; operation setup is `src/operation/run_instances.rs`; query serializer is `src/protocol_serde/shape_run_instances_input.rs`. Nested type sources follow below.

The generated structs are non-exhaustive. Every dependency update requires re-auditing these 35 shapes / 157 members, behavior version, retries, timeouts, serialization and error normalization. Compilation alone cannot approve an update. `tests/fixtures/sdk-members-v1.json` retains the exact inventory.

## Complete member inventory

P = explicitly populated; O = explicitly fixed omission with defined semantics; U = unsupported. No member requires a frozen serialized-contract change. Unsupported parent branches are not constructed; all their children are U.

| Type / source | P members | O members | U members |
| --- | --- | --- | --- |
| `RunInstancesInput` (`operation/run_instances/_run_instances_input.rs`) | `block_device_mappings`, `image_id`, `instance_type`, `max_count`, `min_count`, `monitoring`, `placement`, `user_data`, `tag_specifications`, `capacity_reservation_specification`, `hibernation_options`, `metadata_options`, `enclave_options`, `private_dns_name_options`, `maintenance_options`, `disable_api_stop`, `disable_api_termination`, `instance_initiated_shutdown_behavior`, `client_token`, `network_interfaces`, `iam_instance_profile`, `ebs_optimized` | `key_name`, `security_group_ids`, `security_groups`, `subnet_id`, `instance_market_options`, `cpu_options`, `dry_run`, `private_ip_address` | `ipv6_address_count`, `ipv6_addresses`, `kernel_id`, `ramdisk_id`, `elastic_gpu_specification`, `elastic_inference_accelerators`, `launch_template`, `credit_specification`, `license_specifications`, `enable_primary_ipv6`, `network_performance_options`, `operator`, `secondary_interfaces`, `additional_info` |
| `BlockDeviceMapping` (`types/_block_device_mapping.rs`) | `ebs`, `device_name` | — | `no_device`, `virtual_name` |
| `InstanceIpv6Address` (`types/_instance_ipv6_address.rs`) | — | — | `ipv6_address`, `is_primary_ipv6` |
| `RunInstancesMonitoringEnabled` (`types/_run_instances_monitoring_enabled.rs`) | `enabled` | — | — |
| `Placement` (`types/_placement.rs`) | `tenancy`, `availability_zone` | `availability_zone_id` | `affinity`, `group_name`, `partition_number`, `host_id`, `spread_domain`, `host_resource_group_arn`, `group_id` |
| `ElasticGpuSpecification` (`types/_elastic_gpu_specification.rs`) | — | — | `r#type` |
| `ElasticInferenceAccelerator` (`types/_elastic_inference_accelerator.rs`) | — | — | `r#type`, `count` |
| `TagSpecification` (`types/_tag_specification.rs`) | `resource_type`, `tags` | — | — |
| `LaunchTemplateSpecification` (`types/_launch_template_specification.rs`) | — | — | `launch_template_id`, `launch_template_name`, `version` |
| `InstanceMarketOptionsRequest` (`types/_instance_market_options_request.rs`) | — | — | `market_type`, `spot_options` |
| `CreditSpecificationRequest` (`types/_credit_specification_request.rs`) | — | — | `cpu_credits` |
| `CpuOptionsRequest` (`types/_cpu_options_request.rs`) | — | — | `core_count`, `threads_per_core`, `amd_sev_snp`, `nested_virtualization` |
| `CapacityReservationSpecification` (`types/_capacity_reservation_specification.rs`) | `capacity_reservation_preference` | — | `capacity_reservation_target` |
| `HibernationOptionsRequest` (`types/_hibernation_options_request.rs`) | `configured` | — | — |
| `LicenseConfigurationRequest` (`types/_license_configuration_request.rs`) | — | — | `license_configuration_arn` |
| `InstanceMetadataOptionsRequest` (`types/_instance_metadata_options_request.rs`) | `http_tokens`, `http_put_response_hop_limit`, `http_endpoint`, `http_protocol_ipv6`, `instance_metadata_tags` | — | — |
| `EnclaveOptionsRequest` (`types/_enclave_options_request.rs`) | `enabled` | — | — |
| `PrivateDnsNameOptionsRequest` (`types/_private_dns_name_options_request.rs`) | `hostname_type`, `enable_resource_name_dns_a_record`, `enable_resource_name_dns_aaaa_record` | — | — |
| `InstanceMaintenanceOptionsRequest` (`types/_instance_maintenance_options_request.rs`) | `auto_recovery` | — | — |
| `InstanceNetworkPerformanceOptionsRequest` (`types/_instance_network_performance_options_request.rs`) | — | — | `bandwidth_weighting` |
| `OperatorRequest` (`types/_operator_request.rs`) | — | — | `principal` |
| `InstanceSecondaryInterfaceSpecificationRequest` (`types/_instance_secondary_interface_specification_request.rs`) | — | — | `delete_on_termination`, `device_index`, `private_ip_addresses`, `private_ip_address_count`, `secondary_subnet_id`, `interface_type`, `network_card_index` |
| `InstanceNetworkInterfaceSpecification` (`types/_instance_network_interface_specification.rs`) | `associate_public_ip_address`, `delete_on_termination`, `device_index`, `groups`, `ipv6_address_count`, `subnet_id`, `associate_carrier_ip_address`, `interface_type`, `network_card_index`, `primary_ipv6` | `description`, `private_ip_address` | `ipv6_addresses`, `network_interface_id`, `private_ip_addresses`, `secondary_private_ip_address_count`, `ipv4_prefixes`, `ipv4_prefix_count`, `ipv6_prefixes`, `ipv6_prefix_count`, `ena_srd_specification`, `connection_tracking_specification`, `ena_queue_count` |
| `IamInstanceProfileSpecification` (`types/_iam_instance_profile_specification.rs`) | `arn` | `name` | — |
| `EbsBlockDevice` (`types/_ebs_block_device.rs`) | `delete_on_termination`, `iops`, `volume_size`, `volume_type`, `kms_key_id`, `throughput`, `encrypted` | `snapshot_id`, `ebs_card_index` | `outpost_arn`, `availability_zone`, `volume_initialization_rate`, `availability_zone_id` |
| `Tag` (`types/_tag.rs`) | `key`, `value` | — | — |
| `SpotMarketOptions` (`types/_spot_market_options.rs`) | — | — | `max_price`, `spot_instance_type`, `block_duration_minutes`, `valid_until`, `instance_interruption_behavior` |
| `CapacityReservationTarget` (`types/_capacity_reservation_target.rs`) | — | — | `capacity_reservation_id`, `capacity_reservation_resource_group_arn` |
| `InstanceSecondaryInterfacePrivateIpAddressRequest` (`types/_instance_secondary_interface_private_ip_address_request.rs`) | — | — | `private_ip_address` |
| `PrivateIpAddressSpecification` (`types/_private_ip_address_specification.rs`) | — | — | `primary`, `private_ip_address` |
| `Ipv4PrefixSpecificationRequest` (`types/_ipv4_prefix_specification_request.rs`) | — | — | `ipv4_prefix` |
| `Ipv6PrefixSpecificationRequest` (`types/_ipv6_prefix_specification_request.rs`) | — | — | `ipv6_prefix` |
| `EnaSrdSpecificationRequest` (`types/_ena_srd_specification_request.rs`) | — | — | `ena_srd_enabled`, `ena_srd_udp_specification` |
| `ConnectionTrackingSpecificationRequest` (`types/_connection_tracking_specification_request.rs`) | — | — | `tcp_established_timeout`, `udp_stream_timeout`, `udp_timeout` |
| `EnaSrdUdpSpecificationRequest` (`types/_ena_srd_udp_specification_request.rs`) | — | — | `ena_srd_udp_enabled` |

## Exact populated values and binding-only facts

`r` is the frozen validated `RunInstancesRequestV2`, `l = r.spec.launch`, and
`p = l.policy`. Projection requires a genuine `DurableLaunchAttempt`, verifies
all retained document cross-bindings and matches its dispatch binding. It returns
only a private data wrapper, never a fluent operation builder or client.

| Logical source | SDK member / value |
| --- | --- |
| `l.ami.image_id`, `l.instance_type` | `RunInstancesInput.image_id`, `.instance_type`: exact strings, no lookup |
| `r.min_count`, `r.max_count`, `r.client_token` | `.min_count=1`, `.max_count=1`, exact `.client_token` |
| `l.availability_zone`, `p.tenancy` | `Placement.availability_zone`, `.tenancy=default` |
| `p.network_device_index`, `p.network_card_index` | ENI `.device_index=0`, `.network_card_index=0` |
| `l.subnet_id`, `l.security_group_ids` | ENI `.subnet_id`, exact sorted `.groups` |
| Frozen ordinary private primary ENI policy | ENI `.interface_type=interface`, `.associate_public_ip_address=false`, `.associate_carrier_ip_address=false`, `.delete_on_termination=true`, `.ipv6_address_count=0`, `.primary_ipv6=false` |
| `l.instance_profile_arn` | Profile `.arn`; no friendly name |
| `l.ami.root_device_name` | Sole `BlockDeviceMapping.device_name` |
| `l.root_volume` | Sole `.ebs`: `.volume_type=gp3`, exact `.volume_size`, `.iops`, `.throughput`, `.kms_key_id`; `.encrypted=true`, `.delete_on_termination=true` |
| Frozen metadata policy | `.http_endpoint=enabled`, `.http_tokens=required`, `.http_put_response_hop_limit=1`, `.http_protocol_ipv6=disabled`, `.instance_metadata_tags=disabled` |
| Frozen runtime policy | `.monitoring.enabled=false`, `.ebs_optimized=true`, `.instance_initiated_shutdown_behavior=stop`, `.disable_api_termination=false`, `.disable_api_stop=true` |
| Frozen capacity policy | `.capacity_reservation_specification.capacity_reservation_preference=none` |
| Frozen optional-feature policy | `.hibernation_options.configured=false`, `.enclave_options.enabled=false`, `.maintenance_options.auto_recovery=disabled` |
| Frozen DNS policy | `.hostname_type=ip-name`, `.enable_resource_name_dns_a_record=false`, `.enable_resource_name_dns_aaaa_record=false` |
| `l.tags` | Three `.tag_specifications`: instance, volume, network-interface; exact sorted key/value pairs, no added SDK tags |
| `l.user_data.as_bytes()` | `.user_data`: standard RFC 4648 Base64 with padding, no wrapping, exactly once; original terminal LF preserved |

Account, Region, AZ ID, VPC, unique role/profile IDs, review/provenance/trust and
resource/cost ceilings remain Borrowser validation/binding facts. They do not become
extra RunInstances parameters. Provider corroboration is not implemented in #1396.
All nine u64-to-i32 projection conversions (counts, device/card indices, IPv6 count,
root size/IOPS/throughput, metadata hop limit) use checked conversion before input
construction. Booleans are explicit even when the AWS default happens to agree.

## Reviewed omissions and unsupported behavior

- `connection_tracking_specification`: **Unsupported**, unset. No nested TCP/UDP
  timeouts; no claim about provider defaults and no post-launch mutation.
- `description`: fixed omission; no supplied ENI description or resulting-description
  claim. Reviewed tags carry identity metadata.
- `ena_srd_specification`, all nested ENA SRD settings, and `ena_queue_count`:
  **Unsupported**, unset. No network-performance override or default-performance claim.
- AZ ID is omitted because `Placement` documents AZ name/ID as mutually exclusive.
  The exact AZ ID remains a frozen Borrowser binding. No automatic AZ selection.
- CPU options are wholly omitted. All nested customizations, including AMD SEV-SNP
  and nested virtualization, are unsupported. No confidential-computing state claim.
- This exact `InstanceMaintenanceOptionsRequest` has only `auto_recovery`.
  It has no `RebootMigration` member. No state inference or maintenance mutation.
- Top-level subnet/security groups are absent because the one ENI supplies them.
  Primary private IPv4 is subnet-assigned; additional addresses/prefixes are unsupported.
- Root snapshot override is absent: immutable reviewed AMI supplies the snapshot.
  EBS card selection is absent; separate EBS AZ selectors and initialization-rate
  overrides are unsupported. Future admission must establish compatible AMI/type facts.
- Market options are absent for On-Demand; key pair, launch template, licenses,
  accelerators, secondary interfaces, operator delegation and dry-run are not supplied.
- Capacity reservation target is absent with explicit preference `none`.

The source serializer sends the supplied user-data string unchanged to the query
writer; it does not Base64-encode. Query escaping is separate. The generated
idempotency interceptor generates a token only when absent: Borrowser always supplies
and tests the retained token, and exposes no operation builder that could drop it.

## Explicit execution configuration

BehaviorVersion is exactly `v2026_01_12()`, never `latest()`. Direct authority-relevant
support crates are `aws-credential-types=1.2.14`, `aws-types=1.3.16`,
`aws-smithy-runtime-api=1.12.3`, `aws-smithy-types=1.5.0`,
`aws-smithy-async=1.2.14`, `aws-smithy-http-client=1.1.13`.
The lockfile also fixes `aws-smithy-runtime=1.11.3`, `aws-runtime=1.7.5` and
`aws-sigv4=1.4.5`. Base64 is `0.22.1`; zeroize is `1.8.1`.
Tokio `1.53.1` is a direct test runtime; production library calls require a compatible
caller runtime. The CLI creates no runtime and invokes none of these AWS paths.

All three clients derive from the same explicit low-level `SdkConfig`. No aws-config
crate, provider chain, credential refresh, shared profile, environment credential,
Region or endpoint discovery exists. The explicit rustls/AWS-LC HTTP builder leaves
proxy discovery disabled (unlike the SDK's default client factory). TLS verification
remains enabled. Production cannot supply an endpoint or injected connector.
S3 Express session authentication is explicitly disabled: the client must never
mint session credentials implicitly. Even directory-bucket-shaped synthetic input
is tested to produce only HeadBucket, not CreateSession. This is not directory-bucket
admission or evidence-plane approval.
SDK max attempts is exactly 1, standard/non-adaptive mode. Connect timeout is 5s,
operation-attempt timeout 30s and total operation timeout 30s: a single read or future
wire invocation receives no extra time budget from hidden retries. Controller retry
rules remain frozen. A failed read returns without controller retries.

## External secret file

Internal loader schema: `format=borrowser-aws-operator-session`, `schema_version=1`,
mandatory `access_key_id`, `secret_access_key`, `session_token`,
`expiration_unix_seconds`. No CLI entry accepts these values or a secret document.
It is separate from every persisted authority contract. Absolute path components
are opened by descriptor with no symlink following; final input must be a mode-0600,
effective-user-owned, single-link regular file. Size is 1–16,384 bytes. Unknown and
duplicate fields reject. Secrets are nonempty ASCII graphic strings bounded to
128/256/8192 bytes respectively. Expiry is explicit, checked and passed to SDK
credentials; no refresh is implemented. A later session cannot reset attempt state.

Raw bytes and parsed secret strings use zeroizing storage. The SDK credential
wrapper is private. Its own Debug reveals the access-key ID, so application wrappers
never delegate Debug to it; all three components stay redacted. Session wrappers have no secret accessors or serialization;
secret Debug output is a fixed redacted string. Errors contain only static messages,
never paths, parser contents or SDK response/error Debug output. SDK-internal
credential copies follow the pinned SDK's credential storage behavior; no claim of
complete allocator/OS-memory erasure is made.

## Read-only admission and no-send invariant

Only STS `GetCallerIdentity` and S3 `HeadBucket` are invoked. STS retains bounded
account/ARN/UserId in memory, requires the reviewed account and rejects root. It
mints no credentials. S3 supplies exact bucket and ExpectedBucketOwner, requires an
observed Region equal to deployment/evidence/endpoint Region, and performs no listing
or object operation. Neither admission grants allocation authority or adds an event.

EC2 client storage is private; there is no `run_instances()` or `.send()` consumer
for allocation. Projection data cannot grant authority. The local final gate samples
the existing Linux controller clock and checks the genuine capability's request,
spec/token/authorization binding, boot/time namespace and exclusive deadline. An
expired consumed attempt is locally definitely-not-transmitted; no capability is
reconstructed. Any future transport must consume the actual capability by value.

Normalization accepts synthetic/pinned SDK result types only. Exact
IdempotentParameterMismatch blocks as parameter conflict; access and throttling
responses map to their frozen categories. Other service responses remain unresolved;
SDK transport/timeouts after invocation are uncertain, never assumed untransmitted.
No unrestricted provider response/error is retained or emitted.

## Validation boundary

Tests inspect typed SDK fields and every omission on populated shapes. The 157-member
inventory records excluded nested branches. Synthetic SDK replay connectors replace
HTTP for STS/S3 tests; no real AWS calls are permitted. An isolated subprocess poisons
ambient AWS settings to test explicit admission configuration without racing process
environment changes. Existing local durability/replay tests and frozen vectors remain
required. Linux Rust-1.92 offline tests/build remain a separate validation requirement.

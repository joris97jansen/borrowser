//! Owned-field values and explicit exclusions through the production SDK boundary.
use super::{tests::*, *};
use crate::{
    aws::{ec2_infrastructure_reads_tests::reader, tests::response},
    provider::{
        allocation_value_v5::MemberV5 as M,
        ec2_allocation_observation_v5::{InstanceV5, ObservationDataV5 as D},
        management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2 as U},
        observation_v5::ObservationEntryV5,
    },
};
use serde_json::{Value, json};
const INSTANCES: ReadOperationV1 = ReadOperationV1::DescribeInstances;
fn instances(members: &str, reservation_metadata: &str) -> String {
    xml(
        INSTANCES,
        &format!(
            "<reservationSet><item><reservationId>reservation</reservationId>{reservation_metadata}<instancesSet><item><instanceId>i-bbbbbbbb</instanceId>{members}</item></instancesSet></item></reservationSet>"
        ),
    )
}
fn instance(r: &QueryResult) -> &InstanceV5 {
    r.records
        .iter()
        .find_map(|r| {
            if let D::Instance(v) = data(r) {
                Some(v)
            } else {
                None
            }
        })
        .unwrap()
}
fn instance_bytes(r: &QueryResult) -> Vec<u8> {
    r.records
        .iter()
        .find(|r| matches!(data(r), D::Instance(_)))
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
async fn observe(op: ReadOperationV1, body: &str) -> QueryResult {
    let (mut session, _, _) = reader(vec![response(200, body, None)]);
    session.allocation(read(op), true).await.unwrap()
}
#[tokio::test]
async fn instance_address_summaries_are_independent_of_eni_presence_values_and_canonical_identity()
{
    let summaries = "<privateIpAddress>10.0.0.9</privateIpAddress><ipAddress>198.51.100.9</ipAddress><ipv6Address>2001:db8::9</ipv6Address>";
    for eni in [
        "",
        "<networkInterfaceSet/>",
        "<networkInterfaceSet><item><privateIpAddress>10.0.0.1</privateIpAddress><association><publicIp>192.0.2.1</publicIp></association><ipv6AddressesSet><item><ipv6Address>2001:db8::1</ipv6Address></item></ipv6AddressesSet></item></networkInterfaceSet>",
    ] {
        let body = instances(&format!("{summaries}{eni}"), "");
        let r = observe(INSTANCES, &body).await;
        check(&r, 1, true, CoverageStatus::Complete);
        assert_eq!(
            instance(&r).private_ipv4,
            M::Present("10.0.0.9".parse().unwrap())
        );
        assert_eq!(
            instance(&r).public_ipv4,
            M::Present("198.51.100.9".parse().unwrap())
        );
        assert_eq!(
            instance(&r).ipv6,
            M::Present("2001:db8::9".parse().unwrap())
        );
        assert_eq!(
            r.coverage.records,
            if eni.contains("<item>") { 4 } else { 2 }
        );
        if let Some(v) = r.records.iter().find_map(|r| {
            if let D::NetworkInterface(v) = data(r) {
                Some(v)
            } else {
                None
            }
        }) {
            assert_eq!(v.private_ipv4, M::Present("10.0.0.1".parse().unwrap()));
            assert!(
                matches!(&v.association,V::Present(a) if a.public_ip==M::Present("192.0.2.1".parse().unwrap()))
            );
            assert!(
                matches!(&v.ipv6,V::Present(v) if v.as_slice()[0].address==M::Present("2001:db8::1".parse().unwrap()))
            );
        }
        // Each scalar changes only the instance evidence, never source credit or ENI facts.
        for (old, new) in [
            ("10.0.0.9", "10.0.0.10"),
            ("198.51.100.9", "198.51.100.10"),
            ("2001:db8::9", "2001:db8::a"),
        ] {
            let changed = observe(INSTANCES, &body.replace(old, new)).await;
            check(&changed, 1, true, CoverageStatus::Complete);
            assert_ne!(instance_bytes(&r), instance_bytes(&changed));
            assert_eq!(r.coverage.records, changed.coverage.records);
            let independent = |r: &QueryResult| {
                r.records
                    .iter()
                    .filter(|r| !matches!(data(r), D::Instance(_)))
                    .map(|r| r.canonical_bytes().unwrap())
                    .collect::<Vec<_>>()
            };
            assert_eq!(independent(&r), independent(&changed));
        }
    }
}
#[tokio::test]
async fn address_omissions_empty_malformed_and_duplicates_keep_explicit_dispositions() {
    for (wire, field, valid) in [
        ("privateIpAddress", "private_ipv4", "10.0.0.9"),
        ("ipAddress", "public_ipv4", "198.51.100.9"),
        ("ipv6Address", "ipv6", "2001:db8::9"),
    ] {
        for (raw, state) in [
            (None, "not-returned"),
            (Some(""), "empty"),
            (Some("bad-address"), "malformed"),
            (Some(valid), "present"),
        ] {
            let member = raw
                .map(|v| format!("<{wire}>{v}</{wire}>"))
                .unwrap_or_default();
            let r = observe(INSTANCES, &instances(&member, "")).await;
            check(
                &r,
                1,
                true,
                if state == "malformed" {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                } else {
                    CoverageStatus::Complete
                },
            );
            assert_eq!(r.coverage.records, 2);
            assert_eq!(r.records.len(), 4);
            let value = serde_json::to_value(instance(&r)).unwrap();
            assert_eq!(value[field]["kind"], state);
            if matches!(state, "malformed" | "present") {
                assert_eq!(value[field]["value"], raw.unwrap());
            }
            assert!(matches!(instance(&r).id, M::Present(_)));
        }
        let first = token(INSTANCES, &instances("", ""), "next");
        let duplicate = instances(
            &format!("<{wire}>{valid}</{wire}><{wire}>{valid}</{wire}>"),
            "",
        );
        let (mut session, _, _) = reader(vec![
            response(200, &first, None),
            response(200, &duplicate, None),
        ]);
        let r = session.allocation(read(INSTANCES), true).await.unwrap();
        check(
            &r,
            2,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert_eq!(r.coverage.records, 2);
        assert_eq!(r.records.len(), 4);
        assert!(
            r.records
                .iter()
                .all(|r| matches!(r,ObservationEntryV5::V5(v) if u64::from(v.source.page)==1))
        );
    }
}
#[tokio::test]
async fn excluded_metadata_has_no_observation_meaning_field_limits_or_singleton_monitoring() {
    let oversized = "unowned".repeat(600); // >2 KiB, but safely below the transport ceiling.
    let ami = xml(
        ReadOperationV1::DescribeImages,
        "<imagesSet><item><imageId>ami-bbbbbbbb</imageId><blockDeviceMapping><item><deviceName>/dev/sda1</deviceName><ebs><snapshotId>snap-bbbbbbbb</snapshotId></ebs></item></blockDeviceMapping></item></imagesSet>",
    );
    let provision = format!(
        "<volumeSize>16</volumeSize><volumeType>future-type</volumeType><iops>3000</iops><throughput>125</throughput><encrypted>true</encrypted><kmsKeyId>{oversized}</kmsKeyId><deleteOnTermination>true</deleteOnTermination>"
    );
    let embedded = "<networkInterfaceSet><item><association><publicIp>192.0.2.1</publicIp></association><privateIpAddressesSet><item><privateIpAddress>10.0.0.1</privateIpAddress><association><publicIp>192.0.2.2</publicIp></association></item></privateIpAddressesSet></item></networkInterfaceSet>";
    let inst = instances(embedded, "");
    let eni = xml(
        ReadOperationV1::DescribeNetworkInterfaces,
        "<networkInterfaceSet><item><requesterId>owned-requester</requesterId><requesterManaged>false</requesterManaged><operator><principal>owned-principal</principal></operator><association><publicIp>192.0.2.1</publicIp></association><privateIpAddressesSet><item><privateIpAddress>10.0.0.1</privateIpAddress><association><publicIp>192.0.2.2</publicIp></association></item></privateIpAddressesSet></item></networkInterfaceSet>",
    );
    let dns = format!("<publicDnsName>{oversized}</publicDnsName>");
    let private_dns = format!("<privateDnsName>{oversized}</privateDnsName>");
    let cases = [
        (
            ReadOperationV1::DescribeImages,
            ami.clone(),
            ami.replace("</ebs>", &format!("{provision}</ebs>")),
        ),
        (
            INSTANCES,
            inst.clone(),
            inst.replace(
                "</reservationId>",
                &format!("</reservationId><requesterId>{oversized}</requesterId>"),
            )
            .replace("</association>", &format!("{dns}</association>"))
            .replace(
                "</instanceId>",
                &format!("</instanceId>{private_dns}<dnsName>{oversized}</dnsName>"),
            )
            .replace(
                "</privateIpAddress>",
                &format!("</privateIpAddress>{private_dns}"),
            ),
        ),
        (
            ReadOperationV1::DescribeNetworkInterfaces,
            eni.clone(),
            eni.replace("</association>", &format!("{dns}</association>"))
                .replace(
                    "</requesterManaged>",
                    &format!("</requesterManaged>{private_dns}"),
                )
                .replace(
                    "</privateIpAddress>",
                    &format!("</privateIpAddress>{private_dns}"),
                ),
        ),
    ];
    let cases = cases.into_iter().flat_map(|(op, baseline, with_unowned)| {
        let repeated = with_unowned
            .replace(&dns, &format!("{dns}<publicDnsName>last</publicDnsName>"))
            .replace(
                &format!("<requesterId>{oversized}</requesterId>"),
                &format!("<requesterId>{oversized}</requesterId><requesterId>last</requesterId>"),
            );
        [
            (op, baseline.clone(), with_unowned),
            (op, baseline, repeated),
        ]
    });
    for (op, baseline, with_unowned) in cases {
        let before = observe(op, &baseline).await;
        check(&before, 1, true, CoverageStatus::Complete);
        let (mut session, _, round) = reader(vec![response(200, &with_unowned, None)]);
        let after = session.allocation(read(op), true).await.unwrap();
        check(&after, 1, true, CoverageStatus::Complete);
        assert_eq!(round.failure(), None);
        assert_eq!(before.coverage, after.coverage);
        let bytes = |r: &QueryResult| {
            r.records
                .iter()
                .map(|r| r.canonical_bytes().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(bytes(&before), bytes(&after));
        if op == ReadOperationV1::DescribeNetworkInterfaces {
            let D::NetworkInterface(v) = data(&after.records[0]) else {
                panic!()
            };
            assert_eq!(
                v.requester.identity,
                M::Present("owned-requester".to_owned().try_into().unwrap())
            );
            assert_eq!(v.requester.managed, V::Present(false));
            assert!(
                matches!(&v.operator,V::Present(v) if v.managed==V::Unavailable(U::NotReturned) && matches!(v.principal,M::Present(_)))
            );
        }
    }
}
#[tokio::test]
async fn excluding_metadata_keeps_sdk_decode_failures_and_whole_response_safeguards() {
    for unowned in [
        "<encrypted>not-a-boolean</encrypted>",
        "<volumeSize>not-an-integer</volumeSize>",
        "<kmsKeyId>&undefined;</kmsKeyId>",
    ] {
        let body = xml(
            ReadOperationV1::DescribeImages,
            &format!(
                "<imagesSet><item><blockDeviceMapping><item><ebs><snapshotId>snap-bbbbbbbb</snapshotId>{unowned}</ebs></item></blockDeviceMapping></item></imagesSet>"
            ),
        );
        let r = observe(ReadOperationV1::DescribeImages, &body).await;
        check(
            &r,
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert!(r.records.is_empty());
        assert_eq!(r.coverage.records, 0);
    }
    let body = instances(
        "",
        &format!(
            "<requesterId>{}</requesterId>",
            "x".repeat(crate::provider::limits::RESPONSE_BYTES as usize)
        ),
    );
    let r = observe(INSTANCES, &body).await;
    check(
        &r,
        1,
        false,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(
            crate::provider::limits::LimitKind::ResponseBytes,
        )),
    );
    assert!(r.records.is_empty());
}

// Independent expected values for rich fixtures: every owned variant has semantic
// assertions, including source-specific omissions and the explicitly excluded fields.
pub(super) fn assert_rich_fields(record: &ObservationEntryV5) {
    let value = serde_json::to_value(data(record)).unwrap();
    let expect = |pointer: &str, expected: Value| {
        assert_eq!(
            value.pointer(pointer),
            Some(&expected),
            "{} {pointer}",
            value["kind"]
        )
    };
    match data(record) {
        D::Image(_) => {
            expect("/id/value", json!("ami-bbbbbbbb"));
            expect("/owner/value", json!("222222222222"));
            for path in [
                "/architecture/value",
                "/platform/value",
                "/virtualization/value",
                "/root_type/value",
            ] {
                expect(path, json!("future-value"));
            }
            expect(
                "/mappings/value/0/device/value",
                json!("returned-device_name"),
            );
            expect(
                "/mappings/value/0/virtual_name/value",
                json!("returned-virtual_name"),
            );
            expect(
                "/mappings/value/0/no_device/value",
                json!("returned-no_device"),
            );
            expect(
                "/mappings/value/0/ebs/value",
                json!({"snapshot":{"kind":"present","value":"snap-bbbbbbbb"}}),
            );
            expect(
                "/product_codes/value/0/id/value",
                json!("returned-product_code_id"),
            );
            expect("/usage_operation/value", json!("returned-usage_operation"));
        }
        D::InstanceType(_) => {
            expect("/name/value", json!("m7i.large"));
            expect(
                "/processor/value/architectures/value/0/value",
                json!("future-value"),
            );
            for field in ["virtualization", "root_types", "usage_classes"] {
                expect(&format!("/{field}/value/0/value"), json!("future-value"));
            }
            for field in ["burstable", "instance_store", "supported_in_region"] {
                expect(&format!("/{field}/value"), json!(false));
            }
            for field in ["default_vcpus", "default_cores", "default_threads"] {
                expect(&format!("/cpu/value/{field}/value"), json!("-1"));
            }
            expect("/memory/value/mib/value", json!("-1"));
            expect("/network/value/max_interfaces/value", json!("-1"));
            expect("/network/value/max_cards/value", json!("-1"));
            expect("/ebs/value/support/value", json!("future-value"));
            expect(
                "/ebs/value/performance/value/maximum_iops/value",
                json!("-1"),
            );
            expect(
                "/ebs/value/performance/value/maximum_throughput_mbps/value",
                json!("3ff4000000000000"),
            );
            for field in ["gpu", "fpga", "inference", "media", "neuron"] {
                expect(
                    &format!("/{field}/value/devices/value/0/name/value"),
                    json!("returned-name"),
                );
                expect(
                    &format!("/{field}/value/devices/value/0/count/value"),
                    json!("-1"),
                );
            }
            expect(
                "/neuron/value/devices/value/0/manufacturer/kind",
                json!("not-exposed-by-source"),
            );
        }
        D::TypeOffering(_) => {
            expect("/name/value", json!("m7i.large"));
            expect("/location_type/value", json!("future-value"));
            expect("/location/value", json!("returned-location"));
        }
        D::ProfileAssociation(_) => {
            expect("/association/value", json!("iip-assoc-bbbbbbbb"));
            expect("/instance/value", json!("i-bbbbbbbb"));
            expect("/profile/value/id/value", json!("AIPA00000000000000000"));
            expect("/timestamp/value/seconds", json!("-1"));
        }
        D::Reservation(_) => {
            expect("/id/value", json!("returned-reservation_id"));
            expect("/owner/value", json!("222222222222"));
            expect("/instances/count", json!(1));
            assert!(value.get("requester").is_none());
        }
        D::Instance(_) => {
            expect("/id/value", json!("i-bbbbbbbb"));
            expect("/private_ipv4/value", json!("10.0.0.9"));
            expect("/public_ipv4/value", json!("198.51.100.9"));
            expect("/ipv6/value", json!("2001:db8::9"));
            expect("/reservation_owner/value", json!("222222222222"));
            expect("/token/value", json!("returned-client_token"));
            expect("/placement/value/zone/value", json!("eu-central-1a"));
            expect("/placement/value/tenancy/value", json!("future-value"));
            expect("/subnet/value", json!("subnet-bbbbbbbb"));
            expect("/vpc/value", json!("vpc-bbbbbbbb"));
            expect("/image/value", json!("ami-bbbbbbbb"));
            expect("/state/value/code/value", json!("-1"));
            expect("/profile/value/id/value", json!("AIPA00000000000000000"));
            expect("/interfaces/count", json!(1));
            expect("/ebs_mappings/count", json!(1));
            expect("/secondary_interfaces/count", json!(1));
            expect("/cpu/value/cores/value", json!("-1"));
            expect("/tags/value/0/key/value", json!("returned-key"));
            expect(
                "/operator/value/principal/value",
                json!("service.amazonaws.com"),
            );
        }
        D::InstanceOptions(_) => {
            expect("/metadata/value/hop_limit/value", json!("-1"));
            expect("/metadata/value/tokens/value", json!("future-value"));
            expect("/monitoring/value/state/value", json!("future-value"));
            expect("/ebs_optimized/value", json!(false));
            expect(
                "/capacity/value/target/value/reservation/value",
                json!("returned-capacity_reservation_id"),
            );
            expect("/capacity_block/value", json!("returned-capacity_block_id"));
            expect("/hibernation/value/configured/value", json!(false));
            expect("/enclave/value/enabled/value", json!(false));
            expect(
                "/maintenance/value/auto_recovery/value",
                json!("future-value"),
            );
            expect("/dns/value/hostname_type/value", json!("future-value"));
            expect("/dns/value/dns_a/value", json!(false));
            expect("/dns/value/dns_aaaa/value", json!(false));
        }
        D::ExcludedFeatures(v) => {
            let ObservationEntryV5::V5(record) = record else {
                panic!("allocation fixture must use V5")
            };
            if record.query.operation == INSTANCES {
                expect("/key_pair/value", json!("returned-key_name"));
                expect(
                    "/licenses/value/0/arn/value",
                    json!("returned-license_configuration_arn"),
                );
                expect(
                    "/elastic_gpu/value/0/id/value",
                    json!("returned-elastic_gpu_association_id"),
                );
                expect(
                    "/elastic_inference/value/0/id/value",
                    json!("returned-elastic_inference_accelerator_association_id"),
                );
                expect("/placement_group/value", json!("returned-group_name"));
                expect("/dedicated_host/value", json!("returned-host_id"));
            } else {
                expect("/key_pair/kind", json!("not-exposed-by-source"));
            }
            if matches!(
                record.query.operation,
                INSTANCES | ReadOperationV1::DescribeImages
            ) {
                expect("/kernel/value", json!("returned-kernel_id"));
                expect("/ramdisk/value", json!("returned-ramdisk_id"));
            } else {
                expect("/kernel/kind", json!("not-exposed-by-source"));
                expect("/ramdisk/kind", json!("not-exposed-by-source"));
            }
            if record.query.operation != ReadOperationV1::DescribeImages {
                expect("/outpost/value", json!("returned-outpost_arn"));
            } else {
                expect("/outpost/kind", json!("not-exposed-by-source"));
            }
            assert!(matches!(v.resource_id, M::Present(_)));
        }
        D::NetworkInterface(v) => {
            expect("/id/value", json!("eni-bbbbbbbb"));
            expect("/private_ipv4/value", json!("10.0.0.1"));
            expect("/ipv4/value/0/address/value", json!("10.0.0.1"));
            expect("/ipv6/value/0/address/value", json!("2001:db8::1"));
            expect("/association/value/public_ip/value", json!("192.0.2.1"));
            expect("/association/value/carrier_ip/value", json!("192.0.2.2"));
            expect(
                "/association/value/customer_owned_ip/value",
                json!("192.0.2.3"),
            );
            expect("/groups/value/0/id/value", json!("sg-bbbbbbbb"));
            expect(
                "/ipv4_prefixes/value/0/value",
                json!({"network": 167772160, "prefix": 24}),
            );
            expect("/ipv6_prefixes/value/0/value", json!("2001:db8::/64"));
            expect("/operator/value/managed/value", json!(false));
            expect("/operator/value/hidden_by_default/value", json!(false));
            expect(
                "/operator/value/principal/value",
                json!("service.amazonaws.com"),
            );
            assert!(value["association"]["value"].get("public_dns").is_none());
            if matches!(v.enclosing_instance, M::Present(_)) {
                expect("/requester/identity/kind", json!("not-exposed-by-source"));
            } else {
                expect("/requester/identity/value", json!("returned-requester_id"));
                expect("/requester/managed/value", json!(false));
            }
        }
        D::InstanceEniAttachment(_) => {
            expect("/enclosing_instance/value", json!("i-bbbbbbbb"));
            expect("/enclosing_interface/value", json!("eni-bbbbbbbb"));
            expect("/attachment/value/id/value", json!("eni-attach-bbbbbbbb"));
            expect(
                "/attachment/value/delete_on_termination/value",
                json!(false),
            );
        }
        D::StandaloneEniAttachment(_) => {
            expect("/enclosing_interface/value", json!("eni-bbbbbbbb"));
            expect("/attachment/value/instance/value", json!("i-bbbbbbbb"));
            expect("/attachment/value/fields/card_index/value", json!("-1"));
            expect(
                "/attachment/value/fields/delete_on_termination/value",
                json!(false),
            );
        }
        D::InstanceEbsMapping(_) => {
            expect("/device/value", json!("returned-device_name"));
            expect("/ebs/value/volume/value", json!("vol-bbbbbbbb"));
            expect("/ebs/value/volume_owner/value", json!("222222222222"));
            expect("/ebs/value/delete_on_termination/value", json!(false));
            expect(
                "/ebs/value/operator/value/principal/value",
                json!("service.amazonaws.com"),
            );
        }
        D::Volume(_) => {
            expect("/id/value", json!("vol-bbbbbbbb"));
            expect("/snapshot/value", json!("snap-bbbbbbbb"));
            expect("/volume_type/value", json!("future-value"));
            for field in ["size_gib", "iops", "throughput_mib_s"] {
                expect(&format!("/{field}/value"), json!("-1"));
            }
            expect("/encrypted/value", json!(false));
            expect("/multi_attach/value", json!(false));
            expect(
                "/key/value",
                json!(
                    "arn:aws:kms:eu-central-1:222222222222:key/12345678-1234-1234-1234-123456789abc"
                ),
            );
            expect("/attachments/count", json!(1));
            expect(
                "/operator/value/principal/value",
                json!("service.amazonaws.com"),
            );
        }
        D::VolumeAttachment(_) => {
            expect("/enclosing_volume/value", json!("vol-bbbbbbbb"));
            expect("/volume/value", json!("vol-cccccccc"));
            expect("/instance/value", json!("i-bbbbbbbb"));
            expect(
                "/associated_resource/value",
                json!("returned-associated_resource"),
            );
            expect(
                "/instance_owning_service/value",
                json!("returned-instance_owning_service"),
            );
            expect("/delete_on_termination/value", json!(false));
        }
        D::UnsupportedSecondaryInterface(_) => {
            expect("/enclosing_instance/value", json!("i-bbbbbbbb"));
            expect("/id/value", json!("returned-secondary_interface_id"));
            expect("/interface_type/value", json!("future-value"));
        }
        D::InstanceAttributes(_) => {
            expect("/id/value", json!("i-bbbbbbbb"));
            expect("/user_data/value/value/value", json!("VFE9PQ=="));
            expect(
                "/shutdown_behavior/value/value/value",
                json!("returned-value"),
            );
            expect("/disable_api_termination/value/value/value", json!(false));
            expect("/disable_api_stop/value/value/value", json!(false));
        }
    }
}

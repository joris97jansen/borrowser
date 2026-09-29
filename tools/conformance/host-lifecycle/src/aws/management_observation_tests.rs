use super::{
    configuration,
    management_observation::*,
    response_limits::{BoundedHttp, ObservationRound},
    tests::{replay, response, secret},
};
use crate::provider::{
    inventory_v2::AttachmentV2, management_observation_v2::*, observation::ProviderText,
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;

fn missing<T>() -> ObservationValueV2<T> {
    ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotReturned)
}
fn present<T>(v: T) -> ObservationValueV2<T> {
    ObservationValueV2::Present(v)
}
fn literal(v: &str) -> ObservationValueV2<ProviderText> {
    present(v.to_owned().try_into().unwrap())
}
fn client(
    xml: &str,
) -> (
    aws_sdk_ec2::Client,
    aws_smithy_http_client::test_util::StaticReplayClient,
) {
    let replay = replay(vec![response(200, xml, None)]);
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(
            SharedHttpClient::new(replay.clone()),
            ObservationRound::test(),
        ),
    )
    .unwrap();
    (
        aws_sdk_ec2::Client::from_conf(
            aws_sdk_ec2::config::Builder::from(&conf)
                .use_fips(false)
                .use_dual_stack(false)
                .build(),
        ),
        replay,
    )
}
fn instances_xml(instance: &str) -> String {
    format!(
        "<DescribeInstancesResponse xmlns=\"http://ec2.amazonaws.com/doc/2016-11-15/\"><reservationSet><item><instancesSet><item>{instance}</item></instancesSet></item></reservationSet></DescribeInstancesResponse>"
    )
}
fn interfaces_xml(interface: &str) -> String {
    format!(
        "<DescribeNetworkInterfacesResponse xmlns=\"http://ec2.amazonaws.com/doc/2016-11-15/\"><networkInterfaceSet><item>{interface}</item></networkInterfaceSet></DescribeNetworkInterfacesResponse>"
    )
}
fn volumes_xml(volume: &str) -> String {
    format!(
        "<DescribeVolumesResponse xmlns=\"http://ec2.amazonaws.com/doc/2016-11-15/\"><volumeSet><item>{volume}</item></volumeSet></DescribeVolumesResponse>"
    )
}
fn assert_no_absence<T: serde::Serialize>(value: &T) {
    fn check(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(o) => {
                assert_ne!(o.get("kind"), Some(&serde_json::json!("absent")));
                o.values().for_each(check);
            }
            serde_json::Value::Array(a) => a.iter().for_each(check),
            _ => (),
        }
    }
    check(&serde_json::to_value(value).unwrap());
}

#[tokio::test]
async fn all_five_operator_locations_use_real_pinned_deserializers() {
    let mut cases = vec![(String::new(), None, None, None, false)];
    for managed in [None, Some(false), Some(true)] {
        for principal in [None, Some("service.example")] {
            for hidden in [None, Some(false), Some(true)] {
                let mut xml = "<operator>".to_owned();
                if let Some(v) = managed {
                    xml.push_str(&format!("<managed>{v}</managed>"));
                }
                if let Some(v) = principal {
                    xml.push_str(&format!("<principal>{v}</principal>"));
                }
                if let Some(v) = hidden {
                    xml.push_str(&format!("<hiddenByDefault>{v}</hiddenByDefault>"));
                }
                xml.push_str("</operator>");
                cases.push((xml, managed, principal, hidden, true));
            }
        }
    }
    cases.push(("<operator/>".to_owned(), None, None, None, true));
    for (xml, managed, principal, hidden, object_present) in cases {
        let expected = if object_present {
            present(OperatorEvidenceV2 {
                managed: managed.map_or_else(missing, present),
                principal: principal.map_or_else(missing, literal),
                hidden_by_default: hidden.map_or_else(missing, present),
            })
        } else {
            missing()
        };
        let body = instances_xml(&format!(
            "<instanceId>i-01</instanceId>{xml}<networkInterfaceSet><item><networkInterfaceId>eni-01</networkInterfaceId>{xml}</item></networkInterfaceSet><blockDeviceMapping><item><ebs><volumeId>vol-01</volumeId>{xml}</ebs></item></blockDeviceMapping>"
        ));
        let (sdk, replay) = client(&body);
        let output = sdk
            .describe_instances()
            .instance_ids("i-01")
            .send()
            .await
            .unwrap();
        assert_eq!(replay.actual_requests().count(), 1);
        let instance = &output.reservations()[0].instances()[0];
        let eni = &instance.network_interfaces()[0];
        let mapping = &instance.block_device_mappings()[0];
        for decoded in [
            instance.operator(),
            eni.operator(),
            mapping.ebs().unwrap().operator(),
        ] {
            assert_eq!(decoded.is_some(), object_present);
            if let Some(decoded) = decoded {
                assert_eq!(decoded.managed(), managed);
                assert_eq!(decoded.principal(), principal);
                assert_eq!(decoded.hidden_by_default(), hidden);
            }
        }
        assert_eq!(instance_operator(instance).unwrap(), expected);
        let (source, op, requester) = instance_interface(instance, eni).unwrap();
        assert_eq!(op, expected);
        assert_eq!(
            source,
            NetworkInterfaceSourceV2::Instance {
                instance: present("i-01".parse().unwrap())
            }
        );
        assert_eq!(
            requester.managed,
            ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotExposedBySource)
        );
        assert_eq!(
            requester.identity,
            ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotExposedBySource)
        );
        let mapping = instance_ebs(instance, mapping).unwrap();
        if let AttachmentV2::InstanceEbsMapping {
            ebs: ObservationValueV2::Present(ebs),
            ..
        } = &mapping
        {
            assert_eq!(ebs.operator, expected);
        } else {
            panic!("present EBS object lost");
        }
        assert_no_absence(&mapping);
        let (sdk, replay) = client(&interfaces_xml(&format!(
            "<networkInterfaceId>eni-01</networkInterfaceId>{xml}"
        )));
        let output = sdk
            .describe_network_interfaces()
            .network_interface_ids("eni-01")
            .send()
            .await
            .unwrap();
        assert_eq!(replay.actual_requests().count(), 1);
        let eni = &output.network_interfaces()[0];
        assert_eq!(eni.operator().is_some(), object_present);
        if let Some(decoded) = eni.operator() {
            assert_eq!(
                (
                    decoded.managed(),
                    decoded.principal(),
                    decoded.hidden_by_default()
                ),
                (managed, principal, hidden)
            );
        }
        let normalized = standalone_interface(eni).unwrap();
        assert_eq!(normalized.1, expected);
        assert_no_absence(&normalized);
        let (sdk, replay) = client(&volumes_xml(&format!("<volumeId>vol-01</volumeId>{xml}")));
        let output = sdk
            .describe_volumes()
            .volume_ids("vol-01")
            .send()
            .await
            .unwrap();
        assert_eq!(replay.actual_requests().count(), 1);
        let volume = &output.volumes()[0];
        assert_eq!(volume.operator().is_some(), object_present);
        if let Some(decoded) = volume.operator() {
            assert_eq!(
                (
                    decoded.managed(),
                    decoded.principal(),
                    decoded.hidden_by_default()
                ),
                (managed, principal, hidden)
            );
        }
        assert_eq!(volume_operator(volume).unwrap(), expected);
        assert_no_absence(&expected);
    }
}

#[tokio::test]
async fn requester_members_are_independent_of_each_other_and_operator() {
    for managed in [None, Some(false), Some(true)] {
        for identity in [None, Some("service.example")] {
            let mut xml = "<networkInterfaceId>eni-01</networkInterfaceId><operator><managed>false</managed><principal>other.example</principal></operator>".to_owned();
            if let Some(v) = managed {
                xml.push_str(&format!("<requesterManaged>{v}</requesterManaged>"));
            }
            if let Some(v) = identity {
                xml.push_str(&format!("<requesterId>{v}</requesterId>"));
            }
            let (sdk, _) = client(&interfaces_xml(&xml));
            let out = sdk.describe_network_interfaces().send().await.unwrap();
            let decoded = &out.network_interfaces()[0];
            assert_eq!(decoded.requester_managed(), managed);
            assert_eq!(decoded.requester_id(), identity);
            let normalized = standalone_interface(decoded).unwrap();
            assert_eq!(normalized.2.managed, managed.map_or_else(missing, present));
            assert_eq!(
                normalized.2.identity,
                identity.map_or_else(missing, literal)
            );
            assert_no_absence(&normalized);
        }
    }
}

#[tokio::test]
async fn ebs_attachment_omissions_never_depend_on_managed_metadata() {
    for associated in [false, true] {
        for owning in [false, true] {
            for instance_present in [false, true] {
                for device_present in [false, true] {
                    let mut fields = "<volumeId>vol-02</volumeId><status>attached</status><deleteOnTermination>false</deleteOnTermination><ebsCardIndex>-1</ebsCardIndex><attachTime>2020-01-01T00:00:00Z</attachTime>".to_owned();
                    if associated {
                        fields.push_str("<associatedResource>arn:aws:service:eu-central-1:111111111111:resource/example</associatedResource>");
                    }
                    if owning {
                        fields.push_str(
                            "<instanceOwningService>service.example</instanceOwningService>",
                        );
                    }
                    if instance_present {
                        fields.push_str("<instanceId>i-01</instanceId>");
                    }
                    if device_present {
                        fields.push_str("<device>/dev/sda1</device>");
                    }
                    let (sdk, _) = client(&volumes_xml(&format!(
                        "<volumeId>vol-01</volumeId><attachmentSet><item>{fields}</item><item>{fields}</item></attachmentSet>"
                    )));
                    let out = sdk.describe_volumes().send().await.unwrap();
                    let volume = &out.volumes()[0];
                    assert_eq!(volume.attachments().len(), 2);
                    let decoded = &volume.attachments()[0];
                    assert_eq!(decoded.instance_id(), instance_present.then_some("i-01"));
                    assert_eq!(decoded.device(), device_present.then_some("/dev/sda1"));
                    let normalized = volume_attachment(volume, decoded).unwrap();
                    assert_eq!(
                        normalized,
                        volume_attachment(volume, &volume.attachments()[1]).unwrap()
                    );
                    if let AttachmentV2::VolumeAttachment {
                        described_volume,
                        attachment,
                    } = &normalized
                    {
                        assert_eq!(*described_volume, present("vol-01".parse().unwrap()));
                        assert_eq!(attachment.volume, present("vol-02".parse().unwrap()));
                        assert_eq!(
                            attachment.instance,
                            if instance_present {
                                present("i-01".parse().unwrap())
                            } else {
                                missing()
                            }
                        );
                        assert_eq!(
                            attachment.device,
                            if device_present {
                                literal("/dev/sda1")
                            } else {
                                missing()
                            }
                        );
                        assert_eq!(
                            attachment.instance_owning_service,
                            if owning {
                                literal("service.example")
                            } else {
                                missing()
                            }
                        );
                        assert_eq!(
                            attachment.associated_resource,
                            if associated {
                                literal(
                                    "arn:aws:service:eu-central-1:111111111111:resource/example",
                                )
                            } else {
                                missing()
                            }
                        );
                        assert_eq!(attachment.delete_on_termination, present(false));
                        assert_eq!(attachment.card_index, present((-1).into()));
                        assert_eq!(
                            attachment.attached_at_ns,
                            present(1_577_836_800_000_000_000)
                        );
                    } else {
                        panic!("wrong source");
                    }
                    assert_no_absence(&normalized);
                }
            }
        }
    }
}

#[tokio::test]
async fn instance_ebs_source_preserves_its_own_metadata_and_missing_enclosing_object() {
    let (sdk, _) = client(&instances_xml(
        "<instanceId>i-01</instanceId><blockDeviceMapping><item><deviceName>/dev/sda1</deviceName><ebs><volumeId>vol-01</volumeId><associatedResource>resource.example</associatedResource><volumeOwnerId>222222222222</volumeOwnerId><ebsCardIndex>-1</ebsCardIndex><deleteOnTermination>false</deleteOnTermination><status>attached</status><attachTime>2020-01-01T00:00:00Z</attachTime><operator><managed>true</managed></operator></ebs></item><item/><item><ebs/></item></blockDeviceMapping>",
    ));
    let out = sdk.describe_instances().send().await.unwrap();
    let instance = &out.reservations()[0].instances()[0];
    let mappings: Vec<_> = instance
        .block_device_mappings()
        .iter()
        .map(|m| instance_ebs(instance, m).unwrap())
        .collect();
    if let AttachmentV2::InstanceEbsMapping {
        instance,
        device,
        ebs: ObservationValueV2::Present(ebs),
    } = &mappings[0]
    {
        assert_eq!(*instance, present("i-01".parse().unwrap()));
        assert_eq!(*device, literal("/dev/sda1"));
        assert_eq!(ebs.volume_owner, present("222222222222".parse().unwrap()));
        assert_eq!(ebs.associated_resource, literal("resource.example"));
        assert_eq!(ebs.card_index, present((-1).into()));
        assert_eq!(ebs.delete_on_termination, present(false));
        assert_eq!(ebs.attached_at_ns, present(1_577_836_800_000_000_000));
    } else {
        panic!("wrong source");
    }
    if let AttachmentV2::InstanceEbsMapping { device, ebs, .. } = &mappings[1] {
        assert_eq!(*device, missing());
        assert_eq!(*ebs, missing());
    } else {
        panic!("wrong source");
    }
    if let AttachmentV2::InstanceEbsMapping {
        ebs: ObservationValueV2::Present(ebs),
        ..
    } = &mappings[2]
    {
        assert_eq!(ebs.operator, missing());
        assert_eq!(ebs.volume, missing());
    } else {
        panic!("empty EBS must stay present");
    }
    assert_no_absence(&mappings);
    let serialized = serde_json::to_value(&mappings[0]).unwrap();
    assert!(
        serialized["ebs"]["value"]
            .get("instance_owning_service")
            .is_none()
    );
}

#[tokio::test]
async fn malformed_booleans_are_protocol_failures_and_identifiers_are_never_fabricated() {
    let (sdk, replay) = client(&volumes_xml(
        "<operator><managed>not-a-bool</managed></operator>",
    ));
    assert!(sdk.describe_volumes().send().await.is_err());
    assert_eq!(replay.actual_requests().count(), 1);
    let (sdk, _) = client(&volumes_xml(
        "<attachmentSet><item><instanceId/><volumeId>invalid</volumeId></item></attachmentSet>",
    ));
    let out = sdk.describe_volumes().send().await.unwrap();
    let v = &out.volumes()[0];
    let normalized = volume_attachment(v, &v.attachments()[0]).unwrap();
    if let AttachmentV2::VolumeAttachment {
        described_volume,
        attachment,
    } = &normalized
    {
        assert_eq!(*described_volume, missing());
        let malformed =
            UnavailableEvidenceV2::Read(crate::provider::coverage::ReadFailureV1::Malformed);
        assert_eq!(
            attachment.instance,
            ObservationValueV2::Unavailable(malformed)
        );
        assert_eq!(
            attachment.volume,
            ObservationValueV2::Unavailable(malformed)
        );
    } else {
        panic!("wrong source");
    }
    assert_no_absence(&normalized);
}

#[tokio::test]
async fn bounded_text_and_unknown_attachment_states_are_not_normalized_away() {
    for length in [2048, 2049] {
        let body = volumes_xml(&format!(
            "<operator><principal>{}</principal></operator>",
            "x".repeat(length)
        ));
        let (sdk, _) = client(&body);
        let out = sdk.describe_volumes().send().await.unwrap();
        let result = volume_operator(&out.volumes()[0]);
        assert_eq!(result.is_ok(), length == 2048);
        if let Ok(ObservationValueV2::Present(op)) = result {
            assert_eq!(op.principal, literal(&"x".repeat(length)));
            assert_eq!(op.managed, missing());
        }
    }
    let (sdk, _) = client(&volumes_xml(
        "<attachmentSet><item><status>future-state</status><device>/dev/sda1</device></item></attachmentSet>",
    ));
    let out = sdk.describe_volumes().send().await.unwrap();
    let v = &out.volumes()[0];
    let result = volume_attachment(v, &v.attachments()[0]).unwrap();
    if let AttachmentV2::VolumeAttachment { attachment, .. } = &result {
        assert_eq!(attachment.state, literal("future-state"));
        assert_eq!(attachment.instance, missing());
    } else {
        panic!("wrong source");
    }
    assert_no_absence(&result);
}

#[tokio::test]
async fn nested_sources_do_not_borrow_the_parent_operator() {
    let (sdk, _) = client(&instances_xml(
        "<instanceId>i-01</instanceId><operator><managed>false</managed><principal>parent.example</principal></operator><networkInterfaceSet><item><networkInterfaceId>eni-01</networkInterfaceId><operator><managed>true</managed><hiddenByDefault>false</hiddenByDefault></operator></item></networkInterfaceSet><blockDeviceMapping><item><ebs><volumeId>vol-01</volumeId></ebs></item></blockDeviceMapping>",
    ));
    let out = sdk.describe_instances().send().await.unwrap();
    let instance = &out.reservations()[0].instances()[0];
    assert_eq!(
        instance_operator(instance).unwrap(),
        present(OperatorEvidenceV2 {
            managed: present(false),
            principal: literal("parent.example"),
            hidden_by_default: missing(),
        })
    );
    let child = instance_interface(instance, &instance.network_interfaces()[0]).unwrap();
    assert_eq!(
        child.1,
        present(OperatorEvidenceV2 {
            managed: present(true),
            principal: missing(),
            hidden_by_default: present(false),
        })
    );
    let ebs = instance_ebs(instance, &instance.block_device_mappings()[0]).unwrap();
    if let AttachmentV2::InstanceEbsMapping {
        ebs: ObservationValueV2::Present(ebs),
        ..
    } = &ebs
    {
        assert_eq!(ebs.operator, missing());
    } else {
        panic!("wrong source");
    }
    assert_no_absence(&child);
    assert_no_absence(&ebs);
}

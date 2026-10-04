use super::{
    tests::{check, data, parameters, xml},
    *,
};
use crate::aws::{ec2_infrastructure_reads_tests::reader, tests::response};
use crate::{
    canonical,
    provider::{
        allocation_value_v5::*,
        ec2_allocation_observation_v5::{ObservationDataV5 as D, TextAttributeV5},
        management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2 as U},
        observation_v5::*,
    },
};
use base64::{Engine, engine::general_purpose::STANDARD};
const OP: ReadOperationV1 = ReadOperationV1::DescribeInstanceAttribute;
fn selected(attribute: InstanceAttribute) -> AllocationRead {
    AllocationRead::InstanceAttribute {
        instance: "i-aaaaaaaa".parse().unwrap(),
        attribute,
    }
}
fn payload(encoded: &str) -> String {
    xml(
        OP,
        &format!(
            "<instanceId>i-bbbbbbbb</instanceId><userData><value>{encoded}</value></userData>"
        ),
    )
}
fn bytes(r: &QueryResult) -> &[u8] {
    let D::InstanceAttributes(v) = data(&r.records[0]) else {
        panic!()
    };
    let V::Present(v) = &v.user_data else {
        panic!()
    };
    let UserDataValueV5::Present(v) = &v.value else {
        panic!()
    };
    v.as_bytes()
}
#[tokio::test]
async fn retained_collector_payload_is_recovered_without_rewriting_or_losing_lf() {
    let d = crate::test_support::launch_documents();
    d.approval.validate(&d.deployment, &d.trust).unwrap();
    d.specification
        .validate(&d.deployment, &d.approval, &d.trust)
        .unwrap();
    let expected = include_bytes!("../../tests/fixtures/collector-config-v2.json");
    let config: crate::collector_config::CollectorConfigV2 = canonical::decode(expected).unwrap();
    assert_eq!(config.user_data_bytes().unwrap(), expected);
    assert_eq!(d.approval.launch.user_data.as_bytes(), expected);
    assert!(expected.ends_with(b"\n"));
    assert_eq!(expected.len(), 282);
    let (mut session, _, _) = reader(vec![response(
        200,
        &payload(&STANDARD.encode(expected)),
        None,
    )]);
    let r = session
        .allocation(selected(InstanceAttribute::UserData), true)
        .await
        .unwrap();
    check(&r, 1, true, CoverageStatus::Complete);
    assert_eq!(bytes(&r), expected);
    assert_eq!(canonical::sha256(bytes(&r)), canonical::sha256(expected));
}
#[tokio::test]
async fn binary_empty_non_utf8_nul_trailing_bytes_and_double_decode_sentinel() {
    for original in [
        vec![],
        vec![0],
        vec![255, 254, 128, 0, 13, 10, 10],
        b"payload\n".to_vec(),
        b"TQ==".to_vec(),
        vec![0; 2049],
        vec![255; 8192],
    ] {
        let (mut session, _, _) = reader(vec![response(
            200,
            &payload(&STANDARD.encode(&original)),
            None,
        )]);
        let r = session
            .allocation(selected(InstanceAttribute::UserData), true)
            .await
            .unwrap();
        check(&r, 1, true, CoverageStatus::Complete);
        assert_eq!(bytes(&r), original);
    }
}
#[tokio::test]
async fn malformed_base64_and_separate_encoded_decoded_bounds_keep_identity() {
    for (encoded, expected, reason) in [
        (
            "!".to_owned(),
            UserDataValueV5::MalformedBase64,
            ReadFailureV1::Malformed,
        ),
        (
            "TQ".to_owned(),
            UserDataValueV5::MalformedBase64,
            ReadFailureV1::Malformed,
        ),
        (
            "TR==".to_owned(),
            UserDataValueV5::MalformedBase64,
            ReadFailureV1::Malformed,
        ),
        (
            STANDARD.encode(vec![0; 8193]),
            UserDataValueV5::DecodedLimit,
            ReadFailureV1::Limit(crate::provider::limits::LimitKind::RecordBytes),
        ),
        (
            STANDARD.encode(vec![0; 8194]),
            UserDataValueV5::EncodedLimit,
            ReadFailureV1::Limit(crate::provider::limits::LimitKind::RecordBytes),
        ),
    ] {
        let (mut session, transport, round) = reader(vec![response(200, &payload(&encoded), None)]);
        let r = session
            .allocation(selected(InstanceAttribute::UserData), true)
            .await
            .unwrap();
        check(&r, 1, true, CoverageStatus::Incomplete(reason));
        let D::InstanceAttributes(v) = data(&r.records[0]) else {
            panic!()
        };
        assert!(matches!(&v.id,MemberV5::Present(id) if id.as_str()=="i-bbbbbbbb"));
        let V::Present(v) = &v.user_data else {
            panic!()
        };
        assert_eq!(v.value, expected);
        assert_eq!(transport.actual_requests().count(), 1);
        assert_eq!(
            round.failure().is_some(),
            matches!(reason, ReadFailureV1::Limit(_))
        );
    }
}
#[tokio::test]
async fn every_selected_attribute_has_exact_request_and_missing_object_value_dispositions() {
    for (attribute, wire, value) in [
        (InstanceAttribute::UserData, "userData", ""),
        (
            InstanceAttribute::InstanceInitiatedShutdownBehavior,
            "instanceInitiatedShutdownBehavior",
            "future-behavior",
        ),
        (
            InstanceAttribute::DisableApiTermination,
            "disableApiTermination",
            "false",
        ),
        (InstanceAttribute::DisableApiStop, "disableApiStop", "true"),
    ] {
        for selected_xml in [
            String::new(),
            format!("<{wire}/>"),
            format!("<{wire}><value>{value}</value></{wire}>"),
        ] {
            let present = selected_xml.contains("<value>");
            // This independently returned sibling is preserved when the selected object/value is missing.
            let sibling = if wire == "disableApiStop" {
                "<disableApiTermination><value>true</value></disableApiTermination>"
            } else {
                "<disableApiStop><value>false</value></disableApiStop>"
            };
            let body = xml(
                OP,
                &format!("<instanceId>i-bbbbbbbb</instanceId>{selected_xml}{sibling}"),
            );
            let (mut session, transport, _) = reader(vec![response(200, &body, None)]);
            let r = session.allocation(selected(attribute), true).await.unwrap();
            check(
                &r,
                1,
                true,
                if present {
                    CoverageStatus::Complete
                } else {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                },
            );
            let mut expected = vec![format!("Attribute={wire}"), "InstanceId=i-aaaaaaaa".into()];
            expected.sort();
            assert_eq!(
                parameters(transport.actual_requests().next().unwrap()),
                expected
            );
            let D::InstanceAttributes(v) = data(&r.records[0]) else {
                panic!()
            };
            assert!(matches!(&v.id,MemberV5::Present(id) if id.as_str()=="i-bbbbbbbb"));
            if wire == "disableApiStop" {
                assert!(matches!(v.disable_api_termination, V::Present(_)));
            } else {
                assert!(matches!(v.disable_api_stop, V::Present(_)));
            }
            if wire == "userData" && selected_xml.is_empty() {
                assert!(matches!(v.user_data, V::Unavailable(U::NotReturned)));
            }
        }
    }
}
#[tokio::test]
async fn singleton_integrity_or_boolean_decoder_failure_cannot_create_terminal_evidence() {
    for inside in [
        "<userData><value></value><value></value></userData>",
        "<disableApiStop><value>not-a-boolean</value></disableApiStop>",
        "<userData><value>QQ==</value>",
    ] {
        let (mut session, _, _) = reader(vec![response(200, &xml(OP, inside), None)]);
        let r = session
            .allocation(selected(InstanceAttribute::UserData), true)
            .await
            .unwrap();
        check(
            &r,
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert!(r.records.is_empty());
    }
}
#[tokio::test]
async fn canonical_record_bound_counts_complete_overhead_and_is_not_a_decoded_byte_limit() {
    use crate::provider::limits::{LimitKind, RECORD_BYTES};
    let encoded = STANDARD.encode(vec![0; 8192]);
    assert_eq!(encoded.len(), 10924);
    let (mut session, _, _) = reader(vec![response(200, &payload(&encoded), None)]);
    let r = session
        .allocation(selected(InstanceAttribute::UserData), true)
        .await
        .unwrap();
    let ObservationEntryV5::V5(base) = &r.records[0] else {
        panic!()
    };
    let mut record = (**base).clone();
    let D::InstanceAttributes(v) = &mut record.data else {
        panic!()
    };
    v.shutdown_behavior = V::Present(TextAttributeV5 {
        value: MemberV5::Present("x".to_owned().try_into().unwrap()),
    });
    let overhead = canonical::encode(&record).unwrap().len() - 1;
    let padding = RECORD_BYTES - overhead;
    let text = format!("{}{}", "\n".repeat(padding / 6), "x".repeat(padding % 6));
    assert!(text.len() <= 2048);
    for extra in ["", "x"] {
        let body=payload(&encoded).replace("</DescribeInstanceAttributeResponse>",&format!("<instanceInitiatedShutdownBehavior><value>{text}{extra}</value></instanceInitiatedShutdownBehavior></DescribeInstanceAttributeResponse>"));
        let (mut session, _, round) = reader(vec![response(200, &body, None)]);
        let r = session
            .allocation(selected(InstanceAttribute::UserData), true)
            .await
            .unwrap();
        if extra.is_empty() {
            check(&r, 1, true, CoverageStatus::Complete);
            assert_eq!(r.records[0].canonical_bytes().unwrap().len(), RECORD_BYTES);
        } else {
            check(
                &r,
                1,
                true,
                CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
            );
            assert!(r.records.is_empty());
            assert_eq!(r.coverage.records, 1);
            assert_eq!(round.failure(), Some(LimitKind::RecordBytes));
        }
    }
}

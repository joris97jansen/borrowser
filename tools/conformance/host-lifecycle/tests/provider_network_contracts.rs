use Observed::{Absent, Present};
use borrowser_host_lifecycle::{
    canonical,
    provider::{
        coverage::ReadOperationV1,
        limits::{ObservationAccounting, RECORD_BYTES},
        manifest::{Effect, EndpointPolicy, Ipv4Cidr, ReviewedRoute, SecurityRule},
        network_observation::*,
        observation::*,
    },
};

fn text(v: &str) -> ProviderText {
    v.to_owned().try_into().unwrap()
}
fn list<T>(v: Vec<T>) -> EvidenceList<T> {
    v.try_into().unwrap()
}
fn record(data: ObservationDataV1, operation: ReadOperationV1) -> ObservationRecordV1 {
    let mut r = ObservationRecordV1::parse(include_bytes!(
        "fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    r.data = data;
    r.query.operation = operation;
    r
}
fn roundtrip(r: &ObservationRecordV1) {
    assert_eq!(
        ObservationRecordV1::parse(&r.canonical_bytes().unwrap()).unwrap(),
        *r
    );
}
fn route_record() -> ObservationRecordV1 {
    ObservationRecordV1::parse(include_bytes!(
        "fixtures/provider-foundation-v1/route-observation.json"
    ))
    .unwrap()
}

#[test]
fn route_facts_are_independent_of_reviewed_routes_and_identity_binds_each_fact() {
    let original = route_record();
    assert_eq!(
        original.identity().unwrap().sha256.as_str(),
        include_str!("fixtures/provider-foundation-v1/route-observation.sha256").trim()
    );
    roundtrip(&original); // default route to NAT, blackhole, propagated, disassociated
    for change in 0..5 {
        let mut r = original.clone();
        if let ObservationDataV1::RouteTable {
            routes: Present(routes),
            ..
        } = &mut r.data
        {
            let mut route = routes.as_slice()[0].clone();
            match change {
                0 => route.state = Present(RouteStateObservation::Active),
                1 => route.origin = Present(RouteOriginObservation::CreateRoute),
                2 => {
                    route.targets =
                        Present(list(vec![RouteTargetObservation::Gateway(text("igw-01"))]))
                }
                3 => {
                    route.destinations = Present(list(vec![RouteDestinationObservation::Ipv6(
                        "::/0".to_owned().try_into().unwrap(),
                    )]))
                }
                _ => {
                    route.state = Present(RouteStateObservation::Unrecognized(text("future-state")))
                }
            }
            *routes = list(vec![route]);
        } else {
            panic!("route fixture");
        }
        roundtrip(&r);
        assert_ne!(r.identity().unwrap(), original.identity().unwrap());
    }
    let reviewed = ReviewedRoute::Local {
        cidr: Ipv4Cidr {
            network: 0,
            prefix: 0,
        },
    };
    assert!(
        serde_json::from_slice::<RouteObservation>(&canonical::encode(&reviewed).unwrap()).is_err()
    );
    if let ObservationDataV1::RouteTable {
        routes: Present(routes),
        ..
    } = original.data
    {
        assert!(
            serde_json::from_slice::<ReviewedRoute>(
                &canonical::encode(&routes.as_slice()[0]).unwrap()
            )
            .is_err()
        );
    }
    assert!(
        serde_json::from_str::<RouteTargetObservation>(
            r#"{"kind":"other-facts","value":{"allowed":true}}"#
        )
        .is_err()
    );
}

#[test]
fn sg_and_nacl_retain_foreign_ipv6_arbitrary_protocol_and_contradictory_ranges() {
    let rule = SecurityGroupRuleObservation {
        protocol: Present(text("132")),
        from_port: Present(9000.into()),
        to_port: Present(1.into()),
        peers: Present(list(vec![
            SecurityGroupPeerObservation::Ipv6 {
                cidr: Present("::/0".to_owned().try_into().unwrap()),
                description: Absent,
            },
            SecurityGroupPeerObservation::Group {
                account: Present("999999999999".parse().unwrap()),
                vpc: Present("vpc-ff".parse().unwrap()),
                group: Present("sg-ff".parse().unwrap()),
                name: Absent,
                description: Absent,
                peering_connection: Present(text("pcx-01")),
                peering_status: Present(text("deleted")),
            },
        ])),
    };
    assert!(serde_json::from_slice::<SecurityRule>(&canonical::encode(&rule).unwrap()).is_err());
    roundtrip(&record(
        ObservationDataV1::SecurityGroup {
            id: "sg-01".parse().unwrap(),
            owner: Absent,
            vpc: Absent,
            ingress: Present(list(vec![rule])),
            egress: Absent,
        },
        ReadOperationV1::DescribeSecurityGroups,
    ));
    let entry = NaclEntryObservation {
        number: Present(32767.into()),
        egress: Present(false),
        action: Present(RuleActionObservation::Known(Effect::Allow)),
        ipv4: Present(Ipv4Cidr {
            network: 0,
            prefix: 0,
        }),
        ipv6: Present("::/0".to_owned().try_into().unwrap()),
        protocol: Present(text("future-protocol")),
        ports: Present(PortRangeObservation {
            from: Present(9000.into()),
            to: Present(1.into()),
        }),
        icmp: Present(IcmpObservation {
            icmp_type: Present((-1).into()),
            code: Present(7.into()),
        }),
    };
    roundtrip(&record(
        ObservationDataV1::Nacl {
            id: "acl-01".parse().unwrap(),
            owner: Absent,
            vpc: Absent,
            subnets: Absent,
            entries: Present(list(vec![entry.clone(), entry])), // no sorting/deduplication/default-deny substitution
        },
        ReadOperationV1::DescribeNetworkAcls,
    ));
}

fn policy() -> EndpointPolicyObservation {
    EndpointPolicyObservation {
        version: Present(text("2012-10-17")),
        id: Absent,
        statements: Present(list(vec![PolicyStatementObservation {
            sid: Absent,
            effect: Present(RuleActionObservation::Known(Effect::Allow)),
            principal: Present(PolicyPrincipalsObservation::Any),
            not_principal: Present(PolicyPrincipalsObservation::Entries(list(vec![
                PolicyPrincipalObservation {
                    kind: PolicyPrincipalKind::Service,
                    identities: list(vec![text("ec2.amazonaws.com")]),
                },
            ]))),
            actions: Present(list(vec![text("s3:*")])),
            not_actions: Present(list(vec![text("s3:DeleteObject")])),
            resources: Present(list(vec![text("arn:aws:s3:::foreign-bucket/*")])),
            not_resources: Present(list(vec![text(
                "arn:aws:s3:::foreign-bucket/private/${aws:username}/*",
            )])),
            conditions: Present(list(vec![PolicyConditionObservation {
                operator: text("StringNotEqualsIfExists"),
                key: text("aws:SourceVpc"),
                values: list(vec![PolicyConditionLiteral::Text(text("vpc-ff"))]),
            }])),
        }])),
        unsupported: list(vec![UnsupportedPolicyStructure::ConditionValue {
            statement: 0,
            condition: 0,
            shape: PolicyValueShape::Object,
        }]),
    }
}
fn policy_record(policy: EndpointPolicyObservation) -> ObservationRecordV1 {
    record(
        ObservationDataV1::Endpoint {
            id: "vpce-01".parse().unwrap(),
            owner: Absent,
            vpc: Absent,
            service: Absent,
            endpoint_type: Absent,
            state: Absent,
            route_tables: Absent,
            policy: Present(policy),
        },
        ReadOperationV1::DescribeVpcEndpoints,
    )
}
#[test]
fn policy_structure_preserves_literals_negation_and_unsupported_shapes_without_evaluation() {
    let p = policy();
    assert!(serde_json::from_slice::<EndpointPolicy>(&canonical::encode(&p).unwrap()).is_err());
    let r = policy_record(p.clone());
    roundtrip(&r);
    let mut different = p;
    different.unsupported = list(vec![]);
    assert_ne!(
        policy_record(different).identity().unwrap(),
        r.identity().unwrap()
    );
    assert!(
        serde_json::from_str::<PolicyConditionLiteral>(
            r#"{"kind":"text","value":{"arbitrary":"map"}}"#
        )
        .is_err()
    );
}

#[test]
fn dhcp_custom_and_unknown_options_remain_evidence_and_addresses_are_semantic() {
    roundtrip(&record(
        ObservationDataV1::PrefixList {
            id: "pl-01".parse().unwrap(),
            name: Present(text("foreign-ipv6-service")),
            cidrs: Present(list(vec![IpCidrObservation::Ipv6(
                "::/0".to_owned().try_into().unwrap(),
            )])),
        },
        ReadOperationV1::DescribePrefixLists,
    ));
    let options = vec![
        DhcpOptionObservation {
            key: Present(DhcpOptionKey::DomainNameServers),
            values: Present(list(vec![text("8.8.8.8"), text("2001:db8::1")])),
        },
        DhcpOptionObservation {
            key: Present(DhcpOptionKey::DomainName),
            values: Present(list(vec![text("CUSTOM.Example")])),
        },
        DhcpOptionObservation {
            key: Present(DhcpOptionKey::Unrecognized(text("future-dhcp-option"))),
            values: Present(list(vec![text("explicit-value")])),
        },
    ];
    roundtrip(&record(
        ObservationDataV1::Dhcp {
            id: "dopt-01".parse().unwrap(),
            owner: Absent,
            configuration: Present(list(options)),
        },
        ReadOperationV1::DescribeDhcpOptions,
    ));
    let address = Address {
        address: "203.0.113.7".parse().unwrap(),
        primary: Present(true),
    };
    assert_eq!(
        canonical::decode::<Address>(&canonical::encode(&address).unwrap()).unwrap(),
        address
    );
    assert!(
        serde_json::from_str::<Address>(
            r#"{"address":"not-an-address","primary":{"kind":"absent"}}"#
        )
        .is_err()
    );
    for v in ["2001:db8::1/64", "::/129", "2001:0db8::/32"] {
        assert!(Ipv6Cidr::try_from(v.to_owned()).is_err());
    }
    for n in [i32::MIN, -1, 0, i32::MAX] {
        let v = ProviderI32::from(n);
        assert_eq!(
            canonical::decode::<ProviderI32>(&canonical::encode(&v).unwrap())
                .unwrap()
                .value(),
            n
        );
    }
    for v in ["-0", "+1", "01", "2147483648"] {
        assert!(ProviderI32::try_from(v.to_owned()).is_err());
    }
}

#[test]
fn observation_record_and_aggregate_limits_still_apply_to_provider_facts() {
    let mut values = vec![text(&"x".repeat(2048)); 7];
    values.push(text(""));
    let make = |values| {
        record(
            ObservationDataV1::Dhcp {
                id: "dopt-01".parse().unwrap(),
                owner: Absent,
                configuration: Present(list(vec![DhcpOptionObservation {
                    key: Present(DhcpOptionKey::Unrecognized(text("future-option"))),
                    values: Present(list(values)),
                }])),
            },
            ReadOperationV1::DescribeDhcpOptions,
        )
    };
    let base = make(values.clone()).canonical_bytes().unwrap().len();
    values[7] = text(&"x".repeat(RECORD_BYTES - base));
    let exact = make(values.clone());
    assert_eq!(exact.canonical_bytes().unwrap().len(), RECORD_BYTES);
    roundtrip(&exact);
    let mut accounting = ObservationAccounting::default();
    for _ in 0..16 {
        accounting.canonical_record(&exact).unwrap();
    }
    assert!(accounting.canonical_record(&exact).is_err());
    values[7] = text(&"x".repeat(RECORD_BYTES - base + 1));
    assert!(make(values).canonical_bytes().is_err());
    assert!(
        EvidenceList::<RouteObservation>::try_from(vec![
            RouteObservation {
                destinations: Absent,
                targets: Absent,
                instance_owner: Absent,
                state: Absent,
                origin: Absent
            };
            129
        ])
        .is_err()
    );
}

#[test]
fn endpoint_policy_exact_byte_and_collection_bounds_remain_enforced() {
    let mut p = EndpointPolicyObservation {
        version: Absent,
        id: Present(text("")),
        statements: Absent,
        unsupported: list(vec![
            UnsupportedPolicyStructure::DocumentMember {
                name: text(&"x".repeat(2048))
            };
            3
        ]),
    };
    let base = canonical::encode(&p).unwrap().len();
    p.id = Present(text(&"x".repeat(8192 - base)));
    assert_eq!(canonical::encode(&p).unwrap().len(), 8192);
    roundtrip(&policy_record(p.clone()));
    p.id = Present(text(&"x".repeat(8192 - base + 1)));
    assert!(policy_record(p).canonical_bytes().is_err());
    let mut p = policy();
    if let Present(s) = &p.statements {
        let mut minimal = s.as_slice()[0].clone();
        minimal.principal = Absent;
        minimal.not_principal = Absent;
        minimal.actions = Absent;
        minimal.not_actions = Absent;
        minimal.resources = Absent;
        minimal.not_resources = Absent;
        minimal.conditions = Absent;
        p.statements = Present(list(vec![minimal.clone(); 16]));
        roundtrip(&policy_record(p.clone()));
        p.statements = Present(list(vec![minimal; 17]));
        assert!(policy_record(p).canonical_bytes().is_err());
    }
}

#[test]
fn policy_polarities_share_limits_and_never_hide_excess_elements() {
    for kind in 0..4 {
        let mut p = policy();
        let Present(statements) = &p.statements else {
            panic!("policy fixture");
        };
        let mut s = statements.as_slice()[0].clone();
        s.not_actions = Absent;
        s.not_resources = Absent;
        s.not_principal = Absent;
        match kind {
            0 => s.actions = Present(list(vec![text("s3:*"); 8])),
            1 => s.resources = Present(list(vec![text("*"); 16])),
            2 => {
                s.conditions = Present(list(vec![
                    PolicyConditionObservation {
                        operator: text("Bool"),
                        key: text("aws:SecureTransport"),
                        values: list(vec![PolicyConditionLiteral::Boolean(false)]),
                    };
                    8
                ]))
            }
            _ => {
                s.principal = Present(PolicyPrincipalsObservation::Entries(list(vec![
                    PolicyPrincipalObservation {
                        kind: PolicyPrincipalKind::Aws,
                        identities: list(vec![text("*"); 16]),
                    },
                ])))
            }
        }
        p.statements = Present(list(vec![s.clone()]));
        roundtrip(&policy_record(p.clone()));
        match kind {
            0 => s.not_actions = Present(list(vec![text("s3:GetObject")])),
            1 => s.not_resources = Present(list(vec![text("*")])),
            2 => {
                let Present(c) = &s.conditions else {
                    panic!("conditions");
                };
                s.conditions = Present(list(vec![c.as_slice()[0].clone(); 9]));
            }
            _ => {
                s.not_principal = Present(PolicyPrincipalsObservation::Entries(list(vec![
                    PolicyPrincipalObservation {
                        kind: PolicyPrincipalKind::Service,
                        identities: list(vec![text("ec2.amazonaws.com")]),
                    },
                ])))
            }
        }
        p.statements = Present(list(vec![s]));
        assert!(policy_record(p).canonical_bytes().is_err());
    }
}

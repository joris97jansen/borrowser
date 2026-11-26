use super::*;
use addressing::{
    ResourceCapability as Capability, ResourceOperation, UrlFailureKind, UrlParseError,
};
use core_types::{DomHandle, DomVersion};

struct Fixture {
    document: Node,
    base: BrowserUrl,
}
impl Fixture {
    fn new(markup: &str) -> Self {
        Self::with_attribute_limit(markup, 16 * 1024)
    }
    fn with_attribute_limit(markup: &str, maximum: usize) -> Self {
        // Resource fixtures may use the parser's existing configurable limit.
        // No production parser or AJ6 admission policy is changed.
        let mut options = html::HtmlParseOptions::default();
        options.tokenizer.limits.max_attribute_value_bytes = maximum;
        let parsed = html::parse_document(format!("<!doctype html>{markup}"), options).unwrap();
        let mut store = crate::dom_store::DomStore::new();
        let handle = DomHandle(1);
        store.create(handle).unwrap();
        store
            .apply(
                handle,
                DomVersion::INITIAL,
                DomVersion::INITIAL.next(),
                &parsed.patches,
            )
            .unwrap();
        Self {
            document: store.materialize_owned(handle).unwrap(),
            base: BrowserUrl::parse("https://example.test/dir/page?q=1#doc")
                .unwrap()
                .url,
        }
    }
    fn discover(&self) -> StylesheetDiscovery<'_> {
        self.with_limits(&Default::default()).unwrap()
    }
    fn with_limits(
        &self,
        limits: &StylesheetDiscoveryLimits,
    ) -> Result<StylesheetDiscovery<'_>, StylesheetDiscoveryError> {
        discover_stylesheets(&self.document, &self.base, limits)
    }
}
fn link<'a, 'dom>(record: &'a StylesheetOccurrence<'dom>) -> &'a LinkDiscovery<'dom> {
    let StylesheetSource::Link(link) = record.source() else {
        panic!("expected link")
    };
    link
}
fn block<'a, 'dom>(record: &'a StylesheetOccurrence<'dom>) -> &'a StyleBlockDiscovery<'dom> {
    let StylesheetSource::StyleBlock(block) = record.source() else {
        panic!("expected block")
    };
    block
}
fn address<'a>(record: &'a StylesheetOccurrence<'_>) -> &'a ResourceAddress {
    let StylesheetHrefOutcome::Resolved { address, .. } = link(record).href().outcome() else {
        panic!("expected resolved address")
    };
    address
}
fn error(
    result: Result<StylesheetDiscovery<'_>, StylesheetDiscoveryError>,
) -> StylesheetDiscoveryError {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected atomic failure"),
    }
}
fn limits_error(
    fixture: &Fixture,
    limits: StylesheetDiscoveryLimits,
    dimension: StylesheetDiscoveryLimit,
    maximum: usize,
) {
    assert_eq!(
        error(fixture.with_limits(&limits)),
        StylesheetDiscoveryError::LimitExceeded { dimension, maximum }
    );
}
fn element(
    id: u32,
    namespace: ElementNamespace,
    name: &str,
    attrs: &[(&str, &str)],
    children: Vec<Node>,
) -> Node {
    let mut names = html::AtomTable::new();
    html::internal::node_element_from_parts(
        Id(id),
        html::internal::expanded_name(namespace, name),
        attrs
            .iter()
            .map(|(k, v)| {
                let atom = names.intern_exact(k).unwrap();
                html::ParserCreatedAttribute::unqualified(&names, atom, (*v).into()).unwrap()
            })
            .collect(),
        Vec::new(),
        children,
    )
}
fn document(children: Vec<Node>) -> Node {
    Node::Document {
        id: Id(1),
        doctype: None,
        children,
    }
}

#[test]
fn aj6_one_external() {
    let fixture = Fixture::new("<link rel=stylesheet href=a.css>");
    let inventory = fixture.discover();
    assert_eq!(inventory.records().len(), 1);
    let record = &inventory.records()[0];
    assert_eq!(record.occurrence_index().get(), 0);
    assert_eq!(record.dependency_order().unwrap().get(), 0);
    assert_ne!(record.source_node(), Id::INVALID);
    assert_eq!(
        address(record).url().as_str(),
        "https://example.test/dir/a.css"
    );
    assert_eq!(address(record).kind(), ResourceKind::Stylesheet);
}
#[test]
fn aj6_multiple_external() {
    let fixture = Fixture::new("<link rel=stylesheet href=a.css><link rel=stylesheet href=b.css>");
    let inventory = fixture.discover();
    assert_eq!(
        inventory
            .dependencies()
            .map(|r| address(r).url().as_str())
            .collect::<Vec<_>>(),
        [
            "https://example.test/dir/a.css",
            "https://example.test/dir/b.css"
        ]
    );
    assert_eq!(
        inventory
            .dependencies()
            .map(|r| r.dependency_order().unwrap().get())
            .collect::<Vec<_>>(),
        [0, 1]
    );
}
#[test]
fn aj6_inline_blocks() {
    let fixture = Fixture::new("<style></style><style>p{color:red}</style>");
    let inventory = fixture.discover();
    assert_eq!(inventory.records().len(), 2);
    assert_eq!(
        block(&inventory.records()[0])
            .content()
            .text_chunks()
            .count(),
        0
    );
    assert_eq!(
        block(&inventory.records()[1])
            .content()
            .text_chunks()
            .collect::<String>(),
        "p{color:red}"
    );
    let split = document(vec![element(
        2,
        ElementNamespace::Html,
        "style",
        &[],
        vec![
            Node::Text {
                id: Id(3),
                text: "p{color:".into(),
            },
            Node::Comment {
                id: Id(4),
                text: "ignored".into(),
            },
            Node::Text {
                id: Id(5),
                text: "red}".into(),
            },
        ],
    )]);
    let inventory = discover_stylesheets(&split, &fixture.base, &Default::default()).unwrap();
    let chunks = block(&inventory.records()[0])
        .content()
        .text_chunks()
        .collect::<Vec<_>>();
    assert_eq!(chunks, ["p{color:", "red}"]);
    assert_eq!(chunks.concat(), "p{color:red}");
}
#[test]
fn aj6_mixed_dependency_order() {
    let fixture = Fixture::new(
        "<link rel=help href=unrelated style='x:y'><link rel=stylesheet><style type=other style='color:red'></style><link rel=stylesheet href='javascript:inert'><p style=''></p>",
    );
    let inventory = fixture.discover();
    assert_eq!(inventory.records().len(), 7);
    assert_eq!(
        inventory
            .records()
            .iter()
            .map(|r| r.dependency_order().map(StylesheetDependencyOrder::get))
            .collect::<Vec<_>>(),
        [None, None, Some(0), Some(1), None, Some(2), None]
    );
    assert_eq!(
        inventory
            .records()
            .iter()
            .map(|r| r.occurrence_index().get())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6]
    );
    assert_eq!(inventory.style_attributes().count(), 3);
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 1);
    assert!(matches!(
        link(&inventory.records()[0]).href().outcome(),
        StylesheetHrefOutcome::NotStylesheetRelationship
    ));
}
#[test]
fn aj6_source_identity_and_duplicates() {
    let fixture = Fixture::new(
        "<link rel=stylesheet href=same.css><link rel=stylesheet href=same.css><style>x{}</style><style>x{}</style>",
    );
    let inventory = fixture.discover();
    assert_eq!(inventory.records().len(), 4);
    let owners = inventory
        .records()
        .iter()
        .map(StylesheetOccurrence::source_node)
        .collect::<Vec<_>>();
    assert_eq!(
        owners
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        address(&inventory.records()[0]).url(),
        address(&inventory.records()[1]).url()
    );
    let again = fixture.discover();
    assert_eq!(
        again
            .records()
            .iter()
            .map(StylesheetOccurrence::source_node)
            .collect::<Vec<_>>(),
        owners
    );
}
#[test]
fn aj6_rel_tokens() {
    for separator in [' ', '\t', '\n', '\r', '\u{000c}'] {
        let fixture = Fixture::new(&format!(
            "<link rel='unknown{separator}StYlEsHeEt{separator}stylesheet{separator}alternate' href=a.css>"
        ));
        let inventory = fixture.discover();
        let source = link(&inventory.records()[0]);
        assert_eq!(source.relation().kind(), RelationKind::Stylesheet);
        assert_eq!(source.relation().alternate(), AlternateFact::Present);
        assert!(source.relation().other_tokens());
        assert_eq!(inventory.dependencies().count(), 1);
    }
    for rel in [
        "",
        " ",
        "style",
        "stylesheet-extra",
        "alternate\u{00a0}stylesheet",
        "alternate\u{000b}stylesheet",
    ] {
        let fixture = Fixture::new(&format!("<link rel='{rel}' href=a.css>"));
        let inventory = fixture.discover();
        assert_eq!(inventory.records().len(), 1);
        assert_eq!(inventory.dependencies().count(), 0);
        assert_eq!(WORK.with(|w| w.get().resolution_calls), 0);
    }
    let fixture = Fixture::new("<link href=a.css>");
    assert_eq!(
        link(&fixture.discover().records()[0]).relation().kind(),
        RelationKind::Missing
    );
}
#[test]
fn aj6_href_presence_and_whitespace() {
    let fixture = Fixture::new(
        "<link rel=stylesheet><link rel=stylesheet href><link rel=stylesheet href=' \t\n\u{000c}\r '><link rel=stylesheet href='&#160;a.css&#160;'>",
    );
    let inventory = fixture.discover();
    assert!(matches!(
        link(&inventory.records()[0]).href().outcome(),
        StylesheetHrefOutcome::Missing
    ));
    assert!(matches!(
        link(&inventory.records()[1]).href().outcome(),
        StylesheetHrefOutcome::Empty
    ));
    assert_eq!(
        address(&inventory.records()[2]).url().as_str(),
        "https://example.test/dir/page?q=1"
    );
    assert_eq!(
        link(&inventory.records()[2]).href().outcome().diagnostics(),
        [UrlDiagnostic::BoundaryControlOrSpaceIgnored]
    );
    assert_eq!(
        address(&inventory.records()[3]).url().as_str(),
        "https://example.test/dir/%C2%A0a.css%C2%A0"
    );
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 2);
}
#[test]
fn aj6_url_outcomes() {
    let fixture = Fixture::new(
        "<link rel=stylesheet href=' https://u:p@h.test:70000/'><link rel=stylesheet href='/a%zz\\b%xx'><link rel=stylesheet href=later.css>",
    );
    let inventory = fixture.discover();
    let StylesheetHrefOutcome::Failed(failure) = link(&inventory.records()[0]).href().outcome()
    else {
        panic!("typed failure")
    };
    assert_eq!(
        failure.kind,
        UrlFailureKind::InvalidInput(UrlParseError::InvalidPort)
    );
    assert_eq!(failure.base.as_ref(), Some(&fixture.base));
    assert_eq!(
        failure.diagnostics,
        [
            UrlDiagnostic::BoundaryControlOrSpaceIgnored,
            UrlDiagnostic::EmbeddedCredentials
        ]
    );
    assert_eq!(
        address(&inventory.records()[1]).url().as_str(),
        "https://example.test/a%zz/b%xx"
    );
    assert_eq!(
        link(&inventory.records()[1]).href().outcome().diagnostics(),
        [
            UrlDiagnostic::InvalidPercentEncoding,
            UrlDiagnostic::Backslash,
            UrlDiagnostic::InvalidPercentEncoding
        ]
    );
    assert_eq!(
        address(&inventory.records()[2]).url().as_str(),
        "https://example.test/dir/later.css"
    );
    // Test the actual production adapter, without fabricating backend OOM evidence.
    for kind in [
        UrlFailureKind::CapacityExceeded,
        UrlFailureKind::BackendFailure,
    ] {
        let outcome = resolution_outcome(Err(UrlFailure {
            kind,
            diagnostics: vec![UrlDiagnostic::Backslash, UrlDiagnostic::Backslash],
            base: Some(fixture.base.clone()),
        }));
        let StylesheetHrefOutcome::Failed(failure) = outcome else {
            panic!("failure preserved")
        };
        assert_eq!(failure.kind, kind);
        assert_eq!(
            failure.diagnostics,
            [UrlDiagnostic::Backslash, UrlDiagnostic::Backslash]
        );
        assert_eq!(failure.base.as_ref(), Some(&fixture.base));
    }
}
#[test]
fn aj6_retrieval_capabilities() {
    for (href, expected) in [
        ("https://other.test/a.css", Capability::Supported),
        ("http://other.test/a.css", Capability::Supported),
        ("file:///a.css", Capability::Supported),
        ("data:text/css,x", Capability::Deferred),
        ("blob:https://other.test/id", Capability::Deferred),
        ("ws://other.test/", Capability::Reserved),
        ("wss://other.test/", Capability::Reserved),
        ("about:blank", Capability::Unsupported),
        ("javascript:inert", Capability::Unsupported),
        ("ftp://other.test/a", Capability::Unsupported),
        ("unknown:opaque", Capability::Unsupported),
    ] {
        let fixture = Fixture::new(&format!("<link rel=stylesheet href='{href}'>"));
        let inventory = fixture.discover();
        assert_eq!(
            address(&inventory.records()[0]).capability(ResourceOperation::Retrieval),
            expected
        );
        assert_eq!(inventory.dependencies().count(), 1);
    }
}
#[test]
fn aj6_selection_and_media() {
    for (media, expected_dom, deferred) in [
        (None, None, false),
        (Some(""), Some(""), false),
        (Some(" \t\r\n\u{000c}"), Some(" \t\n\u{000c}"), false),
        (Some("all"), Some("all"), true),
        (Some("screen"), Some("screen"), true),
        (Some("\u{000b}"), Some("\u{000b}"), true),
        (Some("\u{00a0}"), Some("\u{00a0}"), true),
    ] {
        let attr = media.map_or(String::new(), |m| format!(" media='{m}'"));
        let fixture = Fixture::new(&format!(
            "<link rel=stylesheet href=a.css{attr}><style{attr}></style>"
        ));
        let inventory = fixture.discover();
        for fact in [
            link(&inventory.records()[0]).media(),
            block(&inventory.records()[1]).media(),
        ] {
            assert_eq!(fact.authored(), expected_dom);
            assert_eq!(matches!(fact, MediaFact::DeferredUnsupported(_)), deferred);
        }
    }
    for (title_attr, title_fact) in [
        ("", AlternateTitleFact::Missing),
        (" title=''", AlternateTitleFact::Empty),
        (" title=' '", AlternateTitleFact::Named),
    ] {
        let fixture = Fixture::new(&format!(
            "<link rel='alternate stylesheet' disabled=false{title_attr}>"
        ));
        let inventory = fixture.discover();
        let source = link(&inventory.records()[0]);
        assert_eq!(
            source.selection(),
            SetSelectionFact::AlternateSelectionUnsupported(title_fact)
        );
        assert_eq!(source.disabled().authored(), Some("false"));
        assert!(matches!(source.disabled(), DisabledFact::Present(_)));
    }
    let fixture = Fixture::new("<link rel=stylesheet title=' '><style title='named'></style>");
    let inventory = fixture.discover();
    assert_eq!(
        link(&inventory.records()[0]).selection(),
        SetSelectionFact::NamedSelectionUnsupported
    );
    assert_eq!(
        block(&inventory.records()[1]).selection(),
        SetSelectionFact::NamedSelectionUnsupported
    );
}
#[test]
fn aj6_combined_attributes() {
    let fixture = Fixture::new(
        "<link rel='stylesheet alternate unknown' href='http://[' type='other' title='named' disabled=false media=screen crossorigin='' integrity='private' referrerpolicy='private'><style disabled=false crossorigin='' integrity='private' referrerpolicy='private'></style><link rel=help media='' disabled crossorigin=''>",
    );
    let inventory = fixture.discover();
    let source = link(&inventory.records()[0]);
    assert!(matches!(
        source.href().outcome(),
        StylesheetHrefOutcome::Failed(_)
    ));
    assert!(matches!(
        source.type_hint(),
        LinkTypeFact::AdvisoryUnassessed("other")
    ));
    assert!(source.relation().other_tokens());
    assert!(matches!(source.disabled(), DisabledFact::Present("false")));
    assert!(matches!(
        source.media(),
        MediaFact::DeferredUnsupported("screen")
    ));
    assert!(matches!(
        source.policies().crossorigin(),
        PolicyFact::PresentUnenforced("")
    ));
    assert!(matches!(
        source.policies().integrity(),
        PolicyFact::PresentUnenforced("private")
    ));
    assert!(matches!(
        source.policies().referrerpolicy(),
        PolicyFact::PresentUnenforced("private")
    ));
    let raw = block(&inventory.records()[1]).inapplicable_link_attributes();
    assert_eq!(
        (
            raw.disabled(),
            raw.crossorigin(),
            raw.integrity(),
            raw.referrerpolicy()
        ),
        (Some("false"), Some(""), Some("private"), Some("private"))
    );
    let unrelated = link(&inventory.records()[2]);
    assert!(matches!(
        unrelated.media(),
        MediaFact::NotStylesheetRelationship(Some(""))
    ));
    assert!(matches!(
        unrelated.policies().crossorigin(),
        PolicyFact::NotAssessedForOtherRelationship(Some(""))
    ));
}
#[test]
fn aj6_type_contexts() {
    for (attr, kind) in [
        ("", 0),
        (" type=''", 1),
        (" type='TeXt/CsS'", 2),
        (" type='text/css;charset=utf-8'", 3),
        (" type=' text/css '", 3),
    ] {
        let fixture = Fixture::new(&format!("<link rel=stylesheet{attr}><style{attr}></style>"));
        let inventory = fixture.discover();
        let actual = match block(&inventory.records()[1]).type_fact() {
            StyleTypeFact::Missing => 0,
            StyleTypeFact::Empty => 1,
            StyleTypeFact::Css(_) => 2,
            StyleTypeFact::Other(_) => 3,
        };
        assert_eq!(actual, kind);
        assert_eq!(
            link(&inventory.records()[0]).type_hint().authored(),
            block(&inventory.records()[1]).type_fact().authored()
        );
        assert_eq!(inventory.dependencies().count(), 2);
    }
}
#[test]
fn aj6_af_attribute_eligibility() {
    let fixture = Fixture::new(
        "<template style='host'><p style='inert'></p><style>inert</style></template><svg style='svg'><style>foreign</style><foreignObject><p style='integrated'></p></foreignObject></svg><math style='math'></math><p style=''></p><p style='broken{'></p>",
    );
    let inventory = fixture.discover();
    assert_eq!(inventory.dependencies().count(), 0);
    assert_eq!(
        inventory
            .style_attributes()
            .map(|r| match r.source() {
                StylesheetSource::StyleAttribute { value } => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>(),
        ["host", "svg", "integrated", "math", "", "broken{"]
    );
    let fixture = Fixture {
        document: document(vec![element(
            2,
            ElementNamespace::Svg,
            "g",
            &[("STYLE", "wrong"), ("style", "first"), ("style", "second")],
            vec![],
        )]),
        base: fixture.base,
    };
    let inventory = fixture.discover();
    assert!(matches!(
        inventory.records()[0].source(),
        StylesheetSource::StyleAttribute { value: "first" }
    ));
}
#[test]
fn aj6_parsed_order_and_indices() {
    let fixture = Fixture::new("<table style='table'><div style='div'>x</div></table>");
    let inventory = fixture.discover();
    assert_eq!(
        inventory
            .style_attributes()
            .map(|r| match r.source() {
                StylesheetSource::StyleAttribute { value } => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>(),
        ["div", "table"]
    );
    assert_eq!(
        inventory
            .records()
            .iter()
            .map(|r| r.occurrence_index().get())
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn aj6_limits_are_atomic() {
    let fixture = Fixture {
        document: document(vec![
            element(
                2,
                ElementNamespace::Html,
                "link",
                &[("rel", "stylesheet"), ("href", "a.css")],
                vec![],
            ),
            element(3, ElementNamespace::Html, "p", &[("style", "x")], vec![]),
        ]),
        base: BrowserUrl::parse("https://e.test/").unwrap().url,
    };
    let pair = fixture.base.as_str().len() + 5;
    let exact = StylesheetDiscoveryLimits {
        nodes: 3,
        depth: 1,
        occurrences: 2,
        attributes: 3,
        classification_bytes: 10,
        resolution_pair_bytes: pair,
        resolution_total_bytes: pair,
    };
    assert_eq!(fixture.with_limits(&exact).unwrap().records().len(), 2);
    // The same independently counted input is below each relaxed boundary.
    for limits in [
        StylesheetDiscoveryLimits { nodes: 4, ..exact },
        StylesheetDiscoveryLimits { depth: 2, ..exact },
        StylesheetDiscoveryLimits {
            occurrences: 3,
            ..exact
        },
        StylesheetDiscoveryLimits {
            attributes: 4,
            ..exact
        },
        StylesheetDiscoveryLimits {
            classification_bytes: 11,
            ..exact
        },
        StylesheetDiscoveryLimits {
            resolution_pair_bytes: pair + 1,
            ..exact
        },
        StylesheetDiscoveryLimits {
            resolution_total_bytes: pair + 1,
            ..exact
        },
    ] {
        assert_eq!(fixture.with_limits(&limits).unwrap().records().len(), 2);
    }
    let admitted = || {
        WORK.with(|w| {
            let w = w.get();
            (
                w.nodes,
                w.attributes,
                w.classification_bytes,
                w.resolution_calls,
                w.records,
            )
        })
    };
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits { nodes: 2, ..exact },
        StylesheetDiscoveryLimit::OrdinaryNodes,
        2,
    );
    assert_eq!(admitted(), (2, 2, 10, 1, 1));
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits { depth: 0, ..exact },
        StylesheetDiscoveryLimit::OrdinaryDepth,
        0,
    );
    assert_eq!(admitted(), (1, 0, 0, 0, 0));
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits {
            occurrences: 1,
            ..exact
        },
        StylesheetDiscoveryLimit::Occurrences,
        1,
    );
    assert_eq!(admitted(), (3, 3, 10, 1, 1));
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits {
            attributes: 2,
            ..exact
        },
        StylesheetDiscoveryLimit::AttributeEntries,
        2,
    );
    assert_eq!(admitted(), (3, 2, 10, 1, 1));
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits {
            classification_bytes: 9,
            ..exact
        },
        StylesheetDiscoveryLimit::ClassificationInputBytes,
        9,
    );
    assert_eq!(admitted(), (2, 2, 0, 0, 0));
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits {
            resolution_pair_bytes: pair - 1,
            ..exact
        },
        StylesheetDiscoveryLimit::ResolutionPairInputBytes,
        pair - 1,
    );
    assert_eq!(admitted(), (2, 2, 10, 0, 0));
    assert_eq!(WORK.with(|w| w.get().resolution_bytes), 0);
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits {
            resolution_total_bytes: pair - 1,
            ..exact
        },
        StylesheetDiscoveryLimit::TotalResolutionInputBytes,
        pair - 1,
    );
    assert_eq!(admitted(), (2, 2, 10, 0, 0));
    assert_eq!(WORK.with(|w| w.get().resolution_bytes), 0);
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 0);
    let deny = StylesheetDiscoveryLimits {
        occurrences: 0,
        ..exact
    };
    assert!(matches!(
        fixture.with_limits(&deny),
        Err(StylesheetDiscoveryError::LimitExceeded { .. })
    ));
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 0);
    assert_eq!(
        admit(
            usize::MAX,
            1,
            usize::MAX,
            StylesheetDiscoveryLimit::AttributeEntries
        ),
        Err(StylesheetDiscoveryError::ArithmeticOverflow(
            StylesheetDiscoveryLimit::AttributeEntries
        ))
    );
    if usize::BITS > 32 {
        assert_eq!(
            OccurrenceIndex::from_count(u32::MAX as usize + 1),
            Err(StylesheetDiscoveryError::IndexOverflow(
                StylesheetIndexDomain::Occurrence
            ))
        );
        assert_eq!(
            StylesheetDependencyOrder::from_count(u32::MAX as usize + 1),
            Err(StylesheetDiscoveryError::IndexOverflow(
                StylesheetIndexDomain::Dependency
            ))
        );
    }
}

#[test]
fn aj6_formatter_totals_availability() {
    let format = |result| {
        debug::Snapshot::new()
            .unwrap()
            .finish(format_args!("controlled"), result)
    };
    let absent = format(Ok(None));
    assert!(absent.contains(
        "availability=unavailable semantic_complete=not-applicable formatting_complete=true"
    ));
    assert!(absent.contains("occurrences=0/not-applicable diagnostics=0/not-applicable"));
    assert!(absent.contains("diagnostics_omitted: not-applicable"));
    let empty = Fixture::new("<p>empty</p>");
    let output = format(Ok(Some(empty.discover())));
    assert!(
        output.contains("availability=available semantic_complete=true formatting_complete=true")
    );
    assert!(output.contains("occurrences=0/0 diagnostics=0/Some(0)"));
    assert!(output.contains("diagnostics_omitted: false"));
    let fixture = Fixture::new("<style>PRIVATE_FIRST</style><style>PRIVATE_SECOND</style>");
    let output = format(Ok(Some(fixture.discover())));
    assert!(output.contains("occurrences=2/2 diagnostics=0/Some(0)"));
    assert!(output.contains("semantic_complete=true formatting_complete=true"));
    for (maximum, admitted) in [(0, 0), (1, 1)] {
        let result = fixture.with_limits(&StylesheetDiscoveryLimits {
            occurrences: maximum,
            ..Default::default()
        });
        assert!(matches!(
            result,
            Err(StylesheetDiscoveryError::LimitExceeded {
                dimension: StylesheetDiscoveryLimit::Occurrences,
                ..
            })
        ));
        assert_eq!(WORK.with(|w| w.get().records), admitted);
        let output = format(result.map(Some));
        assert!(
            output.contains("availability=failed semantic_complete=false formatting_complete=true")
        );
        assert!(output.contains("occurrences=0/unavailable diagnostics=0/unavailable"));
        assert!(output.contains("diagnostics_omitted: unavailable"));
        assert!(!output.contains("occurrences=0/0"));
        assert!(!output.contains("occurrence["));
        assert!(!output.contains("PRIVATE_"));
        assert_eq!(
            WORK.with(|w| w.get().records),
            admitted,
            "formatting must not rescan"
        );
        assert!(output.len() <= 8192);
    }
}
#[test]
fn aj6_ah_costs_are_not_input_bytes() {
    let spaces = " ".repeat(22_000);
    let fixture = Fixture::with_attribute_limit(
        &format!("<link rel=stylesheet href='/{spaces}/'>"),
        64 * 1024,
    );
    let inventory = fixture.discover();
    let url = address(&inventory.records()[0]).url().as_str();
    assert_eq!(
        url,
        format!("https://example.test/{}/", "%20".repeat(22_000))
    );
    assert!(url.len() > 64 * 1024);
    assert_eq!(
        link(&inventory.records()[0]).href().outcome().diagnostics(),
        vec![UrlDiagnostic::NonUrlCodePoint; 22_000]
    );
    let fixture = Fixture::new(&format!("<link rel=stylesheet href='/{}'>", "%".repeat(65)));
    let inventory = fixture.discover();
    assert_eq!(
        link(&inventory.records()[0]).href().outcome().diagnostics(),
        vec![UrlDiagnostic::InvalidPercentEncoding; 65]
    );
    let output = debug::Snapshot::new()
        .unwrap()
        .finish(format_args!("controlled"), Ok(Some(inventory)));
    assert!(output.contains("diagnostics=16/Some(65)"));
    assert!(output.contains("semantic_complete=true formatting_complete=false"));
    assert!(output.contains("diagnostics=true"));
    let mut fixture = Fixture::new(
        "<link rel=stylesheet href='https://e.test:70000/'><link rel=stylesheet href='https://e.test:70000/'>",
    );
    fixture.base = BrowserUrl::parse(&format!("https://e.test/{}/", "x".repeat(4096)))
        .unwrap()
        .url;
    let pair = fixture.base.as_str().len() + "https://e.test:70000/".len();
    let limits = StylesheetDiscoveryLimits {
        resolution_total_bytes: pair * 2,
        ..Default::default()
    };
    let inventory = fixture.with_limits(&limits).unwrap();
    for record in inventory.records() {
        let StylesheetHrefOutcome::Failed(f) = link(record).href().outcome() else {
            panic!("invalid port")
        };
        assert_eq!(f.base.as_ref(), Some(&fixture.base));
        assert_ne!(
            f.base.as_ref().unwrap().as_str().as_ptr(),
            fixture.base.as_str().as_ptr()
        );
    }
    assert_eq!(WORK.with(|w| w.get().resolution_bytes), pair * 2);
    limits_error(
        &fixture,
        StylesheetDiscoveryLimits {
            resolution_total_bytes: pair * 2 - 1,
            ..limits
        },
        StylesheetDiscoveryLimit::TotalResolutionInputBytes,
        pair * 2 - 1,
    );
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 1);
}
#[test]
fn aj6_borrowed_content_and_bounded_work() {
    let css = "secret".repeat(20_000);
    let fixture = Fixture::with_attribute_limit(
        &format!("<style>{css}</style><p style='{css}'></p>"),
        256 * 1024,
    );
    let inventory = fixture.discover();
    let text = block(&inventory.records()[0])
        .content()
        .text_chunks()
        .next()
        .unwrap();
    let StylesheetSource::StyleAttribute { value } = inventory.records()[1].source() else {
        panic!("attribute")
    };
    assert_eq!((text.len(), value.len()), (css.len(), css.len()));
    assert_eq!(WORK.with(|w| w.get().classification_bytes), 0);
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 0);
    let wide = document(
        (2..4098)
            .map(|id| element(id, ElementNamespace::Html, "p", &[("style", "")], vec![]))
            .collect(),
    );
    let inventory = discover_stylesheets(&wide, &fixture.base, &Default::default()).unwrap();
    let work = WORK.with(|w| w.get());
    assert_eq!(inventory.records().len(), 4096);
    assert_eq!(work.nodes, 4097);
    assert_eq!(work.frames, 1);
    assert!(work.frame_capacity <= 16);
    assert!(
        work.record_growths <= 10,
        "amortized record growth: {work:?}"
    );
    assert_eq!(work.record_capacity, 4096);
    assert!(work.attribute_examinations <= work.attributes * 2);
    let mut deep = element(300, ElementNamespace::Html, "p", &[("style", "")], vec![]);
    for id in (2..300).rev() {
        deep = element(id, ElementNamespace::Html, "div", &[], vec![deep]);
    }
    let deep = document(vec![deep]);
    let limits = StylesheetDiscoveryLimits {
        depth: 64,
        ..Default::default()
    };
    assert!(matches!(
        discover_stylesheets(&deep, &fixture.base, &limits),
        Err(StylesheetDiscoveryError::LimitExceeded {
            dimension: StylesheetDiscoveryLimit::OrdinaryDepth,
            ..
        })
    ));
    let work = WORK.with(|w| w.get());
    assert_eq!(work.depth, 64);
    assert!(work.frames <= 65 && work.frame_capacity <= 65);
}
#[test]
fn aj6_format_limits() {
    for count in [15, 16, 17] {
        let fixture = Fixture::new(&"<style></style>".repeat(count));
        let output = debug::Snapshot::new()
            .unwrap()
            .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
        assert!(output.contains(&format!("occurrences={}/{count}", count.min(16))));
        assert!(output.contains(if count <= 16 {
            "semantic_complete=true formatting_complete=true"
        } else {
            "semantic_complete=true formatting_complete=false"
        }));
        assert_eq!(output.contains("records=true"), count == 17);
        assert!(!output.contains("bytes=true"));
        assert!(!output.contains("scratch=true"));
    }
    for count in [15, 16, 17] {
        let fixture = Fixture::new(&format!(
            "<link rel=stylesheet href='/{}'>",
            "%".repeat(count)
        ));
        let output = debug::Snapshot::new()
            .unwrap()
            .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
        assert!(output.contains(&format!("diagnostics={}/Some({count})", count.min(16))));
        assert_eq!(output.contains("diagnostics=true"), count == 17);
        assert!(output.contains(if count <= 16 {
            "semantic_complete=true formatting_complete=true"
        } else {
            "semantic_complete=true formatting_complete=false"
        }));
        assert!(!output.contains("bytes=true"));
        assert!(!output.contains("scratch=true"));
    }
    let fixture = Fixture::new(&"<style></style>".repeat(32));
    let inventory = fixture.discover();
    let output = debug::Snapshot::new()
        .unwrap()
        .finish(format_args!("controlled"), Ok(Some(inventory)));
    assert!(output.len() <= 8192);
    assert!(output.contains("occurrences=16/32"));
    assert!(output.contains("records=true"));
    let limits = debug::FormatLimits {
        bytes: 700,
        ..Default::default()
    };
    let output = debug::Snapshot::with_limits(limits)
        .unwrap()
        .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
    assert!(output.len() <= 700);
    assert!(output.contains("bytes=true"));
    let limits = debug::FormatLimits {
        scratch: 32,
        ..Default::default()
    };
    let output = debug::Snapshot::with_limits(limits)
        .unwrap()
        .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
    assert!(output.contains("scratch=true"));
}

#[test]
fn aj6_qualified_attributes_and_identity_failures() {
    let fixture = Fixture::new(
        "<link xlink:rel=stylesheet href=unrelated.css><link rel=stylesheet xlink:href=ignored.css><link rel=stylesheet href=ok.css xlink:media=screen xlink:integrity=secret xlink:crossorigin=secret xlink:referrerpolicy=secret><p xlink:style=wrong style=right></p><svg><a xlink:href=qualified.css xlink:type=other xlink:title=named></a></svg>",
    );
    let inventory = fixture.discover();
    assert!(inventory.records()[0].dependency_order().is_none());
    assert!(matches!(
        link(&inventory.records()[1]).href().outcome(),
        StylesheetHrefOutcome::Missing
    ));
    let supported = link(&inventory.records()[2]);
    assert!(matches!(supported.media(), MediaFact::ActiveSubset(None)));
    assert!(matches!(
        supported.policies().integrity(),
        PolicyFact::Absent
    ));
    assert!(matches!(
        inventory.records()[3].source(),
        StylesheetSource::StyleAttribute { value: "right" }
    ));
    // Obtain genuinely qualified names from the canonical parser rather than
    // widening HTML's private qualified-name constructors just for the observer.
    fn foreign_attributes(node: &Node) -> Option<Vec<html::ParserCreatedAttribute>> {
        if let Node::Element { element } = node
            && element.namespace() == ElementNamespace::Svg
            && element.name() == "a"
        {
            return Some(element.attributes().to_vec());
        }
        node.children()
            .unwrap_or(&[])
            .iter()
            .find_map(foreign_attributes)
    }
    let qualified = foreign_attributes(&fixture.document).unwrap();
    assert!(
        qualified
            .iter()
            .all(|a| a.namespace() == AttributeNamespace::XLink)
    );
    let mut attrs = qualified;
    attrs.push(html::internal::unqualified_attribute("rel", "stylesheet"));
    let node = html::internal::node_element_from_parts(
        Id(2),
        html::internal::html_name("link"),
        attrs,
        vec![],
        vec![],
    );
    let qualified_document = document(vec![node]);
    let inventory =
        discover_stylesheets(&qualified_document, &fixture.base, &Default::default()).unwrap();
    let source = link(&inventory.records()[0]);
    assert!(matches!(
        source.href().outcome(),
        StylesheetHrefOutcome::Missing
    ));
    assert!(matches!(source.type_hint(), LinkTypeFact::Missing));
    assert!(matches!(source.title(), TitleFact::Missing));
    // The same CSS authority consumes neutral qualified style facts; its policy
    // is not reimplemented in Browser and those constructors remain CSS-owned.
    assert_eq!(
        css::get_inline_style(
            ElementNamespace::Svg,
            [
                css::SelectorDomAttribute::new(AttributeNamespace::XLink, "style", "wrong"),
                css::SelectorDomAttribute::new(AttributeNamespace::None, "style", "right")
            ]
        ),
        Some("right")
    );
    let node = element(
        0,
        ElementNamespace::Html,
        "link",
        &[("rel", "stylesheet"), ("href", "a.css")],
        vec![],
    );
    let invalid = document(vec![node]);
    assert_eq!(
        error(discover_stylesheets(
            &invalid,
            &fixture.base,
            &Default::default()
        )),
        StylesheetDiscoveryError::SourceIdentityUnavailable
    );
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 0);
    let pair = document(vec![element(
        2,
        ElementNamespace::Html,
        "link",
        &[("rel", "stylesheet"), ("href", "a.css"), ("style", "")],
        vec![],
    )]);
    let limits = StylesheetDiscoveryLimits {
        occurrences: 1,
        ..Default::default()
    };
    assert!(matches!(
        discover_stylesheets(&pair, &fixture.base, &limits),
        Err(StylesheetDiscoveryError::LimitExceeded {
            dimension: StylesheetDiscoveryLimit::Occurrences,
            ..
        })
    ));
    assert_eq!(WORK.with(|w| w.get().resolution_calls), 0);
}

#[test]
fn aj6_formatter_exact_boundaries_and_shared_diagnostics() {
    const BLOCK: &str = "  occurrence[0] owner=element[0] dependency=Some(0) kind=inline-block text=redacted type=missing-css-default/absent title=absent selection=NoNamedOrAlternateInput media=af-active-subset/absent\n    inapplicable-link-attributes: disabled=absent crossorigin=absent integrity=absent referrerpolicy=absent\n";
    const PREFIX: &str = "StylesheetDiscovery {\n  controlled\n  availability: available semantics=authored-candidates runtime-eligibility=not-observed issued-work=not-observed\n";
    let fixture = Fixture::new("<style></style>");
    for (scratch, complete) in [
        (BLOCK.len() - 1, false),
        (BLOCK.len(), true),
        (BLOCK.len() + 1, true),
    ] {
        let limits = debug::FormatLimits {
            scratch,
            ..Default::default()
        };
        let output = debug::Snapshot::with_limits(limits)
            .unwrap()
            .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
        assert_eq!(output.contains(BLOCK), complete);
        assert_eq!(output.contains("scratch=true"), !complete);
    }
    let exact = PREFIX.len() + BLOCK.len() + 512;
    for (bytes, complete) in [(exact - 1, false), (exact, true), (exact + 1, true)] {
        let limits = debug::FormatLimits {
            bytes,
            ..Default::default()
        };
        let output = debug::Snapshot::with_limits(limits)
            .unwrap()
            .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
        assert_eq!(output.contains(BLOCK), complete);
        assert_eq!(output.contains("bytes=true"), !complete);
        assert!(output.len() <= bytes);
    }
    let fixture = Fixture::new(&format!(
        "<link rel=stylesheet href='/{}'><link rel=stylesheet href='/{}'>",
        "%".repeat(10),
        "%".repeat(10)
    ));
    let output = debug::Snapshot::new()
        .unwrap()
        .finish(format_args!("controlled"), Ok(Some(fixture.discover())));
    assert!(output.contains("occurrences=2/2 diagnostics=16/Some(20)"));
    let second = output.split("occurrence[1]").nth(1).unwrap();
    assert!(second.contains(&format!(
        "total=10 shown={:?}",
        vec![UrlDiagnostic::InvalidPercentEncoding; 6]
    )));
    assert!(output.contains("diagnostics=true bytes=false scratch=false"));
}

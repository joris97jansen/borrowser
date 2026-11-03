use super::*;
use crate::page::PageState;
use addressing::{BrowserUrl, ResourceCapability as C, ResourceOperation as O, UrlDiagnostic as D};
use addressing::{UrlFailureKind, UrlParseError};

const DOCUMENT: &str = "https://example.test/dir/page?q=1#initial";
const EMPTY_RESOLVED: &str = "https://example.test/dir/page?q=1";

fn discover(markup: &str) -> Vec<HtmlResourceReference> {
    let parsed = html::parse_document(
        format!("<!doctype html>{markup}"),
        html::HtmlParseOptions::default(),
    )
    .unwrap();
    // Parser snapshots do not assign materialized IDs. Exercise Browser's actual
    // patch materializer so occurrence identity tests use its owning contract.
    let mut store = crate::dom_store::DomStore::new();
    let handle = core_types::DomHandle(1);
    store.create(handle).unwrap();
    store
        .apply(
            handle,
            core_types::DomVersion::INITIAL,
            core_types::DomVersion::INITIAL.next(),
            &parsed.patches,
        )
        .unwrap();
    let document = store.materialize_owned(handle).unwrap();
    let addresses = DocumentAddressState::new(BrowserUrl::parse(DOCUMENT).unwrap().url).unwrap();
    collect_html_resource_references(&document, &addresses)
}

fn address(record: &HtmlResourceReference) -> &ResourceAddress {
    match &record.outcome {
        HtmlReferenceOutcome::Resolved { address, .. } => {
            assert_eq!(record.kind, address.kind());
            address
        }
        other => panic!("expected resolution, got {other:?}"),
    }
}

#[test]
fn html_resource_representative_typed_occurrences() {
    let records = discover(concat!(
        "<link rel=stylesheet href='../site.css#sheet'>",
        "<body><img src='image.png#view'><a href='/next#part'>next</a>",
        "<form action='submit?q=2#result'></form>",
        "<audio><source src='sound.ogg#t=1'></audio>",
        "<iframe src='frame.html#part'></iframe><script src='app.js#entry'></script>",
    ));
    let expected = [
        (
            "href",
            ResourceKind::Stylesheet,
            "https://example.test/site.css#sheet",
        ),
        (
            "src",
            ResourceKind::Image,
            "https://example.test/dir/image.png#view",
        ),
        (
            "href",
            ResourceKind::Hyperlink,
            "https://example.test/next#part",
        ),
        (
            "action",
            ResourceKind::FormAction,
            "https://example.test/dir/submit?q=2#result",
        ),
        (
            "src",
            ResourceKind::Media,
            "https://example.test/dir/sound.ogg#t=1",
        ),
        (
            "src",
            ResourceKind::Document,
            "https://example.test/dir/frame.html#part",
        ),
        (
            "src",
            ResourceKind::Script,
            "https://example.test/dir/app.js#entry",
        ),
    ];
    assert_eq!(records.len(), expected.len());
    for (record, (attribute, kind, url)) in records.iter().zip(expected) {
        assert_eq!(record.attribute, attribute);
        assert_eq!(record.kind, kind);
        assert_eq!(address(record).url().as_str(), url);
    }
}

#[test]
fn html_resource_missing_empty_valueless_and_whitespace_matrix() {
    for (prefix, suffix, attribute, kind) in [
        (
            "<link rel=stylesheet",
            ">",
            "href",
            ResourceKind::Stylesheet,
        ),
        ("<img", ">", "src", ResourceKind::Image),
        ("<video><source", "></video>", "src", ResourceKind::Media),
        ("<a", "></a>", "href", ResourceKind::Hyperlink),
        ("<form", "></form>", "action", ResourceKind::FormAction),
        ("<iframe", "></iframe>", "src", ResourceKind::Document),
        ("<script", "></script>", "src", ResourceKind::Script),
    ] {
        assert!(discover(&format!("{prefix}{suffix}")).is_empty());
        for syntax in [attribute.to_owned(), format!("{attribute}=''")] {
            let records = discover(&format!("{prefix} {syntax}{suffix}"));
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].kind, kind);
            match kind {
                ResourceKind::Hyperlink => {
                    assert_eq!(address(&records[0]).url().as_str(), EMPTY_RESOLVED);
                }
                ResourceKind::FormAction => {
                    assert_eq!(address(&records[0]).url().as_str(), DOCUMENT);
                }
                _ => assert!(matches!(
                    records[0].outcome,
                    HtmlReferenceOutcome::EmptyReference
                )),
            }
        }
        let records = discover(&format!("{prefix} {attribute}=' \t\n\u{000C}\r '{suffix}"));
        assert_eq!(records.len(), 1);
        assert_eq!(address(&records[0]).url().as_str(), EMPTY_RESOLVED);
        let HtmlReferenceOutcome::Resolved { diagnostics, .. } = &records[0].outcome else {
            unreachable!()
        };
        assert!(
            !diagnostics.is_empty(),
            "whitespace diagnostics must survive"
        );
    }
}

#[test]
fn html_resource_rel_tokens_use_only_html_ascii_whitespace() {
    for separator in [' ', '\t', '\n', '\r', '\u{000C}'] {
        let records = discover(&format!(
            "<link rel='alternate{separator}StYlEsHeEt{separator}stylesheet' href='a.css'>"
        ));
        assert_eq!(records.len(), 1);
    }
    for rel in [
        "",
        " ",
        "style",
        "notstylesheet",
        "stylesheet-extra",
        "alternate\u{00A0}stylesheet",
        "alternate\u{000B}stylesheet",
    ] {
        assert!(discover(&format!("<link rel='{rel}' href='a.css'>")).is_empty());
    }
    assert!(discover("<link href='a.css'>").is_empty());
    assert_eq!(
        discover(
            "<link rel='stylesheet alternate' disabled media=print type='other/type' href=a.css>"
        )
        .len(),
        1
    );
}

#[test]
fn html_resource_script_type_classification_is_inert_and_exact() {
    for attributes in [
        "",
        "type=''",
        "type=module",
        "type=MoDuLe",
        "language=''",
        "language=javascript",
        "language=JavaScript1.5",
        "language=jscript",
        "language=vbscript type=''",
        "language=vbscript type=module",
        "type=' \ttext/JavaScript\n '",
        "nomodule async defer",
    ] {
        let records = discover(&format!("<script {attributes} src='app.js'></script>"));
        assert_eq!(records.len(), 1, "{attributes}");
        assert_eq!(records[0].kind, ResourceKind::Script);
        assert_eq!(address(&records[0]).capability(O::Retrieval), C::Supported);
    }
    // Independently enumerated MIME Sniffing JavaScript essence fixtures.
    for mime in [
        "application/ecmascript",
        "application/javascript",
        "application/x-ecmascript",
        "application/x-javascript",
        "text/ecmascript",
        "text/javascript",
        "text/javascript1.0",
        "text/javascript1.1",
        "text/javascript1.2",
        "text/javascript1.3",
        "text/javascript1.4",
        "text/javascript1.5",
        "text/jscript",
        "text/livescript",
        "text/x-ecmascript",
        "text/x-javascript",
    ] {
        let records = discover(&format!("<script type='{mime}' src='app.js'></script>"));
        assert_eq!(records.len(), 1, "{mime}");
        assert_eq!(address(&records[0]).kind(), ResourceKind::Script);
    }
    for attributes in [
        "type=importmap",
        "type=IMPORTMAP",
        "type=speculationrules",
        "type=application/json",
        "type=text/plain",
        "type=unknown",
        "type=' '",
        "type=' module '",
        "type='text/javascript;charset=utf-8'",
        "type='text/javascript; charset=utf-8'",
        "type='\u{00A0}text/javascript\u{00A0}'",
        "language=vbscript",
        "language=' javascript '",
        "language=javascript type=application/json",
    ] {
        assert!(
            discover(&format!("<script {attributes} src='app.js'></script>")).is_empty(),
            "{attributes}"
        );
    }
    assert!(discover("<script>const text = 'src=hidden.js';</script>").is_empty());
}

#[test]
fn html_resource_media_picture_and_iframe_are_authored_occurrences() {
    let records = discover(concat!(
        "<picture><source src=wrong.png srcset='candidate.png 2x'><img src=fallback.png srcset='other.png 2x'></picture>",
        "<audio src=direct.ogg><source src=alternative.ogg type=unknown media=print></audio>",
        "<video><div><source src=wrong.ogg></div><source src=video.webm></video>",
        "<source src=orphan.ogg>",
        "<iframe src=frame.html srcdoc='&lt;img src=inside.png>'></iframe>",
        "<iframe src=frame.html></iframe>",
    ));
    let urls: Vec<_> = records.iter().map(|r| address(r).url().as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://example.test/dir/fallback.png",
            "https://example.test/dir/alternative.ogg",
            "https://example.test/dir/video.webm",
            "https://example.test/dir/frame.html",
            "https://example.test/dir/frame.html",
        ]
    );
    assert_ne!(records[3].element, records[4].element);
}

#[test]
fn html_resource_namespace_and_template_boundaries() {
    let records = discover(concat!(
        "<template><link rel=stylesheet href=inert.css><img src=inert.png><a href=inert.html>x</a>",
        "<form action=inert></form><script src=inert.js></script></template>",
        "<svg><a href=foreign.html>x</a><script src=foreign.js></script>",
        "<foreignObject><a href=integrated.html>x</a><img src=integrated.png></foreignObject></svg>",
        "<a xlink:href=qualified.html>x</a><img xlink:src=qualified.png>",
        "<math><annotation-xml encoding='text/html'><a href=math-integrated.html>x</a></annotation-xml></math>",
    ));
    assert_eq!(
        records
            .iter()
            .map(|r| address(r).url().as_str())
            .collect::<Vec<_>>(),
        [
            "https://example.test/dir/integrated.html",
            "https://example.test/dir/integrated.png",
            "https://example.test/dir/math-integrated.html",
        ]
    );
}

#[test]
fn html_resource_failures_do_not_stop_ordered_diagnostics_or_later_records() {
    let records = discover(concat!(
        "<a href=' https://u:p@h.test:70000/'>bad</a>",
        "<a href='/a%zz\\b%xx'>recover</a><img src='later.png'>",
    ));
    assert_eq!(records.len(), 3);
    let HtmlReferenceOutcome::Failed(failure) = &records[0].outcome else {
        panic!("expected typed failure")
    };
    assert_eq!(
        failure.kind,
        UrlFailureKind::InvalidInput(UrlParseError::InvalidPort)
    );
    assert_eq!(failure.base.as_ref().unwrap().as_str(), DOCUMENT);
    assert_eq!(
        failure.diagnostics,
        [D::BoundaryControlOrSpaceIgnored, D::EmbeddedCredentials]
    );
    assert_eq!(
        address(&records[1]).url().as_str(),
        "https://example.test/a%zz/b%xx"
    );
    let HtmlReferenceOutcome::Resolved { diagnostics, .. } = &records[1].outcome else {
        unreachable!()
    };
    assert_eq!(
        diagnostics,
        &[
            D::InvalidPercentEncoding,
            D::Backslash,
            D::InvalidPercentEncoding
        ]
    );
    assert_eq!(
        address(&records[2]).url().as_str(),
        "https://example.test/dir/later.png"
    );
}

#[test]
fn html_resource_complete_addresses_preserve_distinct_occurrences_and_decoded_values() {
    let records = discover(
        "<a id=same href='/item?a=1&amp;a=2#one'>x</a><a id=same href='/item?a=1&amp;a=2#two'>x</a><img src='&#160;'>",
    );
    assert_eq!(records.len(), 3);
    assert_ne!(records[0].element, records[1].element);
    assert_eq!(
        address(&records[0]).url().as_str(),
        "https://example.test/item?a=1&a=2#one"
    );
    assert_eq!(address(&records[1]).url().fragment(), Some("two"));
    assert_eq!(
        address(&records[0]).retrieval_key(),
        address(&records[1]).retrieval_key()
    );
    assert_eq!(
        address(&records[2]).url().as_str(),
        "https://example.test/dir/%C2%A0"
    );
}

#[test]
fn html_resource_resolution_success_is_independent_of_operation_support() {
    for (url, retrieval, navigation) in [
        ("https://h.test/", C::Supported, C::Supported),
        ("file:///tmp/a", C::Supported, C::Supported),
        ("about:blank", C::Unsupported, C::Reserved),
        ("data:text/plain,x", C::Deferred, C::Deferred),
        ("blob:https://h.test/id", C::Deferred, C::Deferred),
        ("ws://h.test/", C::Reserved, C::Reserved),
        ("javascript:void(0)", C::Unsupported, C::Unsupported),
        ("custom:opaque", C::Unsupported, C::Unsupported),
    ] {
        let records = discover(&format!("<a href='{url}'>x</a>"));
        let address = address(&records[0]);
        assert_eq!(address.url().as_str(), url);
        assert_eq!(address.capability(O::Retrieval), retrieval);
        assert_eq!(address.capability(O::Navigation), navigation);
    }
}

#[test]
fn html_resource_unmanaged_and_empty_documents_are_distinct() {
    let mut page = PageState::new();
    assert!(page.html_resource_references().is_none());
    page.start_nav(DOCUMENT);
    assert!(page.html_resource_references().is_none());
    page.dom = Some(Box::new(
        html::parse_document("<!doctype html><p>empty", Default::default())
            .unwrap()
            .document,
    ));
    assert!(page.html_resource_references().unwrap().is_empty());
}

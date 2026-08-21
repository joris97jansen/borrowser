use browser::dom_store::DomStore;
use core_types::{DomHandle, DomVersion};
use css::{
    DocumentSelectorMatchingDiagnosticLimits, ParseOptions, SelectorMatchingEnvironment,
    StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder, StylesheetSourceId,
    document_selector_matching_diagnostic, parse_stylesheet_with_options,
};
use html::{DocumentMode, HtmlParseOptions, parse_document};

#[test]
fn parser_document_and_same_full_patch_history_have_selector_visible_parity() {
    let output = parse_document(
        concat!(
            "<!doctype html><html><body>",
            "<main id=scope><p id=target class='card hot' data-kind=Primary></p></main>",
            "<section><b id=before></b>text<!--gap--><?pi data?><i id=after></i></section>",
            "<svg><foreignObject id=fo><article id=integration></article></foreignObject></svg>",
            "<math><mi id=math></mi></math>",
            "<template id=host><span id=in-template></span></template>",
            "</body></html>"
        ),
        HtmlParseOptions::default(),
    )
    .expect("HTML parses once");
    assert_eq!(output.document_mode, DocumentMode::NoQuirks);
    assert!(
        output.contains_full_patch_history,
        "ParseOutput.patches is authoritative only with complete history"
    );

    let stylesheet = parse_stylesheet_with_options(
        concat!(
            "* {} main > p.card[data-kind=Primary] {} #before + #after {} ",
            "foreignObject > article:empty {} mi:empty {} template span {} ",
            ":root {} :hover {} p::before {} > broken {}"
        ),
        &ParseOptions::stylesheet(),
    );
    let environment = SelectorMatchingEnvironment::new(output.document_mode);
    let inputs = [StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &stylesheet,
        StylesheetConditionInput::None,
    )];
    let limits = DocumentSelectorMatchingDiagnosticLimits::default();
    let direct =
        document_selector_matching_diagnostic(&output.document, environment, &inputs, limits)
            .to_debug_snapshot();

    let handle = DomHandle(404);
    let mut store = DomStore::new();
    store
        .create(handle)
        .expect("create staged Browser DOM handle");
    store
        .apply(
            handle,
            DomVersion::INITIAL,
            DomVersion::INITIAL.next(),
            &output.patches,
        )
        .expect("apply the exact parser patch history");
    let materialized = store.materialize(handle).expect("materialize Browser DOM");
    let via_browser =
        document_selector_matching_diagnostic(&materialized, environment, &inputs, limits)
            .to_debug_snapshot();

    assert_eq!(
        via_browser, direct,
        "stable CSS-owned selector-visible behavior must survive parser patch materialization"
    );
    assert!(direct.contains("namespace=svg local=\"foreignObject\""));
    assert!(direct.contains("namespace=html local=\"article\""));
    assert!(!direct.contains("id-attribute=\"in-template\""));
    assert!(direct.contains("matchability=unsupported"));
    assert!(direct.contains("matchability=invalid"));
}

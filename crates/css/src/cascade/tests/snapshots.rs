use super::super::{resolve_document_styles, resolve_document_styles_debug_snapshot};
use super::support::{element, stylesheet};

#[test]
fn resolved_document_style_debug_snapshot_is_stable() {
    let stylesheets = vec![stylesheet("div { color: red; }")];
    let dom = element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

    assert_eq!(
        resolved.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "resolved-document-style\n",
            "element[0]: selector-id=1 name=\"div\"\n",
            "  resolved-style\n",
            "    background-color: initial(transparent)\n",
            "    border-bottom-color: initial(transparent)\n",
            "    border-bottom-style: initial(none)\n",
            "    border-bottom-width: initial(0px)\n",
            "    border-left-color: initial(transparent)\n",
            "    border-left-style: initial(none)\n",
            "    border-left-width: initial(0px)\n",
            "    border-right-color: initial(transparent)\n",
            "    border-right-style: initial(none)\n",
            "    border-right-width: initial(0px)\n",
            "    border-top-color: initial(transparent)\n",
            "    border-top-style: initial(none)\n",
            "    border-top-width: initial(0px)\n",
            "    color: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"red\")\n",
            "    display: initial(inline)\n",
            "    font-size: initial(16px)\n",
            "    height: initial(auto)\n",
            "    margin-bottom: initial(0px)\n",
            "    margin-left: initial(0px)\n",
            "    margin-right: initial(0px)\n",
            "    margin-top: initial(0px)\n",
            "    max-width: initial(none)\n",
            "    min-width: initial(auto)\n",
            "    overflow: initial(visible)\n",
            "    outline-color: initial(transparent)\n",
            "    outline-style: initial(none)\n",
            "    outline-width: initial(0px)\n",
            "    padding-bottom: initial(0px)\n",
            "    padding-left: initial(0px)\n",
            "    padding-right: initial(0px)\n",
            "    padding-top: initial(0px)\n",
            "    position: initial(static)\n",
            "    text-decoration-line: initial(none)\n",
            "    width: initial(auto)\n",
            "    z-index: initial(auto)\n",
        )
    );
}

#[test]
fn document_style_resolution_debug_snapshot_covers_override_inheritance_and_defaults() {
    let stylesheets = vec![stylesheet(
        "section { color: red; } div { color: green; } .hero { color: blue !important; }",
    )];
    let dom = element(
        "section",
        Vec::new(),
        vec![element("div", vec![("class", Some("hero"))], Vec::new())],
    );

    assert_eq!(
        resolve_document_styles_debug_snapshot(&dom, &stylesheets),
        concat!(
            "version: 1\n",
            "document-style-resolution\n",
            "element[0]: selector-id=1 name=\"section\"\n",
            "  cascade-evaluation\n",
            "  rule-inputs: 1\n",
            "    rule-input[0]: source=stylesheet[0/0] origin=author specificity=selector(0,0,1) rule-order=0 declarations=1\n",
            "      declaration[0]: source=stylesheet[0/0]/declaration[0] declaration-order=0 importance=normal property=supported(color) applicability=supported(color) value=\"red\"\n",
            "  candidates-source-order: 1\n",
            "    candidate[0]: property=color source=stylesheet[0/0]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=0 declaration-order=0 value=\"red\"\n",
            "  candidates-cascade-order: 1\n",
            "    candidate[0]: property=color source=stylesheet[0/0]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=0 declaration-order=0 value=\"red\"\n",
            "  winners: 1\n",
            "    color: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"red\")\n",
            "  resolved-style\n",
            "    background-color: initial(transparent)\n",
            "    border-bottom-color: initial(transparent)\n",
            "    border-bottom-style: initial(none)\n",
            "    border-bottom-width: initial(0px)\n",
            "    border-left-color: initial(transparent)\n",
            "    border-left-style: initial(none)\n",
            "    border-left-width: initial(0px)\n",
            "    border-right-color: initial(transparent)\n",
            "    border-right-style: initial(none)\n",
            "    border-right-width: initial(0px)\n",
            "    border-top-color: initial(transparent)\n",
            "    border-top-style: initial(none)\n",
            "    border-top-width: initial(0px)\n",
            "    color: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"red\")\n",
            "    display: initial(inline)\n",
            "    font-size: initial(16px)\n",
            "    height: initial(auto)\n",
            "    margin-bottom: initial(0px)\n",
            "    margin-left: initial(0px)\n",
            "    margin-right: initial(0px)\n",
            "    margin-top: initial(0px)\n",
            "    max-width: initial(none)\n",
            "    min-width: initial(auto)\n",
            "    overflow: initial(visible)\n",
            "    outline-color: initial(transparent)\n",
            "    outline-style: initial(none)\n",
            "    outline-width: initial(0px)\n",
            "    padding-bottom: initial(0px)\n",
            "    padding-left: initial(0px)\n",
            "    padding-right: initial(0px)\n",
            "    padding-top: initial(0px)\n",
            "    position: initial(static)\n",
            "    text-decoration-line: initial(none)\n",
            "    width: initial(auto)\n",
            "    z-index: initial(auto)\n",
            "element[1]: selector-id=2 name=\"div\"\n",
            "  cascade-evaluation\n",
            "  rule-inputs: 2\n",
            "    rule-input[0]: source=stylesheet[0/1] origin=author specificity=selector(0,0,1) rule-order=1 declarations=1\n",
            "      declaration[0]: source=stylesheet[0/1]/declaration[0] declaration-order=0 importance=normal property=supported(color) applicability=supported(color) value=\"green\"\n",
            "    rule-input[1]: source=stylesheet[0/2] origin=author specificity=selector(0,1,0) rule-order=2 declarations=1\n",
            "      declaration[0]: source=stylesheet[0/2]/declaration[0] declaration-order=0 importance=important property=supported(color) applicability=supported(color) value=\"blue\"\n",
            "  candidates-source-order: 2\n",
            "    candidate[0]: property=color source=stylesheet[0/1]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=1 declaration-order=0 value=\"green\"\n",
            "    candidate[1]: property=color source=stylesheet[0/2]/declaration[0] band=author-important specificity=selector(0,1,0) rule-order=2 declaration-order=0 value=\"blue\"\n",
            "  candidates-cascade-order: 2\n",
            "    candidate[0]: property=color source=stylesheet[0/1]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=1 declaration-order=0 value=\"green\"\n",
            "    candidate[1]: property=color source=stylesheet[0/2]/declaration[0] band=author-important specificity=selector(0,1,0) rule-order=2 declaration-order=0 value=\"blue\"\n",
            "  winners: 1\n",
            "    color: winner(source=stylesheet[0/2]/declaration[0], band=author-important, specificity=selector(0,1,0), rule-order=2, declaration-order=0, value=\"blue\")\n",
            "  resolved-style\n",
            "    background-color: initial(transparent)\n",
            "    border-bottom-color: initial(transparent)\n",
            "    border-bottom-style: initial(none)\n",
            "    border-bottom-width: initial(0px)\n",
            "    border-left-color: initial(transparent)\n",
            "    border-left-style: initial(none)\n",
            "    border-left-width: initial(0px)\n",
            "    border-right-color: initial(transparent)\n",
            "    border-right-style: initial(none)\n",
            "    border-right-width: initial(0px)\n",
            "    border-top-color: initial(transparent)\n",
            "    border-top-style: initial(none)\n",
            "    border-top-width: initial(0px)\n",
            "    color: winner(source=stylesheet[0/2]/declaration[0], band=author-important, specificity=selector(0,1,0), rule-order=2, declaration-order=0, value=\"blue\")\n",
            "    display: initial(inline)\n",
            "    font-size: inherited\n",
            "    height: initial(auto)\n",
            "    margin-bottom: initial(0px)\n",
            "    margin-left: initial(0px)\n",
            "    margin-right: initial(0px)\n",
            "    margin-top: initial(0px)\n",
            "    max-width: initial(none)\n",
            "    min-width: initial(auto)\n",
            "    overflow: initial(visible)\n",
            "    outline-color: initial(transparent)\n",
            "    outline-style: initial(none)\n",
            "    outline-width: initial(0px)\n",
            "    padding-bottom: initial(0px)\n",
            "    padding-left: initial(0px)\n",
            "    padding-right: initial(0px)\n",
            "    padding-top: initial(0px)\n",
            "    position: initial(static)\n",
            "    text-decoration-line: initial(none)\n",
            "    width: initial(auto)\n",
            "    z-index: initial(auto)\n",
        )
    );
}

#[test]
fn document_style_resolution_keeps_unknown_properties_with_css_wide_values_unsupported() {
    let stylesheets = vec![stylesheet("div { zoom: initial; color: red; }")];
    let dom = element("div", Vec::new(), Vec::new());

    let snapshot = resolve_document_styles_debug_snapshot(&dom, &stylesheets);

    assert!(
        snapshot.contains(
            "property=unsupported(\"zoom\") applicability=unsupported-property value=\"initial\""
        ),
        "{snapshot}"
    );
    assert!(
        snapshot.contains("color: winner("),
        "known supported declarations should still resolve: {snapshot}"
    );
}

#[test]
fn document_style_resolution_debug_snapshot_shows_outline_shorthand_expansion_order() {
    let stylesheets = vec![stylesheet("div { outline: 2px solid red; }")];
    let dom = element("div", Vec::new(), Vec::new());

    let snapshot = resolve_document_styles_debug_snapshot(&dom, &stylesheets);

    assert!(
        snapshot.contains(
            "rule-input[0]: source=stylesheet[0/0] origin=author specificity=selector(0,0,1) rule-order=0 declarations=3"
        ),
        "{snapshot}"
    );
    assert!(
        snapshot.contains(
            "declaration[0]: source=stylesheet[0/0]/declaration[0] declaration-order=0 importance=normal property=supported(outline-color) applicability=supported(outline-color) value=\"red\""
        ),
        "{snapshot}"
    );
    assert!(
        snapshot.contains(
            "declaration[1]: source=stylesheet[0/0]/declaration[0] declaration-order=0 expansion-order=1 importance=normal property=supported(outline-style) applicability=supported(outline-style) value=\"solid\""
        ),
        "{snapshot}"
    );
    assert!(
        snapshot.contains(
            "declaration[2]: source=stylesheet[0/0]/declaration[0] declaration-order=0 expansion-order=2 importance=normal property=supported(outline-width) applicability=supported(outline-width) value=\"2px\""
        ),
        "{snapshot}"
    );
}

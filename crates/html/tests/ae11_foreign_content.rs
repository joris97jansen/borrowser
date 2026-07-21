#![cfg(feature = "html5")]

use html::dom_snapshot::{DomSnapshot, DomSnapshotOptions};
use html::html5::{DocumentParseContext, Html5ParseSession, TokenizerConfig, TreeBuilderConfig};
use html::{DomPatch, ElementNamespace};
use html::{HtmlParseOptions, parse_document};

fn parse(chunks: &[&str]) -> (Vec<DomPatch>, Vec<html::html5::ParseError>) {
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        DocumentParseContext::new(),
    )
    .unwrap();
    for chunk in chunks {
        session.push_str(chunk).unwrap();
        session.pump().unwrap();
    }
    session.finish().unwrap();
    (session.take_patches(), session.parse_errors())
}

fn created_elements(patches: &[DomPatch]) -> Vec<(ElementNamespace, String)> {
    patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateElement { name, .. } => {
                Some((name.namespace(), name.local_name_str().to_string()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn namespace_inheritance_and_dispatcher_exceptions_are_explicit() {
    let cases = [
        (
            "<svg><math></math></svg><p>x</p>",
            vec![
                (ElementNamespace::Html, "html"),
                (ElementNamespace::Html, "head"),
                (ElementNamespace::Html, "body"),
                (ElementNamespace::Svg, "svg"),
                (ElementNamespace::Svg, "math"),
                (ElementNamespace::Html, "p"),
            ],
        ),
        (
            "<math><svg></svg></math>",
            vec![
                (ElementNamespace::Html, "html"),
                (ElementNamespace::Html, "head"),
                (ElementNamespace::Html, "body"),
                (ElementNamespace::MathMl, "math"),
                (ElementNamespace::MathMl, "svg"),
            ],
        ),
        (
            "<math><annotation-xml><svg></svg></annotation-xml></math>",
            vec![
                (ElementNamespace::Html, "html"),
                (ElementNamespace::Html, "head"),
                (ElementNamespace::Html, "body"),
                (ElementNamespace::MathMl, "math"),
                (ElementNamespace::MathMl, "annotation-xml"),
                (ElementNamespace::Svg, "svg"),
            ],
        ),
    ];
    for (source, expected) in cases {
        let (patches, _) = parse(&[source]);
        assert_eq!(
            created_elements(&patches),
            expected
                .into_iter()
                .map(|(namespace, local)| (namespace, local.to_string()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn svg_adjustments_and_ordered_qualified_attributes_survive_patches() {
    let (patches, _) = parse(&[
        "<svg><lineargradient viewbox='0 0 1 1' xlink:href='#a' xml:lang='en' xmlns xmlns:xlink></lineargradient></svg>",
    ]);
    let (_, attributes) = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement {
                name, attributes, ..
            } if name.is(ElementNamespace::Svg, "linearGradient") => Some((name, attributes)),
            _ => None,
        })
        .expect("adjusted SVG element");
    assert_eq!(
        attributes
            .iter()
            .map(|attribute| (
                attribute.namespace().snapshot_name(),
                attribute.prefix(),
                attribute.local_name(),
                attribute.value(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("none", None, "viewBox", "0 0 1 1"),
            ("xlink", Some("xlink"), "href", "#a"),
            ("xml", Some("xml"), "lang", "en"),
            ("xmlns", None, "xmlns", ""),
            ("xmlns", Some("xmlns"), "xlink", ""),
        ]
    );
}

#[test]
fn cdata_boundary_is_namespace_driven_and_chunk_invariant() {
    let whole = parse(&["<svg><![CDATA[<circle/>]]></svg>"]).0;
    let chunked = parse(&["<svg><![C", "DATA[<circle/>", "]]></svg>"]).0;
    assert_eq!(whole, chunked);
    assert!(whole.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateText { text, .. } if text == "<circle/>"
    )));

    let (html_patches, html_errors) = parse(&["<div><![CDATA[<x>]]></div>"]);
    assert!(!html_patches.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateText { text, .. } if text == "<x>"
    )));
    assert!(!html_errors.is_empty());
}

#[test]
fn pi_like_syntax_in_foreign_cdata_remains_text_at_every_chunk_boundary() {
    let source = "<svg><![CDATA[<?pi?>]]></svg>";
    let whole = parse(&[source]);
    assert!(whole.0.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateText { text, .. } if text == "<?pi?>"
    )));
    assert!(
        !whole
            .0
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateProcessingInstruction { .. }))
    );

    for split in 0..=source.len() {
        let chunked = parse(&[&source[..split], &source[split..]]);
        assert_eq!(chunked, whole, "split at byte {split}");
    }
}

#[test]
fn cdata_eof_recovery_is_state_complete_and_chunk_invariant() {
    for source in ["<svg><![CDATA[x", "<svg><![CDATA[x]", "<svg><![CDATA[x]]"] {
        let whole = parse(&[source]);
        let split = source.len().saturating_sub(1);
        let chunked = parse(&[&source[..split], &source[split..]]);
        assert_eq!(chunked, whole, "{source}");
        assert!(
            whole.0.iter().any(
                |patch| matches!(patch, DomPatch::CreateText { text, .. } if text.contains('x'))
            )
        );
        assert!(
            whole
                .1
                .iter()
                .any(|error| error.detail == Some("eof-in-cdata"))
        );
    }
}

#[test]
fn math_text_exclusions_and_foreign_self_closing_are_stable() {
    let (whole, errors) =
        parse(&["<math><mi><mglyph/></mi><mi><malignmark/></mi></math><svg><g/></svg><p>x</p>"]);
    let chunked = parse(&[
        "<math><mi><m",
        "glyph/></mi><mi><malign",
        "mark/></mi></math><svg><g/>",
        "</svg><p>x</p>",
    ])
    .0;
    assert_eq!(whole, chunked);
    let elements = created_elements(&whole);
    assert!(elements.contains(&(ElementNamespace::MathMl, "mglyph".to_string())));
    assert!(elements.contains(&(ElementNamespace::MathMl, "malignmark".to_string())));
    assert!(elements.contains(&(ElementNamespace::Html, "p".to_string())));
    assert!(!errors.iter().any(|error| {
        error.detail == Some("non-void-html-element-start-tag-with-trailing-solidus")
    }));
}

#[test]
fn every_mathml_text_integration_point_dispatches_per_token() {
    for local_name in ["mi", "mo", "mn", "ms", "mtext"] {
        let source = format!("<math><{local_name}>x<b>html</b></{local_name}></math><p>after</p>");
        let expected = vec![
            (ElementNamespace::Html, "html".to_string()),
            (ElementNamespace::Html, "head".to_string()),
            (ElementNamespace::Html, "body".to_string()),
            (ElementNamespace::MathMl, "math".to_string()),
            (ElementNamespace::MathMl, local_name.to_string()),
            (ElementNamespace::Html, "b".to_string()),
            (ElementNamespace::Html, "p".to_string()),
        ];
        assert_eq!(
            created_elements(&parse(&[&source]).0),
            expected,
            "{local_name}"
        );
        assert_all_ascii_chunk_boundaries(&source);
    }
}

#[test]
fn integration_points_breakout_unknowns_and_templates_preserve_namespaces() {
    let source = concat!(
        "<svg><foreignObject><div disabled></div></foreignObject>",
        "<unknownSvg></unknownSvg></svg>",
        "<math><annotation-xml encoding='APPLICATION/XHTML+XML'><span></span></annotation-xml>",
        "<unknownMath></unknownMath></math>",
        "<template><svg><![CDATA[<g/>]]></svg></template>",
        "<p>after</p>"
    );
    let whole = parse(&[source]).0;
    let chunked = parse(&[
        "<svg><foreign",
        "Object><div disabled></div></foreignObject><unknownSvg>",
        "</unknownSvg></svg><math><annotation-xml encoding='APPLICATION/XHTML+XML'>",
        "<span></span></annotation-xml><unknownMath></unknownMath></math><template>",
        "<svg><![CDATA[<g/>",
        "]]></svg></template><p>after</p>",
    ])
    .0;
    assert_eq!(whole, chunked);
    let elements = created_elements(&whole);
    for expected in [
        (ElementNamespace::Svg, "foreignObject"),
        (ElementNamespace::Html, "div"),
        (ElementNamespace::Svg, "unknownsvg"),
        (ElementNamespace::MathMl, "annotation-xml"),
        (ElementNamespace::Html, "span"),
        (ElementNamespace::MathMl, "unknownmath"),
        (ElementNamespace::Html, "template"),
        (ElementNamespace::Svg, "svg"),
        (ElementNamespace::Html, "p"),
    ] {
        assert!(
            elements.contains(&(expected.0, expected.1.to_string())),
            "{expected:?}"
        );
    }
    let disabled = whole.iter().find_map(|patch| match patch {
        DomPatch::CreateElement {
            name, attributes, ..
        } if name.is(ElementNamespace::Html, "div") => attributes
            .iter()
            .find(|attribute| attribute.local_name() == "disabled"),
        _ => None,
    });
    assert_eq!(disabled.map(|attribute| attribute.value()), Some(""));
}

#[test]
fn breakout_and_foreign_end_tag_errors_are_exact_and_recoverable() {
    let (matching, matching_errors) = parse(&["<svg><g></g></svg>"]);
    assert!(created_elements(&matching).contains(&(ElementNamespace::Svg, "g".to_string())));
    assert_eq!(
        matching_errors
            .iter()
            .filter(|error| error.detail == Some("foreign-end-tag-current-node-mismatch"))
            .count(),
        0
    );

    let (mismatch, _) = parse(&["<svg><g><path></g></svg><p>after</p>"]);
    assert!(created_elements(&mismatch).contains(&(ElementNamespace::Html, "p".to_string())));

    let (breakout, breakout_errors) = parse(&["<svg><g><p>html</p></svg><div>after</div>"]);
    let elements = created_elements(&breakout);
    assert!(elements.contains(&(ElementNamespace::Html, "p".to_string())));
    assert!(elements.contains(&(ElementNamespace::Html, "div".to_string())));
    let _ = breakout_errors;
}

#[test]
fn valueless_html_and_foreign_attributes_have_dom_empty_string_values() {
    for source in [
        "<input disabled>",
        "<input disabled=''>",
        "<svg><g data-flag/></svg>",
        "<svg><g data-flag=''/></svg>",
    ] {
        let (patches, _) = parse(&[source]);
        let value = patches.iter().find_map(|patch| match patch {
            DomPatch::CreateElement { attributes, .. } => attributes
                .iter()
                .find(|attribute| matches!(attribute.local_name(), "disabled" | "data-flag"))
                .map(|attribute| attribute.value()),
            _ => None,
        });
        assert_eq!(value, Some(""), "{source}");
    }
}

fn assert_all_ascii_chunk_boundaries(source: &str) {
    let whole = parse(&[source]);
    for boundary in 0..=source.len() {
        let chunked = parse(&[&source[..boundary], &source[boundary..]]);
        assert_eq!(chunked, whole, "chunk boundary {boundary} for {source}");
    }
}

#[test]
fn integration_dispatch_is_per_token_for_svg_math_and_annotation_xml() {
    let source = concat!(
        "<svg><desc><b>d</b></desc><title><span>t</span></title>",
        "<font></font><font face='serif'>html</font></svg>",
        "<math><mi><b>x</b><mglyph></mglyph><malignmark></malignmark></mi>",
        "<annotation-xml encoding='text/html'><div>h</div></annotation-xml>",
        "<annotation-xml encoding='text/plain'><foreign-token>m</foreign-token></annotation-xml></math>",
        "<p>after</p>"
    );
    let elements = created_elements(&parse(&[source]).0);
    for expected in [
        (ElementNamespace::Svg, "desc"),
        (ElementNamespace::Svg, "title"),
        (ElementNamespace::Html, "b"),
        (ElementNamespace::Html, "span"),
        (ElementNamespace::Svg, "font"),
        (ElementNamespace::Html, "font"),
        (ElementNamespace::MathMl, "mglyph"),
        (ElementNamespace::MathMl, "malignmark"),
        (ElementNamespace::Html, "div"),
        (ElementNamespace::MathMl, "foreign-token"),
        (ElementNamespace::Html, "p"),
    ] {
        assert!(
            elements.contains(&(expected.0, expected.1.to_string())),
            "{expected:?}"
        );
    }
    assert_all_ascii_chunk_boundaries(source);
}

#[test]
fn nested_boundaries_cdata_and_namespace_restoration_are_split_invariant() {
    for source in [
        "<svg><svg><g/></svg></svg><p>after</p>",
        "<math><math><mi>x</mi></math></math><p>after</p>",
        "<svg><math></math></svg><p>after</p>",
        "<math><svg></svg></math><p>after</p>",
        "<math><annotation-xml><svg></svg></annotation-xml></math><p>after</p>",
        "<math><mi><mglyph></mglyph><malignmark></malignmark></mi></math>",
        "<math><![CDATA[<mi>x</mi>]]></math>",
        "<svg><!--kept--><!DOCTYPE html><g></g></svg>",
        "<template><svg><![CDATA[<g/>]]></svg></template><p>after</p>",
    ] {
        assert_all_ascii_chunk_boundaries(source);
        assert!(created_elements(&parse(&[source]).0).contains(&(
            ElementNamespace::Html,
            if source.contains("<p>") { "p" } else { "html" }.to_string()
        )));
    }
}

#[test]
fn deterministic_tree_snapshot_exposes_mixed_namespaces_and_ordered_attributes() {
    let output = parse_document(
        concat!(
            "<svg viewbox='0 0 1 1'><lineargradient xlink:href='#a'/>",
            "<foreignObject><div disabled>h</div></foreignObject></svg>",
            "<math><mi>x</mi></math><p>after</p>"
        ),
        HtmlParseOptions::default(),
    )
    .unwrap();
    let snapshot = DomSnapshot::new(&output.document, DomSnapshotOptions::default()).render();
    assert_eq!(
        snapshot,
        concat!(
            "#dom-snapshot-v2\n",
            "#document\n",
            "  element ns=html local=\"html\" attrs=[]\n",
            "    element ns=html local=\"head\" attrs=[]\n",
            "    element ns=html local=\"body\" attrs=[]\n",
            "      element ns=svg local=\"svg\" attrs=[{ns=none prefix=- local=\"viewBox\" value=\"0 0 1 1\"}]\n",
            "        element ns=svg local=\"linearGradient\" attrs=[{ns=xlink prefix=\"xlink\" local=\"href\" value=\"#a\"}]\n",
            "        element ns=svg local=\"foreignObject\" attrs=[]\n",
            "          element ns=html local=\"div\" attrs=[{ns=none prefix=- local=\"disabled\" value=\"\"}]\n",
            "            \"h\"\n",
            "      element ns=mathml local=\"math\" attrs=[]\n",
            "        element ns=mathml local=\"mi\" attrs=[]\n",
            "          \"x\"\n",
            "      element ns=html local=\"p\" attrs=[]\n",
            "        \"after\""
        )
    );
}

#[test]
fn table_foster_parenting_preserves_algorithm_selected_foreign_namespace() {
    let source = "<table><svg><lineargradient/></svg></table><p>after</p>";
    let (patches, errors) = parse(&[source]);
    let chunked = parse(&[
        "<table><sv",
        "g><lineargrad",
        "ient/></svg></ta",
        "ble><p>after</p>",
    ]);
    assert_eq!(chunked.0, patches);
    assert_eq!(chunked.1, errors);
    assert_all_ascii_chunk_boundaries(source);

    let key_for = |namespace, local| {
        patches
            .iter()
            .find_map(|patch| match patch {
                DomPatch::CreateElement { key, name, .. } if name.is(namespace, local) => {
                    Some(*key)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {namespace:?} {local}"))
    };
    let body = key_for(ElementNamespace::Html, "body");
    let table = key_for(ElementNamespace::Html, "table");
    let svg = key_for(ElementNamespace::Svg, "svg");
    let gradient = key_for(ElementNamespace::Svg, "linearGradient");
    assert!(patches.contains(&DomPatch::InsertBefore {
        parent: body,
        child: svg,
        before: table,
    }));
    assert!(patches.contains(&DomPatch::AppendChild {
        parent: svg,
        child: gradient,
    }));
    assert_eq!(
        patches
            .iter()
            .filter(|patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.is(ElementNamespace::Svg, "svg")))
            .count(),
        1,
        "same-token table reprocessing must create the foreign root exactly once"
    );

    let output = parse_document(source, HtmlParseOptions::default()).expect("table foreign parse");
    assert_eq!(
        DomSnapshot::new(&output.document, DomSnapshotOptions::default()).render(),
        concat!(
            "#dom-snapshot-v2\n",
            "#document\n",
            "  element ns=html local=\"html\" attrs=[]\n",
            "    element ns=html local=\"head\" attrs=[]\n",
            "    element ns=html local=\"body\" attrs=[]\n",
            "      element ns=svg local=\"svg\" attrs=[]\n",
            "        element ns=svg local=\"linearGradient\" attrs=[]\n",
            "      element ns=html local=\"table\" attrs=[]\n",
            "      element ns=html local=\"p\" attrs=[]\n",
            "        \"after\""
        )
    );
}

#[test]
fn template_table_foster_target_does_not_select_foreign_namespace() {
    let source = "<template><table><math><mi>x</mi></math></table></template><p>after</p>";
    let (patches, errors) = parse(&[source]);
    let chunked = parse(&[
        "<template><tab",
        "le><ma",
        "th><mi>x</mi></math></table></template><p>after</p>",
    ]);
    assert_eq!(chunked.0, patches);
    assert_eq!(chunked.1, errors);
    assert_all_ascii_chunk_boundaries(source);

    let contents = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateTemplateContents { contents, .. } => Some(*contents),
            _ => None,
        })
        .expect("template contents key");
    let table = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. }
                if name.is(ElementNamespace::Html, "table") =>
            {
                Some(*key)
            }
            _ => None,
        })
        .expect("table key");
    let math = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. }
                if name.is(ElementNamespace::MathMl, "math") =>
            {
                Some(*key)
            }
            _ => None,
        })
        .expect("MathML key");
    assert!(patches.contains(&DomPatch::InsertBefore {
        parent: contents,
        child: math,
        before: table,
    }));
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateElement { name, .. } if name.is(ElementNamespace::MathMl, "mi")
    )));
}

#[test]
fn namespace_declarations_are_passive_ordered_attributes() {
    let cases = [
        (
            "<div xmlns='http://www.w3.org/2000/svg' xmlns:xlink='urn:x'><g></g></div>",
            vec![
                (ElementNamespace::Html, "div"),
                (ElementNamespace::Html, "g"),
            ],
            vec![
                (
                    html::AttributeNamespace::None,
                    None,
                    "xmlns",
                    "http://www.w3.org/2000/svg",
                ),
                (html::AttributeNamespace::None, None, "xmlns:xlink", "urn:x"),
            ],
        ),
        (
            "<svg xmlns='http://www.w3.org/1999/xhtml' xmlns:xlink='urn:x'><g></g></svg>",
            vec![(ElementNamespace::Svg, "svg"), (ElementNamespace::Svg, "g")],
            vec![
                (
                    html::AttributeNamespace::Xmlns,
                    None,
                    "xmlns",
                    "http://www.w3.org/1999/xhtml",
                ),
                (
                    html::AttributeNamespace::Xmlns,
                    Some("xmlns"),
                    "xlink",
                    "urn:x",
                ),
            ],
        ),
        (
            "<math xmlns='http://www.w3.org/2000/svg' xmlns:xlink='urn:x'><mi></mi></math>",
            vec![
                (ElementNamespace::MathMl, "math"),
                (ElementNamespace::MathMl, "mi"),
            ],
            vec![
                (
                    html::AttributeNamespace::Xmlns,
                    None,
                    "xmlns",
                    "http://www.w3.org/2000/svg",
                ),
                (
                    html::AttributeNamespace::Xmlns,
                    Some("xmlns"),
                    "xlink",
                    "urn:x",
                ),
            ],
        ),
        (
            "<svg><g xmlns='http://www.w3.org/1999/xhtml' xmlns:xlink='urn:one'><path xmlns='urn:contradictory'/></g></svg>",
            vec![
                (ElementNamespace::Svg, "svg"),
                (ElementNamespace::Svg, "g"),
                (ElementNamespace::Svg, "path"),
            ],
            vec![
                (
                    html::AttributeNamespace::Xmlns,
                    None,
                    "xmlns",
                    "http://www.w3.org/1999/xhtml",
                ),
                (
                    html::AttributeNamespace::Xmlns,
                    Some("xmlns"),
                    "xlink",
                    "urn:one",
                ),
            ],
        ),
    ];

    for (source, expected_tail, expected_declarations) in cases {
        let whole = parse(&[source]);
        let split = source.len() / 2;
        let chunked = parse(&[&source[..split], &source[split..]]);
        assert_eq!(chunked, whole, "namespace declaration parity for {source}");
        let created = created_elements(&whole.0);
        for (namespace, local) in expected_tail {
            assert!(
                created.contains(&(namespace, local.to_string())),
                "declarations must not alter {namespace:?} {local}"
            );
        }
        assert!(
            !whole.1.iter().any(|error| error
                .detail
                .is_some_and(|detail| detail.contains("namespace"))),
            "passive namespace declarations must not create a namespace-selection diagnostic"
        );

        let declared = whole.0.iter().find_map(|patch| match patch {
            DomPatch::CreateElement { attributes, .. }
                if attributes.iter().any(|attribute| {
                    attribute.local_name() == "xmlns"
                        || attribute.local_name() == "xmlns:xlink"
                        || attribute.prefix() == Some("xmlns")
                }) =>
            {
                Some(attributes)
            }
            _ => None,
        });
        let declared = declared.expect("element with namespace declarations");
        let declarations = declared
            .iter()
            .filter(|attribute| {
                attribute.local_name() == "xmlns"
                    || attribute.local_name() == "xmlns:xlink"
                    || attribute.prefix() == Some("xmlns")
            })
            .map(|attribute| {
                (
                    attribute.namespace(),
                    attribute.prefix(),
                    attribute.local_name(),
                    attribute.value(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            declarations, expected_declarations,
            "stored order for {source}"
        );
        if source.contains("urn:contradictory") {
            let nested = whole.0.iter().find_map(|patch| match patch {
                DomPatch::CreateElement {
                    name, attributes, ..
                } if name.is(ElementNamespace::Svg, "path") => Some(attributes),
                _ => None,
            });
            let nested = nested.expect("nested SVG path");
            assert_eq!(
                nested
                    .iter()
                    .map(|attribute| {
                        (
                            attribute.namespace(),
                            attribute.prefix(),
                            attribute.local_name(),
                            attribute.value(),
                        )
                    })
                    .collect::<Vec<_>>(),
                vec![(
                    html::AttributeNamespace::Xmlns,
                    None,
                    "xmlns",
                    "urn:contradictory",
                )]
            );
        }
    }
}

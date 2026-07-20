use super::*;
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};
use crate::html5::tree_builder::api::TreeBuilderStateSnapshot;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};
use crate::names::ElementNamespace;

struct Run {
    patches: Vec<DomPatch>,
    errors: Vec<&'static str>,
    state: TreeBuilderStateSnapshot,
    open_element_names: Vec<String>,
}

fn run_chunks(chunks: &[&str]) -> Run {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let mut input = Input::new();
    for chunk in chunks {
        input.push_str(chunk);
        loop {
            builder.prepare_tokenizer_pump(&mut tokenizer);
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                assert!(matches!(
                    result,
                    TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                ));
                break;
            }
            let resolver = batch.resolver();
            for token in batch.iter() {
                let result = builder.process(token, &ctx.atoms, &resolver).unwrap();
                if let Some(control) = result.tokenizer_control {
                    tokenizer.apply_control(control);
                }
            }
        }
    }
    builder.prepare_tokenizer_pump(&mut tokenizer);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let result = builder.process(token, &ctx.atoms, &resolver).unwrap();
            if let Some(control) = result.tokenizer_control {
                tokenizer.apply_control(control);
            }
        }
    }
    let state = builder.state_snapshot();
    let open_element_names = state
        .open_element_names
        .iter()
        .map(|name| ctx.atoms.resolve(*name).expect("live SOE atom").to_string())
        .collect();
    Run {
        patches: builder.drain_patches(),
        errors: builder.take_parse_error_kinds_for_test(),
        state,
        open_element_names,
    }
}

fn elements(run: &Run) -> Vec<(ElementNamespace, String)> {
    run.patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateElement { name, .. } => {
                Some((name.namespace(), name.local_name_str().to_string()))
            }
            _ => None,
        })
        .collect()
}

fn materialized_snapshot(patches: &[DomPatch]) -> Vec<String> {
    let dom = crate::test_harness::materialize_patch_batches(&[patches.to_vec()])
        .expect("foreign-content patches must materialize");
    crate::html5::serialize_dom_for_test(&dom)
}

fn assert_run_parity(actual: &Run, expected: &Run, label: &str) {
    assert_eq!(actual.patches, expected.patches, "patches for {label}");
    assert_eq!(actual.errors, expected.errors, "parse errors for {label}");
    assert_eq!(
        actual.open_element_names, expected.open_element_names,
        "semantic SOE names for {label}"
    );
    assert_eq!(
        actual.state.open_element_keys, expected.state.open_element_keys,
        "SOE identities for {label}"
    );
    assert_eq!(
        actual.state.active_formatting_entries, expected.state.active_formatting_entries,
        "AFE state for {label}"
    );
    assert_eq!(
        actual.state.insertion_mode, expected.state.insertion_mode,
        "insertion mode for {label}"
    );
    assert_eq!(
        actual.state.original_insertion_mode, expected.state.original_insertion_mode,
        "original insertion mode for {label}"
    );
    assert_eq!(
        actual.state.table_text_original_insertion_mode,
        expected.state.table_text_original_insertion_mode,
        "table-text original insertion mode for {label}"
    );
    assert_eq!(
        actual.state.frameset_ok, expected.state.frameset_ok,
        "frameset-ok for {label}"
    );
    assert_eq!(
        actual.state.current_table_key, expected.state.current_table_key,
        "current table key for {label}"
    );
    assert_eq!(
        actual.state.pending_table_character_tokens, expected.state.pending_table_character_tokens,
        "pending table text for {label}"
    );
    assert_eq!(
        actual
            .state
            .pending_table_character_tokens_contains_non_space,
        expected
            .state
            .pending_table_character_tokens_contains_non_space,
        "pending table-text classification for {label}"
    );
}

fn assert_all_ascii_chunk_boundaries(source: &str, expected: &Run) {
    assert!(source.is_ascii());
    for boundary in 0..=source.len() {
        let chunked = run_chunks(&[&source[..boundary], &source[boundary..]]);
        assert_run_parity(&chunked, expected, &format!("byte boundary {boundary}"));
    }
}

fn created_element_key(run: &Run, namespace: ElementNamespace, local: &str) -> PatchKey {
    let keys = run
        .patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. } if name.is(namespace, local) => Some(*key),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 1, "exactly one {namespace:?} {local} element");
    keys[0]
}

#[test]
fn complete_foreign_tables_are_sorted_unique_and_adjust_every_pinned_entry() {
    for table in [
        &SVG_TAG_NAME_ADJUSTMENTS[..],
        &SVG_ATTRIBUTE_ADJUSTMENTS[..],
    ] {
        assert!(table.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(table.iter().all(|(source, adjusted)| source != adjusted));
    }
    assert_eq!(SVG_TAG_NAME_ADJUSTMENTS.len(), 37);
    assert_eq!(SVG_ATTRIBUTE_ADJUSTMENTS.len(), 58);
    assert_eq!(FOREIGN_BREAKOUT_START_TAGS.len(), 44);
    assert_eq!(QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS.len(), 12);
    assert!(
        FOREIGN_BREAKOUT_START_TAGS
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    for &(source, adjusted) in &SVG_TAG_NAME_ADJUSTMENTS {
        assert_eq!(svg_adjusted_tag_name(source), adjusted);
    }
    for &(source, adjusted) in &SVG_ATTRIBUTE_ADJUSTMENTS {
        assert_eq!(svg_adjusted_attribute_name(source), adjusted);
    }
    for name in FOREIGN_BREAKOUT_START_TAGS {
        assert!(is_foreign_breakout_start(name));
    }
    assert_eq!(
        QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS
            .iter()
            .map(|(source, _)| *source)
            .collect::<Vec<_>>(),
        vec![
            "xlink:actuate",
            "xlink:arcrole",
            "xlink:href",
            "xlink:role",
            "xlink:show",
            "xlink:title",
            "xlink:type",
            "xml:base",
            "xml:lang",
            "xml:space",
            "xmlns",
            "xmlns:xlink",
        ]
    );
    assert!(
        QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS
            .windows(2)
            .all(|pair| pair[0].0 < pair[1].0)
    );
    for (source, adjustment) in QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS {
        assert_eq!(
            qualified_foreign_attribute_adjustment(source),
            Some(adjustment)
        );
    }
    assert_eq!(svg_adjusted_tag_name("unknown"), "unknown");
    assert_eq!(svg_adjusted_attribute_name("unknown"), "unknown");
}

#[test]
fn foreign_special_elements_drive_production_adoption_agency_recovery() {
    for (source, chunks, expected_foreign) in [
        (
            "<b><svg><title>x</b></svg><p>after</p>",
            ["<b><svg><ti", "tle>x</b></s", "vg><p>after</p>"],
            (ElementNamespace::Svg, "title"),
        ),
        (
            "<b><math><mi>x</b></math><p>after</p>",
            ["<b><math><", "mi>x</b></ma", "th><p>after</p>"],
            (ElementNamespace::MathMl, "mi"),
        ),
    ] {
        let whole = run_chunks(&[source]);
        let chunked = run_chunks(&chunks);
        assert_eq!(chunked.patches, whole.patches, "patch order for {source}");
        assert_eq!(chunked.errors, whole.errors, "parse errors for {source}");
        assert_eq!(
            chunked.open_element_names, whole.open_element_names,
            "semantic SOE names for {source}"
        );
        assert_eq!(
            chunked.state.open_element_keys, whole.state.open_element_keys,
            "SOE identities for {source}"
        );
        assert_eq!(
            chunked.state.active_formatting_entries, whole.state.active_formatting_entries,
            "AFE state for {source}"
        );
        assert_eq!(chunked.state.insertion_mode, whole.state.insertion_mode);
        assert_eq!(chunked.state.frameset_ok, whole.state.frameset_ok);
        assert!(
            elements(&whole).contains(&(expected_foreign.0, expected_foreign.1.to_string())),
            "foreign special element must be constructed for {source}"
        );
        assert_eq!(
            whole
                .errors
                .iter()
                .filter(|error| **error == "foreign-end-tag-current-node-mismatch")
                .count(),
            2,
            "the formatting close and subsequent foreign-boundary close each have one mismatch diagnostic"
        );
        assert_eq!(whole.open_element_names, ["html", "body", "b"]);
        assert_eq!(whole.state.active_formatting_entries.len(), 1);
        assert!(
            materialized_snapshot(&whole.patches)
                .iter()
                .any(|line| line.contains("local=\"p\"") && line.contains("ns=html")),
            "ordinary HTML processing must be restored after recovery"
        );
    }

    let source = "<b><svg><mi>x</b></svg><p>after</p>";
    let whole = run_chunks(&[source]);
    let selected_chunked = run_chunks(&["<b><sv", "g><mi>x</", "b></sv", "g><p>after</p>"]);
    assert_run_parity(&selected_chunked, &whole, "selected wrong-namespace chunks");
    assert_all_ascii_chunk_boundaries(source, &whole);

    assert_eq!(
        elements(&whole),
        vec![
            (ElementNamespace::Html, "html".to_string()),
            (ElementNamespace::Html, "head".to_string()),
            (ElementNamespace::Html, "body".to_string()),
            (ElementNamespace::Html, "b".to_string()),
            (ElementNamespace::Svg, "svg".to_string()),
            (ElementNamespace::Svg, "mi".to_string()),
            (ElementNamespace::Html, "p".to_string()),
        ],
        "SVG mi must retain SVG identity and must not be recreated"
    );
    assert_eq!(
        whole.errors,
        vec![
            "initial-unexpected-token",
            "before-html-implicit-html",
            "before-head-implicit-head",
            "after-head-implicit-body",
            "foreign-end-tag-current-node-mismatch",
            "adoption-agency-formatting-element-not-current-node",
            "in-body-any-other-end-tag-blocked-by-special",
        ]
    );
    assert_eq!(whole.open_element_names, ["html", "body"]);
    assert_eq!(
        whole.state.open_element_keys,
        [PatchKey(2), PatchKey(4)],
        "recovery must leave the canonical html/body stack identities"
    );
    assert!(whole.state.active_formatting_entries.is_empty());
    assert_eq!(
        whole.state.insertion_mode,
        crate::html5::tree_builder::modes::InsertionMode::InBody
    );
    assert_eq!(whole.state.original_insertion_mode, None);
    assert_eq!(whole.state.table_text_original_insertion_mode, None);
    assert!(!whole.state.frameset_ok);
    assert_eq!(whole.state.current_table_key, None);
    assert!(whole.state.pending_table_character_tokens.is_empty());
    assert!(
        !whole
            .state
            .pending_table_character_tokens_contains_non_space
    );

    let b_key = created_element_key(&whole, ElementNamespace::Html, "b");
    let svg_key = created_element_key(&whole, ElementNamespace::Svg, "svg");
    let mi_key = created_element_key(&whole, ElementNamespace::Svg, "mi");
    let p_key = created_element_key(&whole, ElementNamespace::Html, "p");
    for (parent, child) in [(b_key, svg_key), (svg_key, mi_key), (PatchKey(4), p_key)] {
        assert!(
            whole
                .patches
                .iter()
                .any(|patch| matches!(patch, DomPatch::AppendChild { parent: actual_parent, child: actual_child } if (*actual_parent, *actual_child) == (parent, child))),
            "missing expected append {parent:?} -> {child:?}"
        );
    }
    assert_eq!(
        materialized_snapshot(&whole.patches),
        vec![
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"b\" attrs=[]".to_string(),
            "        element ns=svg local=\"svg\" attrs=[]".to_string(),
            "          element ns=svg local=\"mi\" attrs=[]".to_string(),
            "            \"x\"".to_string(),
            "      element ns=html local=\"p\" attrs=[]".to_string(),
            "        \"after\"".to_string(),
        ],
        "wrong-namespace recovery must preserve DOM ancestry and restore HTML"
    );
}

#[test]
fn table_delegation_reprocesses_foreign_entry_once_and_restores_stack_state() {
    for (source, chunks, namespace, adjusted_local) in [
        (
            "<table><svg><lineargradient/></svg></table><p>after</p>",
            [
                "<table><sv",
                "g><lineargrad",
                "ient/></svg></table><p>after</p>",
            ],
            ElementNamespace::Svg,
            "linearGradient",
        ),
        (
            "<template><table><math><mi>x</mi></math></table></template><p>after</p>",
            [
                "<template><table><ma",
                "th><mi>x</mi></math></tab",
                "le></template><p>after</p>",
            ],
            ElementNamespace::MathMl,
            "mi",
        ),
    ] {
        let whole = run_chunks(&[source]);
        let chunked = run_chunks(&chunks);
        assert_eq!(chunked.patches, whole.patches);
        assert_eq!(chunked.errors, whole.errors);
        assert_eq!(chunked.open_element_names, whole.open_element_names);
        assert_eq!(
            chunked.state.open_element_keys,
            whole.state.open_element_keys
        );
        assert_eq!(
            chunked.state.active_formatting_entries,
            whole.state.active_formatting_entries
        );
        assert_eq!(whole.open_element_names, ["html", "body"]);
        assert!(whole.state.active_formatting_entries.is_empty());
        assert_eq!(whole.state.current_table_key, None);
        assert_eq!(
            elements(&whole)
                .iter()
                .filter(|(actual_namespace, local)| {
                    *actual_namespace == namespace && local == adjusted_local
                })
                .count(),
            1,
            "table delegation must make deterministic progress without duplicate insertion"
        );
    }
}

#[test]
fn complete_qualified_foreign_table_and_mathml_adjustment_preserve_order() {
    let source = concat!(
        "<svg xlink:actuate='1' xlink:arcrole='2' xlink:href='3' xlink:role='4' ",
        "xlink:show='5' xlink:title='6' xlink:type='7' xml:base='8' xml:lang='9' ",
        "xml:space='10' xmlns='11' xmlns:xlink='12'></svg>",
        "<math definitionurl='13'></math>"
    );
    let run = run_chunks(&[source]);
    let svg_attributes = run
        .patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement {
                name, attributes, ..
            } if name.is(ElementNamespace::Svg, "svg") => Some(attributes),
            _ => None,
        })
        .expect("SVG root attributes");
    assert_eq!(
        svg_attributes
            .iter()
            .map(|attribute| (
                attribute.namespace().snapshot_name(),
                attribute.prefix(),
                attribute.local_name(),
                attribute.value(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("xlink", Some("xlink"), "actuate", "1"),
            ("xlink", Some("xlink"), "arcrole", "2"),
            ("xlink", Some("xlink"), "href", "3"),
            ("xlink", Some("xlink"), "role", "4"),
            ("xlink", Some("xlink"), "show", "5"),
            ("xlink", Some("xlink"), "title", "6"),
            ("xlink", Some("xlink"), "type", "7"),
            ("xml", Some("xml"), "base", "8"),
            ("xml", Some("xml"), "lang", "9"),
            ("xml", Some("xml"), "space", "10"),
            ("xmlns", None, "xmlns", "11"),
            ("xmlns", Some("xmlns"), "xlink", "12"),
        ]
    );
    let math_attributes = run
        .patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement {
                name, attributes, ..
            } if name.is(ElementNamespace::MathMl, "math") => Some(attributes),
            _ => None,
        })
        .expect("MathML root attributes");
    assert_eq!(math_attributes[0].local_name(), "definitionURL");
    assert_eq!(math_attributes[0].value(), "13");
}

#[test]
fn synthetic_post_adjustment_attribute_collision_is_first_wins_internal_hardening() {
    use crate::html5::shared::{Attribute, AttributeValue, Token};
    use crate::html5::tokenizer::{TextResolveError, TextResolver};

    struct NoSpans;
    impl TextResolver for NoSpans {
        fn resolve_span(
            &self,
            span: crate::html5::shared::TextSpan,
        ) -> Result<&str, TextResolveError> {
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let svg = ctx.atoms.intern_ascii_folded("svg").unwrap();
    let g = ctx.atoms.intern_ascii_folded("g").unwrap();
    let folded = ctx.atoms.intern_ascii_folded("viewbox").unwrap();
    let canonical = ctx.atoms.intern_exact("viewBox").unwrap();
    let resolver = NoSpans;

    let _ = builder
        .process(
            &Token::StartTag {
                name: svg,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();
    let visible_errors_before = builder.take_parse_error_kinds_for_test();
    let _ = builder
        .process(
            &Token::StartTag {
                name: g,
                attrs: vec![
                    Attribute {
                        name: folded,
                        value: AttributeValue::Owned("first".to_string()),
                    },
                    Attribute {
                        name: canonical,
                        value: AttributeValue::Owned("second".to_string()),
                    },
                ],
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();

    assert_eq!(builder.post_adjustment_attribute_collision_count(), 1);
    assert!(builder.take_parse_error_kinds_for_test().is_empty());
    assert!(
        !visible_errors_before.is_empty(),
        "the pre-existing missing-doctype diagnostic is intentionally separate"
    );
    let attributes = builder
        .drain_patches()
        .into_iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement {
                name, attributes, ..
            } if name.is(ElementNamespace::Svg, "g") => Some(attributes),
            _ => None,
        })
        .expect("synthetic SVG g patch");
    assert_eq!(attributes.len(), 1);
    assert_eq!(attributes[0].local_name(), "viewBox");
    assert_eq!(attributes[0].value(), "first");
}

#[test]
fn foreign_character_frameset_transitions_are_exact_and_cdata_uses_the_same_path() {
    for source in ["<svg> </svg>", "<math> </math>", "<svg><![CDATA[ ]]></svg>"] {
        assert!(run_chunks(&[source]).state.frameset_ok, "{source}");
    }
    for source in ["<svg>x</svg>", "<math>x</math>", "<svg><![CDATA[x]]></svg>"] {
        assert!(!run_chunks(&[source]).state.frameset_ok, "{source}");
    }
    let null = run_chunks(&["<svg>\0</svg>"]);
    assert!(
        null.state.frameset_ok,
        "null replacement alone must not change frameset-ok"
    );
    assert_eq!(
        null.errors
            .iter()
            .filter(|error| **error == "unexpected-null-character-in-foreign-content")
            .count(),
        1
    );
    assert!(
        null.patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { text, .. } if text == "\u{FFFD}"))
    );
}

#[test]
fn foreign_character_and_cdata_state_is_chunk_invariant() {
    for (whole, chunks) in [
        ("<svg> </svg>", vec!["<sv", "g> ", "</svg>"]),
        ("<svg>x</svg>", vec!["<svg>", "x</s", "vg>"]),
        (
            "<svg><![CDATA[ ]]></svg>",
            vec!["<svg><![C", "DATA[ ", "]]></svg>"],
        ),
        (
            "<svg><![CDATA[x]]></svg>",
            vec!["<svg><![CDATA[", "x]", "]></svg>"],
        ),
    ] {
        let a = run_chunks(&[whole]);
        let b = run_chunks(&chunks);
        assert_eq!(a.patches, b.patches, "{whole}");
        assert_eq!(a.errors, b.errors, "{whole}");
        assert_eq!(a.state.frameset_ok, b.state.frameset_ok, "{whole}");
    }
}

#[test]
fn foreign_comments_insert_and_doctypes_report_then_ignore() {
    let run = run_chunks(&["<svg><!--kept--><!DOCTYPE html><g></g></svg>"]);

    assert!(
        run.patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateComment { text, .. } if text == "kept"))
    );
    assert!(
        !run.patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateDocumentType { .. }))
    );
    assert_eq!(
        run.errors
            .iter()
            .filter(|error| **error == "doctype-in-foreign-content")
            .count(),
        1
    );
}

#[test]
fn foreign_end_tag_diagnostics_follow_the_single_mismatch_rule() {
    let matching = run_chunks(&["<svg><g></g></svg>"]);
    assert_eq!(
        matching
            .errors
            .iter()
            .filter(|error| **error == "foreign-end-tag-current-node-mismatch")
            .count(),
        0
    );

    let deeper = run_chunks(&["<svg><g><path></g></svg>"]);
    assert_eq!(
        deeper
            .errors
            .iter()
            .filter(|error| **error == "foreign-end-tag-current-node-mismatch")
            .count(),
        1
    );

    let html_boundary = run_chunks(&["<svg><g></span></g></svg>"]);
    assert_eq!(
        html_boundary
            .errors
            .iter()
            .filter(|error| **error == "foreign-end-tag-current-node-mismatch")
            .count(),
        1
    );
    assert!(
        !html_boundary
            .errors
            .iter()
            .any(|error| error.contains("boundary"))
    );
    assert!(elements(&html_boundary).contains(&(ElementNamespace::Svg, "g".to_string())));
}

#[test]
fn spelling_alone_never_switches_namespace() {
    let cases = [
        ("<svg><math></math></svg>", (ElementNamespace::Svg, "math")),
        (
            "<math><svg></svg></math>",
            (ElementNamespace::MathMl, "svg"),
        ),
        (
            "<math><annotation-xml><svg></svg></annotation-xml></math>",
            (ElementNamespace::Svg, "svg"),
        ),
        ("<svg><svg></svg></svg>", (ElementNamespace::Svg, "svg")),
        (
            "<math><math></math></math>",
            (ElementNamespace::MathMl, "math"),
        ),
    ];
    for (source, expected) in cases {
        let run = run_chunks(&[source]);
        assert!(
            elements(&run).contains(&(expected.0, expected.1.to_string())),
            "{source}"
        );
    }
    for source in [
        "<math><mi><mglyph></mglyph></mi></math>",
        "<math><mi><malignmark></malignmark></mi></math>",
    ] {
        let run = run_chunks(&[source]);
        let local = if source.contains("mglyph") {
            "mglyph"
        } else {
            "malignmark"
        };
        assert!(elements(&run).contains(&(ElementNamespace::MathMl, local.to_string())));
    }
}

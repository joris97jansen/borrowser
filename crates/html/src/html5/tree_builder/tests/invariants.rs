use super::helpers::{EmptyResolver, assert_binding_mismatch_panic};

#[test]
fn tree_builder_rejects_foreign_atom_table() {
    use crate::html5::shared::Token;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let resolver = EmptyResolver;
    let mut owner_ctx = crate::html5::shared::DocumentParseContext::new();
    let mut foreign_ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut owner_ctx,
    )
    .expect("tree builder init");

    let process_panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = builder.process(
            &Token::Eof,
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut foreign_ctx),
            &resolver,
        );
    }))
    .expect_err("process must trip invariant assertion");
    assert_binding_mismatch_panic(process_panic.as_ref(), "process");

    let push_panic = catch_unwind(AssertUnwindSafe(|| {
        let mut out = Vec::new();
        let mut sink = crate::html5::tree_builder::VecPatchSink(&mut out);
        let _ = builder.push_token(
            &Token::Eof,
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut foreign_ctx),
            &resolver,
            &mut sink,
        );
    }))
    .expect_err("push_token must trip invariant assertion");
    assert_binding_mismatch_panic(push_panic.as_ref(), "push_token");

    let recovery_result = builder.process(
        &Token::Eof,
        &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut owner_ctx),
        &resolver,
    );
    assert!(
        recovery_result.is_ok(),
        "builder should remain usable with its bound atom table after rejection"
    );
}

#[cfg(feature = "parser-conformance")]
mod final_audit_tests {
    use super::super::helpers::{EmptyResolver, enter_in_body};
    use crate::attributes::{ParserCreatedAttribute, QualifiedAttributeName};
    use crate::dom_patch::PatchKey;
    use crate::html5::shared::{Attribute, AttributeValue, DocumentParseContext, Token};
    use crate::html5::tree_builder::formatting::{AfeEntry, AfeMarkerKind};
    use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};
    use crate::{ElementNamespace, ExpandedElementName};

    fn audit(
        builder: &Html5TreeBuilder,
        context: &DocumentParseContext,
    ) -> crate::html5::tree_builder::TreeBuilderFinalAudit {
        let mut reserve = |_| Ok(());
        builder
            .final_audit_for_conformance(&context.atoms, &mut reserve)
            .expect("final audit should not allocate in the corruption tests")
    }

    fn builder_in_body() -> (Html5TreeBuilder, DocumentParseContext) {
        let mut context = DocumentParseContext::with_tree_observations_for_test();
        let resolver = EmptyResolver;
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut context)
            .expect("tree builder init");
        let _ = enter_in_body(&mut builder, &mut context, &resolver);
        (builder, context)
    }

    fn process(builder: &mut Html5TreeBuilder, context: &mut DocumentParseContext, token: Token) {
        let _ = builder
            .process(
                &token,
                &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(context),
                &EmptyResolver,
            )
            .expect("production construction should remain recoverable");
    }

    fn formatting_builder() -> (Html5TreeBuilder, DocumentParseContext) {
        let (mut builder, mut context) = builder_in_body();
        let b = context.atoms.intern_ascii_folded("b").expect("b atom");
        let href = context
            .atoms
            .intern_ascii_folded("href")
            .expect("href atom");
        let title = context
            .atoms
            .intern_ascii_folded("title")
            .expect("title atom");
        process(
            &mut builder,
            &mut context,
            Token::StartTag {
                name: b,
                attrs: vec![
                    Attribute {
                        name: href,
                        value: AttributeValue::Owned("one".to_owned()),
                    },
                    Attribute {
                        name: title,
                        value: AttributeValue::Owned("two".to_owned()),
                    },
                ],
                self_closing: false,
            },
        );
        let _ = builder.drain_patches();
        (builder, context)
    }

    #[test]
    fn final_audit_active_formatting_requires_exact_live_attributes_and_unique_keys() {
        enum Mutation {
            Value,
            LocalName,
            Prefix,
            Reorder,
            Remove,
            Extra,
        }

        for mutation in [
            Mutation::Value,
            Mutation::LocalName,
            Mutation::Prefix,
            Mutation::Reorder,
            Mutation::Remove,
            Mutation::Extra,
        ] {
            let (mut builder, mut context) = formatting_builder();
            let replacement = context
                .atoms
                .intern_ascii_folded("replacement")
                .expect("replacement atom");
            let replacement_local = context
                .atoms
                .resolve_local_name(replacement)
                .expect("replacement local");
            let href = context
                .atoms
                .intern_ascii_folded("href")
                .expect("href atom");
            builder
                .active_formatting
                .corrupt_element_for_test(|element| match mutation {
                    Mutation::Value => {
                        let attribute = element.attrs[0].clone();
                        element.attrs[0] = ParserCreatedAttribute::new(
                            attribute.name().clone(),
                            "changed".to_owned(),
                        );
                    }
                    Mutation::LocalName => {
                        let attribute = element.attrs[0].clone();
                        element.attrs[0] = ParserCreatedAttribute::new(
                            QualifiedAttributeName::unqualified(replacement_local.clone()),
                            attribute.value().to_owned(),
                        );
                    }
                    Mutation::Prefix => {
                        let attribute = element.attrs[0].clone();
                        let href_local =
                            context.atoms.resolve_local_name(href).expect("href local");
                        element.attrs[0] = ParserCreatedAttribute::new(
                            QualifiedAttributeName::xlink(href_local),
                            attribute.value().to_owned(),
                        );
                    }
                    Mutation::Reorder => element.attrs.reverse(),
                    Mutation::Remove => {
                        element.attrs.pop();
                    }
                    Mutation::Extra => element.attrs.push(ParserCreatedAttribute::new(
                        QualifiedAttributeName::unqualified(replacement_local.clone()),
                        "extra".to_owned(),
                    )),
                });
            let result = audit(&builder, &context);
            assert!(!result.active_formatting_consistent, "mutation must fail");
        }

        let (mut builder, context) = formatting_builder();
        let (key, live_attrs) = builder
            .active_formatting
            .entries()
            .iter()
            .find_map(|entry| match entry {
                AfeEntry::Element(element) => builder
                    .live_tree
                    .element_semantics_for_final_audit(element.key)
                    .map(|(_, attrs)| (element.key, attrs.to_vec())),
                AfeEntry::Marker(_) => None,
            })
            .expect("production formatting element");
        let _ = key;
        builder
            .active_formatting
            .corrupt_element_for_test(|element| {
                element.attrs.reverse();
                assert!(
                    crate::html5::tree_builder::attributes::same_attributes_for_html_parser(
                        &element.attrs,
                        &live_attrs,
                    )
                );
            });
        let report = audit(&builder, &context);
        assert!(!report.active_formatting_consistent);
        assert!(report.parent_child_links_valid);
        assert!(report.template_associations_valid);

        let (mut builder, mut context) = formatting_builder();
        let i = context.atoms.intern_ascii_folded("i").expect("i atom");
        process(
            &mut builder,
            &mut context,
            Token::StartTag {
                name: i,
                attrs: Vec::new(),
                self_closing: false,
            },
        );
        let first_key = builder
            .active_formatting
            .entries()
            .iter()
            .find_map(|entry| match entry {
                AfeEntry::Element(element) => Some(element.key),
                AfeEntry::Marker(_) => None,
            })
            .expect("first formatting element");
        builder
            .active_formatting
            .corrupt_element_for_test(|element| {
                element.key = first_key;
            });
        assert!(!audit(&builder, &context).active_formatting_consistent);
    }

    fn marker_builder(kind: AfeMarkerKind) -> (Html5TreeBuilder, DocumentParseContext, PatchKey) {
        let (mut builder, mut context) = builder_in_body();
        match kind {
            AfeMarkerKind::FormattingBoundary => {
                let applet = context
                    .atoms
                    .intern_ascii_folded("applet")
                    .expect("applet atom");
                process(
                    &mut builder,
                    &mut context,
                    Token::StartTag {
                        name: applet,
                        attrs: Vec::new(),
                        self_closing: false,
                    },
                );
            }
            AfeMarkerKind::Caption => {
                let table = context
                    .atoms
                    .intern_ascii_folded("table")
                    .expect("table atom");
                let caption = context
                    .atoms
                    .intern_ascii_folded("caption")
                    .expect("caption atom");
                process(
                    &mut builder,
                    &mut context,
                    Token::StartTag {
                        name: table,
                        attrs: Vec::new(),
                        self_closing: false,
                    },
                );
                process(
                    &mut builder,
                    &mut context,
                    Token::StartTag {
                        name: caption,
                        attrs: Vec::new(),
                        self_closing: false,
                    },
                );
            }
            AfeMarkerKind::TableCell => {
                for name in ["table", "tbody", "tr", "td"] {
                    let name = context.atoms.intern_ascii_folded(name).expect("table atom");
                    process(
                        &mut builder,
                        &mut context,
                        Token::StartTag {
                            name,
                            attrs: Vec::new(),
                            self_closing: false,
                        },
                    );
                }
            }
            AfeMarkerKind::Template => {
                let template = context
                    .atoms
                    .intern_ascii_folded("template")
                    .expect("template atom");
                process(
                    &mut builder,
                    &mut context,
                    Token::StartTag {
                        name: template,
                        attrs: Vec::new(),
                        self_closing: false,
                    },
                );
            }
        }
        let owner = builder
            .active_formatting
            .entries()
            .iter()
            .rev()
            .find_map(|entry| match entry {
                AfeEntry::Marker(marker) if marker.kind == kind => marker.owner,
                _ => None,
            })
            .expect("production marker owner");
        let _ = builder.drain_patches();
        (builder, context, owner)
    }

    #[test]
    fn final_audit_validates_each_marker_owner_kind_and_namespace() {
        for kind in [
            AfeMarkerKind::FormattingBoundary,
            AfeMarkerKind::Caption,
            AfeMarkerKind::TableCell,
            AfeMarkerKind::Template,
        ] {
            for corruption in 0..5 {
                let (mut builder, context, owner) = marker_builder(kind);
                match corruption {
                    0 => builder
                        .active_formatting
                        .corrupt_marker_owner_for_test(kind, None),
                    1 => builder
                        .active_formatting
                        .corrupt_marker_owner_for_test(kind, Some(PatchKey::INVALID)),
                    2 => {
                        let body = builder
                            .state_snapshot()
                            .open_element_keys
                            .into_iter()
                            .find(|key| {
                                builder
                                    .live_tree
                                    .element_semantics_for_final_audit(*key)
                                    .is_some_and(|(name, _)| name.is_html("body"))
                            })
                            .expect("body owner");
                        builder
                            .active_formatting
                            .corrupt_marker_owner_for_test(kind, Some(body));
                    }
                    3 => {
                        builder
                            .active_formatting
                            .corrupt_marker_owner_for_test(kind, builder.document_key);
                    }
                    4 => {
                        let (name, _) = builder
                            .live_tree
                            .element_semantics_for_final_audit(owner)
                            .expect("marker owner");
                        builder.live_tree.corrupt_element_name_for_test(
                            owner,
                            ExpandedElementName::new(
                                ElementNamespace::Svg,
                                name.local_name().clone(),
                            ),
                        );
                    }
                    _ => unreachable!(),
                }
                assert!(
                    !audit(&builder, &context).active_formatting_consistent,
                    "marker corruption {corruption} for {kind:?} must fail"
                );
            }
        }
    }

    fn nested_template_builder() -> (
        Html5TreeBuilder,
        DocumentParseContext,
        PatchKey,
        PatchKey,
        PatchKey,
    ) {
        let (mut builder, mut context) = builder_in_body();
        let template = context
            .atoms
            .intern_ascii_folded("template")
            .expect("template atom");
        process(
            &mut builder,
            &mut context,
            Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
        );
        let outer = builder
            .template_modes
            .current()
            .expect("outer mode")
            .owner();
        process(
            &mut builder,
            &mut context,
            Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
        );
        let closed = builder
            .template_modes
            .current()
            .expect("inner mode")
            .owner();
        process(&mut builder, &mut context, Token::EndTag { name: template });
        process(
            &mut builder,
            &mut context,
            Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
        );
        let open = builder
            .template_modes
            .current()
            .expect("second mode")
            .owner();
        let _ = builder.drain_patches();
        (builder, context, outer, closed, open)
    }

    #[test]
    fn final_audit_compares_open_template_ordinals_and_ignores_valid_residual_markers() {
        let (mut builder, context, _, _, _) = nested_template_builder();
        builder
            .active_formatting
            .remove_template_marker_at_for_test(1);
        assert!(!audit(&builder, &context).template_modes_consistent);

        let (mut builder, context, outer, _, _) = nested_template_builder();
        builder
            .active_formatting
            .insert_template_marker_at_for_test(2, outer);
        assert!(!audit(&builder, &context).template_modes_consistent);

        let (mut builder, context, _, _, _) = nested_template_builder();
        builder
            .active_formatting
            .swap_template_markers_for_test(0, 1);
        assert!(!audit(&builder, &context).template_modes_consistent);

        let (mut builder, context, _, _, _) = nested_template_builder();
        builder
            .template_modes
            .corrupt_current_owner_for_test(PatchKey::INVALID);
        assert!(!audit(&builder, &context).template_modes_consistent);

        let (mut builder, context, _, _, _) = nested_template_builder();
        builder.template_modes.swap_entries_for_test(0, 1);
        assert!(!audit(&builder, &context).template_modes_consistent);

        for position in 0..=2 {
            let (mut builder, context, _, closed, _) = nested_template_builder();
            builder
                .active_formatting
                .insert_template_marker_at_for_test(position, closed);
            assert!(audit(&builder, &context).template_modes_consistent);
        }
    }

    #[test]
    fn final_audit_rejects_template_hosts_with_ordinary_children() {
        let (mut builder, mut context) = builder_in_body();
        let template = context
            .atoms
            .intern_ascii_folded("template")
            .expect("template atom");
        let div = context.atoms.intern_ascii_folded("div").expect("div atom");
        process(
            &mut builder,
            &mut context,
            Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
        );
        let host = builder
            .template_modes
            .current()
            .expect("template mode")
            .owner();
        let div_name = ExpandedElementName::new(
            ElementNamespace::Html,
            context.atoms.resolve_local_name(div).expect("div local"),
        );
        builder
            .live_tree
            .corrupt_template_host_with_ordinary_child_for_test(host, div_name);
        assert!(!audit(&builder, &context).template_associations_valid);
    }
}

use super::{
    EmptyResolver, Html5TreeBuilder, InsertionLocation, attach_live_table, bootstrap_html_body,
};
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::DocumentParseContext;
use crate::html5::tree_builder::stack::OpenElement;

#[test]
fn foster_parenting_location_uses_live_table_parent_and_before_key() {
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let (_html, body) = bootstrap_html_body(&mut builder, &mut ctx);
    let table = attach_live_table(&mut builder, &mut ctx, body);

    assert_eq!(
        builder
            .foster_parenting_insertion_location()
            .expect("foster location"),
        InsertionLocation {
            parent: body,
            before: Some(table),
        }
    );
}

#[test]
fn foster_parenting_location_uses_previous_soe_entry_for_detached_table() {
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let (_html, body) = bootstrap_html_body(&mut builder, &mut ctx);
    let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx);
    let atoms = context.atoms();
    builder
        .with_structural_mutation(|this| {
            let table = this
                .create_detached_element(this.known_tags.table, &[], &mut context, atoms)?
                .expect("table setup should not hit resource limits");
            this.open_elements
                .push(OpenElement::new_html(table, this.known_tags.table));
            assert_eq!(
                this.foster_parenting_insertion_location()?,
                InsertionLocation {
                    parent: body,
                    before: None,
                }
            );
            Ok(())
        })
        .expect("detached foster-parent computation should remain recoverable");
}

#[test]
fn foster_parenting_location_prefers_template_above_table() {
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let (_html, body) = bootstrap_html_body(&mut builder, &mut ctx);
    let _table = attach_live_table(&mut builder, &mut ctx, body);
    let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx);
    let atoms = context.atoms();
    builder
        .with_structural_mutation(|this| {
            let template = this
                .create_detached_element(this.known_tags.template, &[], &mut context, atoms)?
                .expect("template setup should not hit resource limits");
            let contents = this.alloc_patch_key()?;
            this.push_structural_patch(DomPatch::CreateTemplateContents {
                host: template,
                contents,
            });
            this.note_node_created();
            this.open_elements
                .push(OpenElement::new_html(template, this.known_tags.template));
            assert_eq!(
                this.foster_parenting_insertion_location()?,
                InsertionLocation {
                    parent: contents,
                    before: None,
                }
            );
            Ok(())
        })
        .expect("template-preferred foster-parent computation should remain recoverable");
}

#[test]
fn foster_parenting_text_insertion_uses_insert_before_for_live_table() {
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let (_html, body) = bootstrap_html_body(&mut builder, &mut ctx);
    let table = attach_live_table(&mut builder, &mut ctx, body);
    let _ = builder.drain_patches();
    builder.foster_parenting_enabled = true;

    builder
        .insert_literal_text_for_test("x", &mut ctx)
        .expect("foster-parent text insertion should remain recoverable");
    let patches = builder.drain_patches();

    assert_eq!(
        patches,
        vec![
            DomPatch::CreateText {
                key: PatchKey(5),
                text: "x".to_string(),
            },
            DomPatch::InsertBefore {
                parent: body,
                child: PatchKey(5),
                before: table,
            },
        ]
    );
}

#[test]
fn foster_parenting_element_insertion_uses_insert_before_for_live_table() {
    let resolver = EmptyResolver;
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let (_html, body) = bootstrap_html_body(&mut builder, &mut ctx);
    let table = attach_live_table(&mut builder, &mut ctx, body);
    let _ = builder.drain_patches();
    builder.foster_parenting_enabled = true;
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");

    let inserted = builder
        .insert_normal_html_element_for_test(div, &[], &mut ctx, &resolver)
        .expect("foster-parent element insertion should remain recoverable")
        .expect("foster-parent element insertion should not hit resource limits");
    let patches = builder.drain_patches();

    assert_eq!(inserted, PatchKey(5));
    assert_eq!(
        patches,
        vec![
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: crate::test_support::html_name("div"),
                attributes: Vec::new(),
            },
            DomPatch::InsertBefore {
                parent: body,
                child: PatchKey(5),
                before: table,
            },
        ]
    );
}

#[test]
fn foster_parenting_reparents_existing_nodes_with_insert_before_only() {
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let (_html, body) = bootstrap_html_body(&mut builder, &mut ctx);
    let table = attach_live_table(&mut builder, &mut ctx, body);
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx);
    let atoms = context.atoms();
    let (container, child) = builder
        .with_structural_mutation(|this| {
            let div_key = this
                .create_detached_element(div, &[], &mut context, atoms)?
                .expect("div setup should not hit resource limits");
            this.append_existing_child(body, div_key, &mut context);
            let span_key = this
                .create_detached_element(span, &[], &mut context, atoms)?
                .expect("span setup should not hit resource limits");
            this.append_existing_child(div_key, span_key, &mut context);
            Ok((div_key, span_key))
        })
        .expect("existing child setup should remain recoverable");
    let _ = builder.drain_patches();

    builder
        .with_structural_mutation(|this| {
            this.insert_existing_child_using_foster_parenting_location(child, &mut context)
        })
        .expect("existing child foster-parent move should remain recoverable");
    let patches = builder.drain_patches();

    assert_eq!(
        patches,
        vec![DomPatch::InsertBefore {
            parent: body,
            child,
            before: table,
        }]
    );
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "foster-parent reparenting must use canonical InsertBefore move encoding"
    );
    assert_eq!(builder.live_tree.parent(child), Some(body));
    assert_eq!(
        builder.live_tree.children_snapshot(container),
        Vec::<PatchKey>::new()
    );
    assert_eq!(
        builder.live_tree.children_snapshot(body),
        vec![child, table, container]
    );
}

use super::helpers::{EmptyResolver, materialized_dom_lines, run_tree_builder_chunks};
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, ProcessingInstructionToken, TextValue, Token};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn pi_parents(patches: &[DomPatch]) -> HashMap<String, PatchKey> {
    let mut pi_keys = HashMap::new();
    let mut parents = HashMap::new();
    for patch in patches {
        match patch {
            DomPatch::CreateProcessingInstruction { key, target, .. } => {
                pi_keys.insert(*key, target.clone());
            }
            DomPatch::AppendChild { parent, child }
            | DomPatch::InsertBefore {
                parent,
                child,
                before: _,
            } => {
                parents.insert(*child, *parent);
            }
            _ => {}
        }
    }
    pi_keys
        .into_iter()
        .map(|(key, target)| (target, parents[&key]))
        .collect()
}

fn element_keys(patches: &[DomPatch]) -> HashMap<String, Vec<PatchKey>> {
    let mut keys: HashMap<String, Vec<PatchKey>> = HashMap::new();
    for patch in patches {
        if let DomPatch::CreateElement { key, name, .. } = patch {
            keys.entry(name.local_name().as_str().to_string())
                .or_default()
                .push(*key);
        }
    }
    keys
}

#[test]
fn processing_instructions_preserve_document_head_body_and_after_body_placement() {
    let patches = run_tree_builder_chunks(&[concat!(
        "<?pre?>",
        "<!doctype html>",
        "<?between?>",
        "<html><?before_head?>",
        "<head><?in_head?></head>",
        "<?after_head?>",
        "<body><?in_body?></body>",
        "<?after_body?></html>",
        "<?after_after?>"
    )]);
    let parents = pi_parents(&patches);
    let elements = element_keys(&patches);
    let document = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateDocument { key, .. } => Some(*key),
            _ => None,
        })
        .unwrap();
    let html = elements["html"][0];
    let head = elements["head"][0];
    let body = elements["body"][0];
    assert_eq!(parents["pre"], document);
    assert_eq!(parents["between"], document);
    assert_eq!(parents["before_head"], html);
    assert_eq!(parents["in_head"], head);
    assert_eq!(parents["after_head"], html);
    assert_eq!(parents["in_body"], body);
    assert_eq!(parents["after_body"], html);
    assert_eq!(parents["after_after"], document);
}

#[test]
fn in_table_pi_is_not_foster_parented_and_table_text_flushes_before_reprocessing() {
    let patches = run_tree_builder_chunks(&[
        "<body><table>x<?table?><caption><?caption?></caption><colgroup><?colgroup?></colgroup>",
        "<tbody><?tbody?><tr><?row?><td><?cell?></td></tr></tbody></table>",
    ]);
    let parents = pi_parents(&patches);
    let elements = element_keys(&patches);
    assert_eq!(parents["table"], elements["table"][0]);
    assert_eq!(parents["caption"], elements["caption"][0]);
    assert_eq!(parents["colgroup"], elements["colgroup"][0]);
    assert_eq!(parents["tbody"], elements["tbody"][0]);
    assert_eq!(parents["row"], elements["tr"][0]);
    assert_eq!(parents["cell"], elements["td"][0]);

    let table_pi_key = patches.iter().find_map(|patch| match patch {
        DomPatch::CreateProcessingInstruction { key, target, .. } if target == "table" => {
            Some(*key)
        }
        _ => None,
    });
    let table_pi_key = table_pi_key.unwrap();
    assert!(patches.iter().any(|patch| {
        matches!(patch, DomPatch::AppendChild { parent, child } if *parent == elements["table"][0] && *child == table_pi_key)
    }));
}

#[test]
fn template_and_foreign_content_use_adjusted_insertion_locations() {
    let template_patches = run_tree_builder_chunks(&["<template><?template?></template>"]);
    let template_parent = pi_parents(&template_patches)["template"];
    let contents = template_patches.iter().find_map(|patch| match patch {
        DomPatch::CreateTemplateContents { contents, .. } => Some(*contents),
        _ => None,
    });
    assert_eq!(Some(template_parent), contents);

    let foreign = run_tree_builder_chunks(&[
        "<svg><?svg?><foreignObject><?integration?></foreignObject></svg>",
        "<math><?math?></math>",
    ]);
    let parents = pi_parents(&foreign);
    let elements = element_keys(&foreign);
    assert_eq!(parents["svg"], elements["svg"][0]);
    assert_eq!(parents["integration"], elements["foreignObject"][0]);
    assert_eq!(parents["math"], elements["math"][0]);
}

#[test]
fn processing_instruction_dom_and_patch_snapshots_escape_deterministically() {
    let patches = run_tree_builder_chunks(&["<body><?Pi a\\\"b\tline?>"]);
    assert!(patches.iter().any(|patch| {
        matches!(patch, DomPatch::CreateProcessingInstruction { target, data, .. } if target == "Pi" && data == "a\\\"b\tline")
    }));
    assert!(
        materialized_dom_lines(&["<body><?Pi a\\\"b\tline?>"])
            .iter()
            .any(|line| line
                .contains("processing-instruction target=\"Pi\" data=\"a\\\\\\\"b\\tline\""))
    );
}

#[test]
fn impossible_text_mode_pi_returns_internal_error_without_mutation() {
    let mut ctx = DocumentParseContext::new();
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    builder.insertion_mode = InsertionMode::Text;
    let before_state = builder.state_snapshot();
    let before_dom = builder.dom_invariant_state();
    let token = Token::ProcessingInstruction(ProcessingInstructionToken {
        target: "pi".into(),
        data: TextValue::Owned("data".into()),
    });
    assert!(builder.process(&token, &ctx.atoms, &EmptyResolver).is_err());
    assert_eq!(builder.state_snapshot(), before_state);
    assert_eq!(builder.dom_invariant_state(), before_dom);
    assert!(builder.drain_patches().is_empty());
    assert!(builder.take_parse_error_kinds_for_test().is_empty());

    let mut direct_context = DocumentParseContext::new();
    let mut direct_builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut direct_context).unwrap();
    direct_builder.insertion_mode = InsertionMode::Text;
    let direct_before_state = direct_builder.state_snapshot();
    let direct_before_dom = direct_builder.dom_invariant_state();
    let bypass = catch_unwind(AssertUnwindSafe(|| {
        let _ = direct_builder.handle_text_mode(&token, &direct_context.atoms, &EmptyResolver);
    }));
    assert!(
        bypass.is_err(),
        "direct Text-mode dispatch must identify that the central preflight was bypassed"
    );
    assert_eq!(direct_builder.state_snapshot(), direct_before_state);
    assert_eq!(direct_builder.dom_invariant_state(), direct_before_dom);
    assert!(direct_builder.drain_patches().is_empty());
    assert!(direct_builder.take_parse_error_kinds_for_test().is_empty());
}

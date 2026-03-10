use super::super::*;
use super::helpers::PatchArena;
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};

#[test]
fn tree_builder_emits_core_patches_matching_dom() {
    let input = "<!doctype html><div>hi<span>yo</span><!--c--></div>";
    let stream = crate::tokenize(input);
    let expected = build_owned_dom(&stream);

    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig::default(),
    );
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let patches = builder.take_patches();
    let _ = builder.materialize().unwrap();

    let mut arena = PatchArena::default();
    arena.apply(&patches);
    let actual = arena.materialize();
    assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
}

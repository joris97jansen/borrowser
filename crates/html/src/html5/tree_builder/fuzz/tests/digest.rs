use super::super::digest::FuzzDigest;
use crate::dom_patch::PatchKey;
use crate::html5::tree_builder::TreeBuilderProgressWitness;
use crate::html5::tree_builder::document::QuirksMode;
use crate::html5::tree_builder::modes::InsertionMode;

fn witness(
    form_element_pointer: Option<PatchKey>,
    pending_textarea_initial_lf: Option<PatchKey>,
) -> TreeBuilderProgressWitness {
    TreeBuilderProgressWitness {
        insertion_mode: InsertionMode::InBody,
        original_insertion_mode: None,
        table_text_original_insertion_mode: None,
        active_text_mode: None,
        form_element_pointer,
        pending_textarea_initial_lf,
        open_element_keys: vec![PatchKey(1), PatchKey(2)],
        current_table_key: None,
        pending_table_character_tokens: Vec::new(),
        pending_table_character_tokens_contains_non_space: false,
        quirks_mode: QuirksMode::NoQuirks,
        frameset_ok: true,
        foster_parenting_enabled: false,
    }
}

fn digest_for(witness: &TreeBuilderProgressWitness) -> u64 {
    let mut digest = FuzzDigest::new(0xAE9A);
    digest.record_future_affecting_state(witness);
    digest.finish()
}

#[test]
fn tree_builder_digest_includes_form_pointer_and_pending_textarea_lf() {
    let baseline = witness(None, None);
    assert_ne!(
        digest_for(&baseline),
        digest_for(&witness(Some(PatchKey(41)), None)),
        "form pointer must participate in the tree-builder fuzz digest"
    );
    assert_ne!(
        digest_for(&baseline),
        digest_for(&witness(None, Some(PatchKey(42)))),
        "pending textarea initial-LF state must participate in the tree-builder fuzz digest"
    );
}

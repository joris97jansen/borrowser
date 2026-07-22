use super::super::digest::{PIPELINE_FUZZ_DIGEST_SCHEMA, PipelineDigestTail, PipelineFuzzDigest};
use crate::DocumentMode;
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::tree_builder::TreeBuilderProgressWitness;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{
    AfeDiagnosticEntry, AfeMarker, AfeMarkerKind, TemplateInsertionMode,
};

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
        head_element_pointer: None,
        template_modes: Vec::new(),
        active_formatting_entries: Vec::new(),
        open_element_keys: vec![PatchKey(1), PatchKey(2)],
        current_table_key: None,
        pending_table_character_tokens: Vec::new(),
        pending_table_character_tokens_contains_non_space: false,
        quirks_mode: DocumentMode::NoQuirks,
        frameset_ok: true,
        foster_parenting_enabled: false,
    }
}

fn digest_for(witness: &TreeBuilderProgressWitness) -> u64 {
    let mut digest = PipelineFuzzDigest::new(0xAE9A);
    digest.record_future_affecting_builder_state(witness);
    digest.finish(PipelineDigestTail {
        token_digest: 0,
        tokens_streamed: 0,
        span_resolve_count: 0,
        patches_emitted: 0,
        tokenizer_controls_applied: 0,
        chunk_count: 0,
        decoded_bytes: 0,
    })
}

fn digest_for_patches(patches: &[DomPatch]) -> u64 {
    let mut digest = PipelineFuzzDigest::new(0xAE12);
    digest.record_patches(patches);
    digest.finish(PipelineDigestTail {
        token_digest: 0,
        tokens_streamed: 0,
        span_resolve_count: 0,
        patches_emitted: patches.len(),
        tokenizer_controls_applied: 0,
        chunk_count: 0,
        decoded_bytes: 0,
    })
}

#[test]
fn pipeline_digest_includes_form_pointer_and_pending_textarea_lf() {
    let baseline = witness(None, None);
    assert_ne!(
        digest_for(&baseline),
        digest_for(&witness(Some(PatchKey(41)), None)),
        "form pointer must participate in the pipeline fuzz digest"
    );
    assert_ne!(
        digest_for(&baseline),
        digest_for(&witness(None, Some(PatchKey(42)))),
        "pending textarea initial-LF state must participate in the pipeline fuzz digest"
    );
}

#[test]
fn pipeline_digest_is_sensitive_to_template_state() {
    assert_eq!(PIPELINE_FUZZ_DIGEST_SCHEMA, 3);
    let baseline = witness(None, None);
    let mut changed = baseline.clone();
    changed.head_element_pointer = Some(PatchKey(9));
    assert_ne!(digest_for(&baseline), digest_for(&changed));
    changed = baseline.clone();
    changed.template_modes = vec![(PatchKey(12), TemplateInsertionMode::InTemplate)];
    assert_ne!(digest_for(&baseline), digest_for(&changed));
    changed = baseline.clone();
    changed.active_formatting_entries = vec![
        AfeDiagnosticEntry::Marker(AfeMarker::new(AfeMarkerKind::Template, Some(PatchKey(12)))),
        AfeDiagnosticEntry::Element(PatchKey(13)),
    ];
    assert_ne!(digest_for(&baseline), digest_for(&changed));
    let mut marker_kind_changed = changed.clone();
    marker_kind_changed.active_formatting_entries[0] =
        AfeDiagnosticEntry::Marker(AfeMarker::new(AfeMarkerKind::Caption, Some(PatchKey(12))));
    assert_ne!(digest_for(&changed), digest_for(&marker_kind_changed));
}

#[test]
fn pipeline_digest_distinguishes_processing_instruction_from_template_contents() {
    let template = DomPatch::CreateTemplateContents {
        host: PatchKey(1),
        contents: PatchKey(2),
    };
    let processing_instruction = DomPatch::CreateProcessingInstruction {
        key: PatchKey(1),
        target: "pi".to_string(),
        data: String::new(),
    };
    assert_ne!(
        digest_for_patches(&[template]),
        digest_for_patches(&[processing_instruction])
    );
}

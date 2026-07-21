use crate::dom_patch::DomPatch;
use crate::html5::tokenizer::{TextModeKind, TokenizerControl};
use crate::html5::tree_builder::AfeDiagnosticEntry;
use crate::html5::tree_builder::TreeBuilderProgressWitness;

pub(super) const PIPELINE_FUZZ_DIGEST_SCHEMA: u8 = 3;

#[derive(Clone, Copy)]
pub(super) struct PipelineFuzzDigest(u64);

pub(super) struct PipelineDigestTail {
    pub(super) token_digest: u64,
    pub(super) tokens_streamed: usize,
    pub(super) span_resolve_count: usize,
    pub(super) patches_emitted: usize,
    pub(super) tokenizer_controls_applied: usize,
    pub(super) chunk_count: usize,
    pub(super) decoded_bytes: usize,
}

impl PipelineFuzzDigest {
    pub(super) fn new(seed: u64) -> Self {
        let mut digest = Self(0xcbf29ce484222325u64 ^ seed.rotate_left(7));
        digest.push_u8(0xfd);
        digest.push_u8(PIPELINE_FUZZ_DIGEST_SCHEMA);
        digest
    }

    pub(super) fn record_chunk_len(&mut self, len: usize) {
        self.push_u8(1);
        self.push_usize(len);
    }

    pub(super) fn record_tokenizer_control(&mut self, control: TokenizerControl) {
        self.push_u8(2);
        match control {
            TokenizerControl::EnterTextMode(spec) => {
                self.push_u8(1);
                self.push_u32(spec.end_tag_name.index());
                self.push_u8(match spec.kind {
                    TextModeKind::RawText => 1,
                    TextModeKind::Rcdata => 2,
                    TextModeKind::ScriptData => 3,
                });
            }
            TokenizerControl::ExitTextMode => {
                self.push_u8(2);
            }
        }
    }

    pub(super) fn record_patches(&mut self, patches: &[DomPatch]) {
        self.push_u8(3);
        self.push_usize(patches.len());
        for patch in patches {
            match patch {
                DomPatch::Clear => self.push_u8(10),
                DomPatch::CreateDocument { key, doctype } => {
                    self.push_u8(11);
                    self.push_u32(key.0);
                    self.push_opt_str(doctype.as_deref());
                }
                DomPatch::CreateDocumentType {
                    key,
                    name,
                    public_id,
                    system_id,
                } => {
                    self.push_u8(21);
                    self.push_u32(key.0);
                    self.push_opt_str(name.as_deref());
                    self.push_opt_str(public_id.as_deref());
                    self.push_opt_str(system_id.as_deref());
                }
                DomPatch::CreateElement {
                    key,
                    name,
                    attributes,
                } => {
                    self.push_u8(12);
                    self.push_u32(key.0);
                    self.push_str(name.namespace().snapshot_name());
                    self.push_str(name.local_name_str());
                    self.push_usize(attributes.len());
                    for attribute in attributes {
                        self.push_str(attribute.namespace().snapshot_name());
                        self.push_opt_str(attribute.prefix());
                        self.push_str(attribute.local_name());
                        self.push_str(attribute.value());
                    }
                }
                DomPatch::CreateTemplateContents { host, contents } => {
                    self.push_u8(22);
                    self.push_u32(host.0);
                    self.push_u32(contents.0);
                }
                DomPatch::CreateText { key, text } => {
                    self.push_u8(13);
                    self.push_u32(key.0);
                    self.push_str(text);
                }
                DomPatch::CreateComment { key, text } => {
                    self.push_u8(14);
                    self.push_u32(key.0);
                    self.push_str(text);
                }
                DomPatch::CreateProcessingInstruction { key, target, data } => {
                    self.push_u8(23);
                    self.push_u32(key.0);
                    self.push_str(target);
                    self.push_str(data);
                }
                DomPatch::AppendChild { parent, child } => {
                    self.push_u8(15);
                    self.push_u32(parent.0);
                    self.push_u32(child.0);
                }
                DomPatch::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    self.push_u8(16);
                    self.push_u32(parent.0);
                    self.push_u32(child.0);
                    self.push_u32(before.0);
                }
                DomPatch::RemoveNode { key } => {
                    self.push_u8(17);
                    self.push_u32(key.0);
                }
                DomPatch::SetAttributes { key, attributes } => {
                    self.push_u8(18);
                    self.push_u32(key.0);
                    self.push_usize(attributes.len());
                    for attribute in attributes {
                        self.push_str(attribute.namespace().snapshot_name());
                        self.push_opt_str(attribute.prefix());
                        self.push_str(attribute.local_name());
                        self.push_str(attribute.value());
                    }
                }
                DomPatch::SetText { key, text } => {
                    self.push_u8(19);
                    self.push_u32(key.0);
                    self.push_str(text);
                }
                DomPatch::AppendText { key, text } => {
                    self.push_u8(20);
                    self.push_u32(key.0);
                    self.push_str(text);
                }
            }
        }
    }

    pub(super) fn record_future_affecting_builder_state(
        &mut self,
        witness: &TreeBuilderProgressWitness,
    ) {
        self.push_u8(5);
        self.push_opt_key(witness.form_element_pointer);
        self.push_opt_key(witness.pending_textarea_initial_lf);
        self.push_opt_key(witness.head_element_pointer);
        self.push_usize(witness.template_modes.len());
        for (owner, mode) in &witness.template_modes {
            self.push_u32(owner.0);
            self.push_u8(mode.digest_tag());
        }
        self.push_usize(witness.active_formatting_entries.len());
        for entry in &witness.active_formatting_entries {
            match entry {
                AfeDiagnosticEntry::Element(key) => {
                    self.push_u8(1);
                    self.push_u32(key.0);
                }
                AfeDiagnosticEntry::Marker(marker) => {
                    self.push_u8(2);
                    self.push_u8(marker.kind.digest_tag());
                    self.push_opt_key(marker.owner);
                }
            }
        }
    }

    pub(super) fn finish(mut self, tail: PipelineDigestTail) -> u64 {
        self.push_u8(4);
        self.push_u64(tail.token_digest);
        self.push_usize(tail.tokens_streamed);
        self.push_usize(tail.span_resolve_count);
        self.push_usize(tail.patches_emitted);
        self.push_usize(tail.tokenizer_controls_applied);
        self.push_usize(tail.chunk_count);
        self.push_usize(tail.decoded_bytes);
        self.0
    }

    fn push_opt_str(&mut self, value: Option<&str>) {
        self.push_u8(u8::from(value.is_some()));
        if let Some(value) = value {
            self.push_str(value);
        }
    }

    fn push_str(&mut self, value: &str) {
        self.push_usize(value.len());
        for &byte in value.as_bytes() {
            self.push_u8(byte);
        }
    }

    fn push_usize(&mut self, value: usize) {
        self.push_u64(value as u64);
    }

    fn push_u32(&mut self, value: u32) {
        self.push_u64(u64::from(value));
    }

    fn push_opt_key(&mut self, value: Option<crate::dom_patch::PatchKey>) {
        self.push_u8(u8::from(value.is_some()));
        if let Some(key) = value {
            self.push_u32(key.0);
        }
    }

    fn push_u8(&mut self, value: u8) {
        self.push_u64(u64::from(value));
    }

    fn push_u64(&mut self, value: u64) {
        self.0 ^= value;
        self.0 = self.0.wrapping_mul(0x100000001b3);
    }
}

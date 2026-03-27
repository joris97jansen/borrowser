use crate::dom_patch::DomPatch;
use crate::html5::tokenizer::{TextModeKind, TokenizerControl};

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
        Self(0xcbf29ce484222325u64 ^ seed.rotate_left(7))
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
                self.push_u32(spec.end_tag_name.0);
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
                DomPatch::CreateElement {
                    key,
                    name,
                    attributes,
                } => {
                    self.push_u8(12);
                    self.push_u32(key.0);
                    self.push_str(name);
                    self.push_usize(attributes.len());
                    for (name, value) in attributes {
                        self.push_str(name);
                        self.push_opt_str(value.as_deref());
                    }
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
                    for (name, value) in attributes {
                        self.push_str(name);
                        self.push_opt_str(value.as_deref());
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

    fn push_u8(&mut self, value: u8) {
        self.push_u64(u64::from(value));
    }

    fn push_u64(&mut self, value: u64) {
        self.0 ^= value;
        self.0 = self.0.wrapping_mul(0x100000001b3);
    }
}

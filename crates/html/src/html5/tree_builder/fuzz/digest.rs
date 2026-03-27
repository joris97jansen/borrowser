use crate::dom_patch::DomPatch;
use crate::html5::shared::{AtomTable, AttributeValue, TextValue, Token};

pub(super) struct FuzzDigest(u64);

impl FuzzDigest {
    pub(super) fn new(seed: u64) -> Self {
        Self(0xcbf29ce484222325u64 ^ seed.rotate_left(13))
    }

    pub(super) fn record_token(&mut self, token: &Token, atoms: &AtomTable) {
        match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                self.push_u8(0);
                self.push_bool(name.is_some());
                if let Some(name) = name.and_then(|id| atoms.resolve(id)) {
                    self.push_str(name);
                }
                self.push_bool(*force_quirks);
                self.push_opt_str(public_id.as_deref());
                self.push_opt_str(system_id.as_deref());
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                self.push_u8(1);
                self.push_str(atoms.resolve(*name).unwrap_or("<invalid-atom>"));
                self.push_bool(*self_closing);
                self.push_usize(attrs.len());
                for attr in attrs {
                    self.push_str(atoms.resolve(attr.name).unwrap_or("<invalid-atom>"));
                    match &attr.value {
                        Some(AttributeValue::Owned(value)) => {
                            self.push_bool(true);
                            self.push_str(value);
                        }
                        Some(AttributeValue::Span(_)) => {
                            self.push_str("<span>");
                        }
                        None => self.push_bool(false),
                    }
                }
            }
            Token::EndTag { name } => {
                self.push_u8(2);
                self.push_str(atoms.resolve(*name).unwrap_or("<invalid-atom>"));
            }
            Token::Comment { text } => {
                self.push_u8(3);
                self.push_text_value(text);
            }
            Token::Text { text } => {
                self.push_u8(4);
                self.push_text_value(text);
            }
            Token::Eof => {
                self.push_u8(5);
            }
        }
    }

    pub(super) fn record_patches(&mut self, patches: &[DomPatch]) {
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

    pub(super) fn finish(self) -> u64 {
        self.0
    }

    fn push_text_value(&mut self, text: &TextValue) {
        match text {
            TextValue::Owned(value) => self.push_str(value),
            TextValue::Span(_) => self.push_str("<span>"),
        }
    }

    fn push_opt_str(&mut self, value: Option<&str>) {
        self.push_bool(value.is_some());
        if let Some(value) = value {
            self.push_str(value);
        }
    }

    fn push_bool(&mut self, value: bool) {
        self.push_u8(u8::from(value));
    }

    fn push_usize(&mut self, value: usize) {
        self.push_u64(value as u64);
    }

    fn push_u32(&mut self, value: u32) {
        self.push_u64(u64::from(value));
    }

    fn push_u8(&mut self, value: u8) {
        self.mix(u64::from(value));
    }

    fn push_u64(&mut self, value: u64) {
        self.mix(value);
    }

    fn push_str(&mut self, value: &str) {
        self.push_usize(value.len());
        for &byte in value.as_bytes() {
            self.push_u8(byte);
        }
    }

    fn mix(&mut self, value: u64) {
        self.0 ^= value;
        self.0 = self.0.wrapping_mul(0x100000001b3);
    }
}

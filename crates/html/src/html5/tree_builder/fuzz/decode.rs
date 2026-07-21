//! Byte-driven synthetic token decoder for tree-builder fuzzing.
//!
//! This is intentionally not derived from the production HTML tokenizer. The
//! decoder maps arbitrary bytes into bounded, partially catalog-backed token
//! structures so the tree builder sees malformed orderings and unusual
//! attribute/name/text combinations that are cheap for libFuzzer to mutate and
//! deterministic for corpus replay.

use super::config::{TreeBuilderFuzzConfig, TreeBuilderFuzzError, TreeBuilderFuzzTermination};
use crate::html5::shared::{
    AtomTable, Attribute, AttributeValue, ProcessingInstructionToken, TextValue, Token,
};

/// Synthetic token decoder V2 marker. An exact full-prefix match opts into V2;
/// every other input, including truncated or unknown marker-like prefixes,
/// remains V1 and treats all bytes as token data.
pub(super) const SYNTHETIC_TOKEN_DECODER_V2_MARKER: &[u8] = b"TB-FUZZ-V2\n";

/// AE12 decoder framing. V3 preserves the V2 tag catalog while adding a typed
/// processing-instruction opcode. Older unmarked/V2 byte mappings are frozen.
pub(super) const SYNTHETIC_TOKEN_DECODER_V3_MARKER: &[u8] = b"TB-FUZZ-V3\n";

/// Exact V1 tag catalog from before the AE9b select extension. Its order,
/// length, and modulo mapping are part of the deterministic V1 byte-format
/// contract.
pub(super) const TAG_NAME_CATALOG_V1: &[&str; 30] = &[
    "html", "head", "body", "title", "textarea", "style", "script", "table", "tbody", "thead",
    "tfoot", "tr", "td", "th", "caption", "colgroup", "col", "template", "p", "div", "span", "a",
    "b", "i", "nobr", "applet", "object", "form", "frameset", "br",
];

/// AE9b select-extension additions available only to explicitly versioned V2
/// inputs. V2 keeps the V1 prefix order and appends these names, preserving a
/// simple bounded modulo-selection format without changing legacy bytes.
const TAG_NAME_CATALOG_V2_ADDITIONS: &[&str; 5] = &["select", "option", "optgroup", "input", "hr"];

const ATTR_NAME_CATALOG: &[&str] = &[
    "id",
    "class",
    "href",
    "src",
    "title",
    "style",
    "hidden",
    "checked",
    "selected",
    "value",
    "name",
    "type",
    "data-x",
    "aria-label",
    "role",
];

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DecodedTokenStream {
    pub(super) tokens: Vec<Token>,
    pub(super) tokens_generated: usize,
    pub(super) attrs_generated: usize,
    pub(super) string_bytes_generated: usize,
    pub(super) termination: Option<TreeBuilderFuzzTermination>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SyntheticTokenDecoderVersion {
    V1,
    V2,
    V3,
}

pub(super) fn decoder_version_for_input(bytes: &[u8]) -> SyntheticTokenDecoderVersion {
    if bytes.starts_with(SYNTHETIC_TOKEN_DECODER_V3_MARKER) {
        SyntheticTokenDecoderVersion::V3
    } else if bytes.starts_with(SYNTHETIC_TOKEN_DECODER_V2_MARKER) {
        SyntheticTokenDecoderVersion::V2
    } else {
        SyntheticTokenDecoderVersion::V1
    }
}

pub(super) fn decode_token_stream(
    bytes: &[u8],
    atoms: &mut AtomTable,
    config: TreeBuilderFuzzConfig,
) -> Result<DecodedTokenStream, TreeBuilderFuzzError> {
    let version = decoder_version_for_input(bytes);
    let token_bytes = match version {
        SyntheticTokenDecoderVersion::V1 => bytes,
        SyntheticTokenDecoderVersion::V2 => &bytes[SYNTHETIC_TOKEN_DECODER_V2_MARKER.len()..],
        SyntheticTokenDecoderVersion::V3 => &bytes[SYNTHETIC_TOKEN_DECODER_V3_MARKER.len()..],
    };
    let mut decoder = SyntheticTokenDecoder {
        bytes: token_bytes,
        cursor: 0,
        version,
        tokens: Vec::new(),
        tokens_generated: 0,
        attrs_generated: 0,
        string_bytes_generated: 0,
        config,
        rejected: None,
    };
    while decoder.cursor < decoder.bytes.len() {
        if let Some(termination) = decoder.rejected {
            return Ok(decoder.finish(termination));
        }
        if decoder.tokens_generated >= decoder.config.max_tokens_generated {
            return Ok(decoder.finish(TreeBuilderFuzzTermination::RejectedMaxTokensGenerated));
        }
        if decoder.attrs_generated > decoder.config.max_total_attrs {
            return Ok(decoder.finish(TreeBuilderFuzzTermination::RejectedMaxAttributesGenerated));
        }
        if decoder.string_bytes_generated > decoder.config.max_string_bytes_generated {
            return Ok(decoder.finish(TreeBuilderFuzzTermination::RejectedMaxStringBytesGenerated));
        }
        decoder.decode_one(atoms)?;
    }
    Ok(decoder.finish_completed())
}

struct SyntheticTokenDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
    version: SyntheticTokenDecoderVersion,
    tokens: Vec<Token>,
    tokens_generated: usize,
    attrs_generated: usize,
    string_bytes_generated: usize,
    config: TreeBuilderFuzzConfig,
    rejected: Option<TreeBuilderFuzzTermination>,
}

impl<'a> SyntheticTokenDecoder<'a> {
    fn finish(self, termination: TreeBuilderFuzzTermination) -> DecodedTokenStream {
        DecodedTokenStream {
            tokens: self.tokens,
            tokens_generated: self.tokens_generated,
            attrs_generated: self.attrs_generated,
            string_bytes_generated: self.string_bytes_generated,
            termination: Some(termination),
        }
    }

    fn finish_completed(self) -> DecodedTokenStream {
        DecodedTokenStream {
            tokens: self.tokens,
            tokens_generated: self.tokens_generated,
            attrs_generated: self.attrs_generated,
            string_bytes_generated: self.string_bytes_generated,
            termination: None,
        }
    }

    fn decode_one(&mut self, atoms: &mut AtomTable) -> Result<(), TreeBuilderFuzzError> {
        let token_index = self.tokens_generated;
        let header = self.next_byte();
        let opcode = match self.version {
            SyntheticTokenDecoderVersion::V1 | SyntheticTokenDecoderVersion::V2 => header % 5,
            SyntheticTokenDecoderVersion::V3 => header % 6,
        };
        let token = match opcode {
            0 => self.decode_doctype(header, token_index, atoms)?,
            1 => self.decode_start_tag(header, token_index, atoms)?,
            2 => self.decode_end_tag(token_index, atoms)?,
            3 => self.decode_comment(token_index)?,
            4 => self.decode_text(token_index)?,
            5 => self.decode_processing_instruction(token_index)?,
            _ => unreachable!("bounded synthetic token opcode"),
        };
        if self.rejected.is_some() {
            return Ok(());
        }
        self.tokens.push(token);
        self.tokens_generated = self.tokens_generated.saturating_add(1);
        Ok(())
    }

    fn decode_doctype(
        &mut self,
        header: u8,
        token_index: usize,
        atoms: &mut AtomTable,
    ) -> Result<Token, TreeBuilderFuzzError> {
        let name = if header & 0x10 != 0 {
            Some(self.take_tag_atom(token_index, atoms, "doctype.name")?)
        } else {
            None
        };
        let public_id = if header & 0x20 != 0 {
            Some(self.take_fuzz_string(token_index, "doctype.public_id")?)
        } else {
            None
        };
        let system_id = if header & 0x40 != 0 {
            Some(self.take_fuzz_string(token_index, "doctype.system_id")?)
        } else {
            None
        };
        Ok(Token::Doctype {
            name,
            public_id,
            system_id,
            force_quirks: header & 0x80 != 0,
        })
    }

    fn decode_start_tag(
        &mut self,
        header: u8,
        token_index: usize,
        atoms: &mut AtomTable,
    ) -> Result<Token, TreeBuilderFuzzError> {
        let name = self.take_tag_atom(token_index, atoms, "start_tag.name")?;
        let attr_count = self.take_count(self.config.max_attrs_per_tag);
        if self.attrs_generated.saturating_add(attr_count) > self.config.max_total_attrs {
            self.attrs_generated = self.attrs_generated.saturating_add(attr_count);
            self.rejected = Some(TreeBuilderFuzzTermination::RejectedMaxAttributesGenerated);
            return Ok(Token::StartTag {
                name,
                attrs: Vec::new(),
                self_closing: header & 0x80 != 0,
            });
        }

        let mut attrs = Vec::with_capacity(attr_count);
        for _ in 0..attr_count {
            let attr_name =
                self.take_atom(token_index, atoms, "start_tag.attr_name", ATTR_NAME_CATALOG)?;
            let value_selector = self.next_optional_byte().unwrap_or(0);
            let value = if value_selector & 1 == 0 {
                AttributeValue::Owned(String::new())
            } else {
                AttributeValue::Owned(self.take_fuzz_string(token_index, "start_tag.attr_value")?)
            };
            attrs.push(Attribute {
                name: attr_name,
                value,
            });
        }
        self.attrs_generated = self.attrs_generated.saturating_add(attr_count);

        Ok(Token::StartTag {
            name,
            attrs,
            self_closing: header & 0x80 != 0,
        })
    }

    fn decode_end_tag(
        &mut self,
        token_index: usize,
        atoms: &mut AtomTable,
    ) -> Result<Token, TreeBuilderFuzzError> {
        Ok(Token::EndTag {
            name: self.take_tag_atom(token_index, atoms, "end_tag.name")?,
        })
    }

    fn decode_comment(&mut self, token_index: usize) -> Result<Token, TreeBuilderFuzzError> {
        Ok(Token::Comment {
            text: TextValue::Owned(self.take_fuzz_string(token_index, "comment.text")?),
        })
    }

    fn decode_text(&mut self, token_index: usize) -> Result<Token, TreeBuilderFuzzError> {
        Ok(Token::Text {
            text: TextValue::Owned(self.take_fuzz_string(token_index, "text.text")?),
        })
    }

    fn decode_processing_instruction(
        &mut self,
        token_index: usize,
    ) -> Result<Token, TreeBuilderFuzzError> {
        const TARGETS: [&str; 4] = ["pi", "Exact-Target", "_private", "a1-b"];
        let target_selector = self.next_optional_byte().unwrap_or(0);
        let target = TARGETS[target_selector as usize % TARGETS.len()].to_string();
        let data = self
            .take_fuzz_string(token_index, "processing_instruction.data")?
            .replace('>', "");
        Ok(Token::ProcessingInstruction(ProcessingInstructionToken {
            target,
            data: TextValue::Owned(data),
        }))
    }

    fn take_atom(
        &mut self,
        token_index: usize,
        atoms: &mut AtomTable,
        field: &'static str,
        catalog: &[&str],
    ) -> Result<crate::html5::shared::AtomId, TreeBuilderFuzzError> {
        let selector = self.next_optional_byte().unwrap_or(0);
        let name = if selector & 1 == 0 {
            catalog[selector as usize % catalog.len()].to_string()
        } else {
            self.take_fuzz_string(token_index, field)?
        };
        atoms.intern_ascii_folded(name.as_str()).map_err(|err| {
            TreeBuilderFuzzError::DecodeFailure {
                token_index,
                detail: format!("{field} atom interning failed: {err:?}"),
            }
        })
    }

    fn take_tag_atom(
        &mut self,
        token_index: usize,
        atoms: &mut AtomTable,
        field: &'static str,
    ) -> Result<crate::html5::shared::AtomId, TreeBuilderFuzzError> {
        let selector = self.next_optional_byte().unwrap_or(0);
        let name = if selector & 1 == 0 {
            tag_catalog_name(self.version, selector).to_string()
        } else {
            self.take_fuzz_string(token_index, field)?
        };
        atoms.intern_ascii_folded(name.as_str()).map_err(|err| {
            TreeBuilderFuzzError::DecodeFailure {
                token_index,
                detail: format!("{field} atom interning failed: {err:?}"),
            }
        })
    }

    fn take_fuzz_string(
        &mut self,
        token_index: usize,
        field: &'static str,
    ) -> Result<String, TreeBuilderFuzzError> {
        let len_seed = self.next_optional_byte().unwrap_or(0);
        let wanted = usize::from(len_seed & 0x1f);
        let take = wanted.min(self.remaining());
        let mut out = String::new();
        for _ in 0..take {
            let fragment = byte_to_fragment(self.next_byte());
            let next_bytes = self.string_bytes_generated.saturating_add(fragment.len());
            if next_bytes > self.config.max_string_bytes_generated {
                let _ = token_index;
                let _ = field;
                self.rejected = Some(TreeBuilderFuzzTermination::RejectedMaxStringBytesGenerated);
                return Ok(out);
            }
            self.string_bytes_generated = next_bytes;
            out.push_str(fragment);
        }
        Ok(out)
    }

    fn take_count(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        usize::from(self.next_optional_byte().unwrap_or(0)) % (max + 1)
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    fn next_optional_byte(&mut self) -> Option<u8> {
        if self.cursor >= self.bytes.len() {
            return None;
        }
        let byte = self.bytes[self.cursor];
        self.cursor += 1;
        Some(byte)
    }

    fn next_byte(&mut self) -> u8 {
        self.next_optional_byte().unwrap_or(0)
    }
}

fn tag_catalog_name(version: SyntheticTokenDecoderVersion, selector: u8) -> &'static str {
    let catalog_len = match version {
        SyntheticTokenDecoderVersion::V1 => TAG_NAME_CATALOG_V1.len(),
        SyntheticTokenDecoderVersion::V2 => {
            TAG_NAME_CATALOG_V1.len() + TAG_NAME_CATALOG_V2_ADDITIONS.len()
        }
        SyntheticTokenDecoderVersion::V3 => {
            TAG_NAME_CATALOG_V1.len() + TAG_NAME_CATALOG_V2_ADDITIONS.len()
        }
    };
    let index = selector as usize % catalog_len;
    if index < TAG_NAME_CATALOG_V1.len() {
        TAG_NAME_CATALOG_V1[index]
    } else {
        TAG_NAME_CATALOG_V2_ADDITIONS[index - TAG_NAME_CATALOG_V1.len()]
    }
}

fn byte_to_fragment(byte: u8) -> &'static str {
    match byte {
        0..=25 => LOWERCASE[(byte % LOWERCASE.len() as u8) as usize],
        26..=51 => UPPERCASE[(byte as usize - 26) % UPPERCASE.len()],
        52..=61 => DIGITS[(byte as usize - 52) % DIGITS.len()],
        62 => "-",
        63 => "_",
        64 => ":",
        65 => ".",
        66 => "/",
        67 => "=",
        68 => " ",
        69 => "\n",
        70 => "&",
        71 => ";",
        72 => "<",
        73 => ">",
        74 => "\"",
        75 => "'",
        76 => "e\u{0301}",
        77 => "\u{fffd}",
        78 => "中",
        79 => "🙂",
        _ => LOWERCASE[(byte as usize) % LOWERCASE.len()],
    }
}

const LOWERCASE: &[&str] = &[
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s",
    "t", "u", "v", "w", "x", "y", "z",
];

const UPPERCASE: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

const DIGITS: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

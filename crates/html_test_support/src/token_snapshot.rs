#[cfg(feature = "parser-fixtures")]
use html::conformance::{ObservedToken, ObservedTokenAttribute};
use html::html5::{DocumentParseContext, TextResolver, Token, TokenFmt};

pub const TOKEN_SNAPSHOT_FORMAT_V1: &str = "html5-token-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenSnapshotReadError {
    InvalidUtf8,
    MissingFormatHeader,
    DuplicateFormatHeader {
        line: usize,
    },
    UnsupportedFormat {
        format: String,
    },
    MissingEof,
    ContentAfterEof {
        line: usize,
    },
    MalformedTokenLine {
        line: usize,
        content: String,
        reason: &'static str,
    },
}

impl std::fmt::Display for TokenSnapshotReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUtf8 => f.write_str("token snapshot must be valid UTF-8"),
            Self::MissingFormatHeader => {
                f.write_str("token snapshot is missing '# format: html5-token-v1'")
            }
            Self::DuplicateFormatHeader { line } => {
                write!(f, "duplicate token snapshot format header at line {line}")
            }
            Self::UnsupportedFormat { format } => {
                write!(f, "unsupported token snapshot format '{format}'")
            }
            Self::MissingEof => f.write_str("token snapshot must end with EOF"),
            Self::ContentAfterEof { line } => {
                write!(
                    f,
                    "token snapshot contains token content after EOF at line {line}"
                )
            }
            Self::MalformedTokenLine {
                line,
                content,
                reason,
            } => write!(
                f,
                "malformed token snapshot line {line} ('{content}'): {reason}"
            ),
        }
    }
}

impl std::error::Error for TokenSnapshotReadError {}

pub fn read_html5_token_v1(bytes: &[u8]) -> Result<Vec<String>, TokenSnapshotReadError> {
    let text = std::str::from_utf8(bytes).map_err(|_| TokenSnapshotReadError::InvalidUtf8)?;
    let mut format = None::<&str>;
    let mut tokens = Vec::new();
    let mut saw_eof = false;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        if let Some(value) = raw.strip_prefix("# format:") {
            if format.is_some() {
                return Err(TokenSnapshotReadError::DuplicateFormatHeader { line });
            }
            format = Some(value.trim());
            continue;
        }
        if raw.starts_with('#') || raw.is_empty() {
            continue;
        }
        if saw_eof {
            return Err(TokenSnapshotReadError::ContentAfterEof { line });
        }
        validate_token_line(raw).map_err(|reason| TokenSnapshotReadError::MalformedTokenLine {
            line,
            content: raw.to_string(),
            reason,
        })?;
        saw_eof = raw == "EOF";
        tokens.push(raw.to_string());
    }
    let Some(format) = format else {
        return Err(TokenSnapshotReadError::MissingFormatHeader);
    };
    if format != TOKEN_SNAPSHOT_FORMAT_V1 {
        return Err(TokenSnapshotReadError::UnsupportedFormat {
            format: format.to_string(),
        });
    }
    if !saw_eof {
        return Err(TokenSnapshotReadError::MissingEof);
    }
    Ok(tokens)
}

fn validate_token_line(line: &str) -> Result<(), &'static str> {
    if line == "EOF" {
        return Ok(());
    }
    if let Some(value) = line.strip_prefix("CHAR text=") {
        return validate_single_quoted(value);
    }
    if let Some(value) = line.strip_prefix("COMMENT text=") {
        return validate_single_quoted(value);
    }
    if let Some(name) = line.strip_prefix("END name=") {
        return validate_bare_name(name);
    }
    if let Some(rest) = line.strip_prefix("PI target=") {
        let rest = consume_quoted(rest)?;
        let Some(rest) = rest.strip_prefix(" data=") else {
            return Err("processing-instruction token requires data");
        };
        return validate_single_quoted(rest);
    }
    if let Some(rest) = line.strip_prefix("DOCTYPE name=") {
        return validate_doctype(rest);
    }
    if let Some(rest) = line.strip_prefix("START name=") {
        return validate_start_tag(rest);
    }
    Err("unknown html5-token-v1 token shape")
}

fn validate_doctype(rest: &str) -> Result<(), &'static str> {
    let Some((name, rest)) = rest.split_once(" public_id=") else {
        return Err("doctype token requires public_id");
    };
    if name != "null" {
        validate_bare_name(name)?;
    }
    let (rest, public_id) = consume_nullable_quoted(rest)?;
    let Some(rest) = rest.strip_prefix(" system_id=") else {
        return Err("doctype token requires system_id");
    };
    let (rest, system_id) = consume_nullable_quoted(rest)?;
    let Some(force_quirks) = rest.strip_prefix(" force_quirks=") else {
        return Err("doctype token requires force_quirks");
    };
    if !matches!(force_quirks, "true" | "false") {
        return Err("doctype force_quirks must be true or false");
    }
    let _ = (public_id, system_id);
    Ok(())
}

fn validate_start_tag(rest: &str) -> Result<(), &'static str> {
    let Some((name, rest)) = rest.split_once(" attrs=[") else {
        return Err("start-tag token requires attrs");
    };
    validate_bare_name(name)?;
    let Some((attributes, self_closing)) = rest.rsplit_once("] self_closing=") else {
        return Err("start-tag token requires self_closing");
    };
    if !matches!(self_closing, "true" | "false") {
        return Err("start-tag self_closing must be true or false");
    }
    validate_attribute_list(attributes)
}

fn validate_attribute_list(mut attributes: &str) -> Result<(), &'static str> {
    while !attributes.is_empty() {
        let Some(separator) = attributes.find('=') else {
            return Err("attribute requires '='");
        };
        validate_bare_name(&attributes[..separator])?;
        attributes = consume_quoted(&attributes[separator + 1..])?;
        if attributes.is_empty() {
            break;
        }
        let Some(rest) = attributes.strip_prefix(' ') else {
            return Err("attributes must be separated by one space");
        };
        attributes = rest;
    }
    Ok(())
}

fn validate_single_quoted(value: &str) -> Result<(), &'static str> {
    if consume_quoted(value)?.is_empty() {
        Ok(())
    } else {
        Err("unexpected content after quoted value")
    }
}

fn consume_nullable_quoted(value: &str) -> Result<(&str, bool), &'static str> {
    if let Some(rest) = value.strip_prefix("null") {
        return Ok((rest, false));
    }
    consume_quoted(value).map(|rest| (rest, true))
}

fn consume_quoted(value: &str) -> Result<&str, &'static str> {
    let Some(mut rest) = value.strip_prefix('"') else {
        return Err("value must begin with a quote");
    };
    loop {
        let Some(index) = rest.find(['"', '\\']) else {
            return Err("quoted value is not terminated");
        };
        match rest.as_bytes()[index] {
            b'"' => return Ok(&rest[index + 1..]),
            b'\\' => {
                rest = &rest[index + 1..];
                let Some(escape) = rest.chars().next() else {
                    return Err("escape sequence is incomplete");
                };
                rest = &rest[escape.len_utf8()..];
                match escape {
                    '\\' | '"' | 'n' | 'r' | 't' => {}
                    'u' => {
                        let Some(hex) = rest.strip_prefix('{') else {
                            return Err("Unicode escape requires '{'");
                        };
                        let Some(end) = hex.find('}') else {
                            return Err("Unicode escape requires '}'");
                        };
                        let digits = &hex[..end];
                        if digits.is_empty()
                            || digits.len() > 6
                            || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
                        {
                            return Err("Unicode escape requires one to six hex digits");
                        }
                        let scalar = u32::from_str_radix(digits, 16)
                            .map_err(|_| "Unicode escape is outside the scalar-value range")?;
                        if char::from_u32(scalar).is_none() {
                            return Err("Unicode escape is not a Unicode scalar value");
                        }
                        rest = &hex[end + 1..];
                    }
                    _ => return Err("unsupported escape sequence"),
                }
            }
            _ => unreachable!(),
        }
    }
}

fn validate_bare_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name.chars().any(|ch| ch.is_ascii_whitespace()) {
        Err("name must be non-empty and contain no ASCII whitespace")
    } else {
        Ok(())
    }
}

pub struct TokenFormatContext<'a> {
    pub case_id: &'a str,
    pub mode: &'a str,
}

pub fn format_tokens(
    tokens: &[Token],
    resolver: &impl TextResolver,
    ctx: &DocumentParseContext,
    context: &TokenFormatContext<'_>,
    index: &mut usize,
    mut saw_eof_token: Option<&mut bool>,
) -> Result<Vec<String>, String> {
    let fmt = TokenFmt::new(&ctx.atoms, resolver);
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let token_index = *index;
        *index = index.saturating_add(1);
        if matches!(token, Token::Eof)
            && let Some(saw_eof) = saw_eof_token.as_deref_mut()
        {
            *saw_eof = true;
        }
        let line = fmt.format_token(token).map_err(|err| {
            format!(
                "{err} for '{}' [{}] token #{}",
                context.case_id, context.mode, token_index
            )
        })?;
        out.push(line);
    }
    Ok(out)
}

#[cfg(feature = "parser-fixtures")]
pub fn observe_tokens(
    tokens: &[Token],
    resolver: &dyn TextResolver,
    ctx: &DocumentParseContext,
) -> Result<Vec<ObservedToken>, String> {
    let fmt = TokenFmt::new(&ctx.atoms, resolver);
    tokens
        .iter()
        .map(|token| match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => Ok(ObservedToken::Doctype {
                name: name
                    .map(|name| fmt.resolve_atom(name).map(str::to_string))
                    .transpose()
                    .map_err(|err| err.to_string())?,
                public_id: public_id.clone(),
                system_id: system_id.clone(),
                force_quirks: *force_quirks,
            }),
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => Ok(ObservedToken::StartTag {
                name: fmt
                    .resolve_atom(*name)
                    .map(str::to_string)
                    .map_err(|err| err.to_string())?,
                attributes: attrs
                    .iter()
                    .map(|attribute| {
                        Ok(ObservedTokenAttribute {
                            name: fmt
                                .resolve_atom(attribute.name)
                                .map(str::to_string)
                                .map_err(|err| err.to_string())?,
                            value: fmt
                                .resolve_attr_value(&attribute.value)
                                .map(|value| value.into_owned())
                                .map_err(|err| err.to_string())?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                self_closing: *self_closing,
            }),
            Token::EndTag { name } => Ok(ObservedToken::EndTag {
                name: fmt
                    .resolve_atom(*name)
                    .map(str::to_string)
                    .map_err(|err| err.to_string())?,
            }),
            Token::Comment { text } => Ok(ObservedToken::Comment {
                data: fmt
                    .resolve_text_value(text)
                    .map(|value| value.into_owned())
                    .map_err(|err| err.to_string())?,
            }),
            Token::ProcessingInstruction(processing_instruction) => {
                Ok(ObservedToken::ProcessingInstruction {
                    target: processing_instruction.target.clone(),
                    data: fmt
                        .resolve_text_value(&processing_instruction.data)
                        .map(|value| value.into_owned())
                        .map_err(|err| err.to_string())?,
                })
            }
            Token::Text { text } => Ok(ObservedToken::Character {
                data: fmt
                    .resolve_text_value(text)
                    .map(|value| value.into_owned())
                    .map_err(|err| err.to_string())?,
            }),
            Token::Eof => Ok(ObservedToken::Eof),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{TOKEN_SNAPSHOT_FORMAT_V1, TokenSnapshotReadError, read_html5_token_v1};
    use crate::wpt_tokenizer::run_tokenizer_whole;
    #[cfg(feature = "parser-fixtures")]
    use crate::wpt_tokenizer::run_tokenizer_whole_observed;
    #[cfg(feature = "parser-fixtures")]
    use html::conformance::ObservedToken;
    use html::html5::{
        DocumentParseContext, TextResolveError, TextResolver, TextSpan, TextValue, Token, TokenFmt,
    };

    struct OwnedOnlyResolver;

    const RICH_TOKENIZER_INPUT: &str = concat!(
        "<!DOCTYPE>",
        "<div first=\"a&quot;b\\c\n\r\t\u{1}\" second=\"two\">",
        "text\"\\\n\r\t\u{1}",
        "<!--comment\"\\\n\r\t\u{1}-->",
        "<?Pi_Target-2 data\"\\\n\r\t\u{1}?>",
        "</div>",
    );

    impl TextResolver for OwnedOnlyResolver {
        fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    #[test]
    fn html5_token_v1_reader_rejects_header_and_eof_contract_violations() {
        assert_eq!(
            read_html5_token_v1(b"CHAR text=\"x\"\nEOF\n"),
            Err(TokenSnapshotReadError::MissingFormatHeader)
        );
        assert!(matches!(
            read_html5_token_v1(b"# format: html5-token-v1\n# format: html5-token-v1\nEOF\n"),
            Err(TokenSnapshotReadError::DuplicateFormatHeader { .. })
        ));
        assert_eq!(
            read_html5_token_v1(b"# format: html5-token-v2\nEOF\n"),
            Err(TokenSnapshotReadError::UnsupportedFormat {
                format: "html5-token-v2".to_string()
            })
        );
        assert_eq!(
            read_html5_token_v1(b"# format: html5-token-v1\nCHAR text=\"x\"\n"),
            Err(TokenSnapshotReadError::MissingEof)
        );
    }

    #[test]
    fn html5_token_v1_reader_rejects_malformed_content() {
        assert!(matches!(
            read_html5_token_v1(b"# format: html5-token-v1\nTOKEN???\nEOF\n"),
            Err(TokenSnapshotReadError::MalformedTokenLine { line: 2, .. })
        ));
        assert!(matches!(
            read_html5_token_v1(b"# format: html5-token-v1\nEOF\nCHAR text=\"x\"\n"),
            Err(TokenSnapshotReadError::ContentAfterEof { line: 3 })
        ));
        assert!(matches!(
            read_html5_token_v1(b"# format: html5-token-v1\nCHAR text=\"\\u{D800}\"\nEOF\n"),
            Err(TokenSnapshotReadError::MalformedTokenLine { line: 2, .. })
        ));
        assert!(matches!(
            read_html5_token_v1(b"# format: html5-token-v1\nEND name=\nEOF\n"),
            Err(TokenSnapshotReadError::MalformedTokenLine {
                line: 2,
                reason: "name must be non-empty and contain no ASCII whitespace",
                ..
            })
        ));
        for malformed in ["END name=two words", "END name=two\twords"] {
            let snapshot = format!("# format: html5-token-v1\n{malformed}\nEOF\n");
            assert_eq!(
                read_html5_token_v1(snapshot.as_bytes()),
                Err(TokenSnapshotReadError::MalformedTokenLine {
                    line: 2,
                    content: malformed.to_string(),
                    reason: "name must be non-empty and contain no ASCII whitespace",
                })
            );
        }
    }

    #[test]
    fn html5_token_v1_reader_accepts_exact_lines_from_the_real_tokenizer_writer() {
        let mut writer_lines =
            run_tokenizer_whole(RICH_TOKENIZER_INPUT, "token-reader-writer-compatibility")
                .expect("production tokenizer and TokenFmt writer");
        // Input preprocessing intentionally turns CR into LF. Exercise the
        // same production TokenFmt writer directly with an owned token so its
        // otherwise unreachable CR escape remains part of the compatibility
        // contract without introducing a second formatter.
        let context = DocumentParseContext::new();
        let cr_line = TokenFmt::new(&context.atoms, &OwnedOnlyResolver)
            .format_token(&Token::Text {
                text: TextValue::Owned("\r".to_string()),
            })
            .expect("TokenFmt formats owned CR text");
        let eof_index = writer_lines
            .iter()
            .position(|line| line == "EOF")
            .expect("tokenizer writer emits EOF");
        writer_lines.insert(eof_index, cr_line);
        let snapshot = format!(
            "# format: {TOKEN_SNAPSHOT_FORMAT_V1}\n{}\n",
            writer_lines.join("\n")
        );
        let reader_lines = read_html5_token_v1(snapshot.as_bytes()).expect("reader accepts writer");

        assert_eq!(reader_lines, writer_lines);
        assert!(writer_lines.iter().any(|line| {
            line == "DOCTYPE name=null public_id=null system_id=null force_quirks=true"
        }));
        let start = writer_lines
            .iter()
            .find(|line| line.starts_with("START name=div "))
            .expect("start tag");
        assert!(
            start.find("first=").expect("first attribute")
                < start.find("second=").expect("second attribute"),
            "attribute encounter order must survive TokenFmt: {start}"
        );
        for prefix in [
            "END name=div",
            "CHAR text=",
            "COMMENT text=",
            "PI target=\"Pi_Target-2\" data=",
            "EOF",
        ] {
            assert!(
                writer_lines.iter().any(|line| line.starts_with(prefix)),
                "missing writer variant {prefix}: {writer_lines:?}"
            );
        }
        for escaped in ["\\\"", "\\\\", "\\n", "\\r", "\\t", "\\u{01}"] {
            assert!(
                writer_lines.iter().any(|line| line.contains(escaped)),
                "missing representative escape {escaped}: {writer_lines:?}"
            );
        }

        let identified_doctype = run_tokenizer_whole(
            "<!DOCTYPE html PUBLIC \"public-id\" \"system-id\">",
            "token-reader-writer-doctype-identifiers",
        )
        .expect("tokenizer writes doctype identifiers");
        let identified_snapshot = format!(
            "# format: {TOKEN_SNAPSHOT_FORMAT_V1}\n{}\n",
            identified_doctype.join("\n")
        );
        assert_eq!(
            read_html5_token_v1(identified_snapshot.as_bytes()).unwrap(),
            identified_doctype
        );
        assert!(identified_doctype.iter().any(|line| {
            line == "DOCTYPE name=html public_id=\"public-id\" system_id=\"system-id\" force_quirks=false"
        }));

        let unicode_name_input = concat!(
            "<!DOCTYPE h\u{00A0}tml>",
            "<tag\u{00A0}name attr\u{2003}name=value>",
            "</tag\u{00A0}name>",
        );
        let unicode_name_lines = run_tokenizer_whole(
            unicode_name_input,
            "token-reader-writer-non-ascii-name-whitespace",
        )
        .expect("production tokenizer and TokenFmt write non-ASCII name whitespace");
        assert!(unicode_name_lines.iter().any(|line| {
            line == "DOCTYPE name=h\u{00A0}tml public_id=null system_id=null force_quirks=false"
        }));
        assert!(unicode_name_lines.iter().any(|line| {
            line
                == "START name=tag\u{00A0}name attrs=[attr\u{2003}name=\"value\"] self_closing=false"
        }));
        assert!(
            unicode_name_lines
                .iter()
                .any(|line| line == "END name=tag\u{00A0}name")
        );
        let unicode_name_snapshot = format!(
            "# format: {TOKEN_SNAPSHOT_FORMAT_V1}\n{}\n",
            unicode_name_lines.join("\n")
        );
        assert_eq!(
            read_html5_token_v1(unicode_name_snapshot.as_bytes()).unwrap(),
            unicode_name_lines
        );
    }

    #[cfg(feature = "parser-fixtures")]
    #[test]
    fn tokenizer_observer_is_passive_for_snapshot_output_and_token_order() {
        let plain_snapshot_lines =
            run_tokenizer_whole(RICH_TOKENIZER_INPUT, "token-observer-disabled-parity")
                .expect("tokenizer without observer");
        let observed_run =
            run_tokenizer_whole_observed(RICH_TOKENIZER_INPUT, "token-observer-enabled-parity")
                .expect("tokenizer with observer");

        assert_eq!(plain_snapshot_lines, observed_run.snapshot_lines);
        assert!(!observed_run.observed_tokens.is_empty());
        assert_eq!(
            observed_run.observed_tokens.len(),
            observed_run.snapshot_lines.len(),
            "the observer must retain each drained production token exactly once"
        );

        let observed_kinds = observed_run
            .observed_tokens
            .iter()
            .map(|token| match token {
                ObservedToken::Doctype { .. } => "doctype",
                ObservedToken::StartTag { .. } => "start-tag",
                ObservedToken::EndTag { .. } => "end-tag",
                ObservedToken::Character { .. } => "character",
                ObservedToken::Comment { .. } => "comment",
                ObservedToken::ProcessingInstruction { .. } => "processing-instruction",
                ObservedToken::Eof => "eof",
            })
            .collect::<Vec<_>>();
        let snapshot_kinds = observed_run
            .snapshot_lines
            .iter()
            .map(|line| {
                if line.starts_with("DOCTYPE ") {
                    "doctype"
                } else if line.starts_with("START ") {
                    "start-tag"
                } else if line.starts_with("END ") {
                    "end-tag"
                } else if line.starts_with("CHAR ") {
                    "character"
                } else if line.starts_with("COMMENT ") {
                    "comment"
                } else if line.starts_with("PI ") {
                    "processing-instruction"
                } else if line == "EOF" {
                    "eof"
                } else {
                    panic!("unexpected TokenFmt line: {line}")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(observed_kinds, snapshot_kinds);
        assert_eq!(
            observed_kinds,
            vec![
                "doctype",
                "start-tag",
                "character",
                "comment",
                "processing-instruction",
                "end-tag",
                "eof",
            ]
        );

        let ObservedToken::StartTag { attributes, .. } = &observed_run.observed_tokens[1] else {
            panic!("second observed token must be the production start tag")
        };
        assert_eq!(attributes[0].name, "first");
        assert_eq!(attributes[1].name, "second");
    }
}

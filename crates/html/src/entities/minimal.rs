use memchr::memchr;
use std::borrow::Cow;

use super::numeric::{MAX_DEC_DIGITS, MAX_HEX_DIGITS, match_bytes};
use super::policy::{
    CharacterReferenceContext, CharacterReferenceDecode, CharacterReferenceDiagnostic,
    CharacterReferenceDiagnosticKind,
};

const NAMED_ENTITIES: &[(&[u8], char)] = &[
    (b"&amp;", '&'),
    (b"&lt;", '<'),
    (b"&gt;", '>'),
    (b"&quot;", '"'),
    (b"&apos;", '\''),
    (b"&nbsp;", '\u{00A0}'),
];

#[cfg(test)]
pub(super) fn decode_entities_minimal(s: &str) -> Cow<'_, str> {
    decode_entities_minimal_with_diagnostics(s, CharacterReferenceContext::DataText).text
}

pub(super) fn decode_entities_minimal_with_diagnostics(
    s: &str,
    _context: CharacterReferenceContext,
) -> CharacterReferenceDecode<'_> {
    let bytes = s.as_bytes();
    if !bytes.contains(&b'&') {
        return CharacterReferenceDecode {
            text: Cow::Borrowed(s),
            diagnostics: Vec::new(),
        };
    }

    let mut out: Option<String> = None;
    let mut diagnostics = Vec::new();
    let mut i = 0;
    let mut copy_start = 0;

    while let Some(rel) = memchr(b'&', &bytes[i..]) {
        i += rel;
        debug_assert_eq!(bytes[i], b'&');

        if let Some((ch, next)) = match_supported_named(bytes, i) {
            let out = out.get_or_insert_with(|| String::with_capacity(s.len()));
            if copy_start < i {
                out.push_str(&s[copy_start..i]);
            }
            out.push(ch);
            i = next;
            copy_start = i;
            continue;
        }

        if let Some(reference) = scan_numeric_reference(bytes, s, i) {
            if let Some(diagnostic) = reference.diagnostic {
                diagnostics.push(diagnostic);
            }
            if let Some(ch) = reference.decoded {
                let out = out.get_or_insert_with(|| String::with_capacity(s.len()));
                if copy_start < i {
                    out.push_str(&s[copy_start..i]);
                }
                out.push(ch);
                i = reference.end;
                copy_start = i;
            } else {
                i = reference.end;
            }
            continue;
        }

        if let Some((diagnostic, next)) = scan_named_diagnostic(bytes, i) {
            diagnostics.push(diagnostic);
            i = next;
            continue;
        }

        i += 1;
    }

    if let Some(mut out) = out {
        if copy_start < bytes.len() {
            out.push_str(&s[copy_start..]);
        }
        CharacterReferenceDecode {
            text: Cow::Owned(out),
            diagnostics,
        }
    } else {
        CharacterReferenceDecode {
            text: Cow::Borrowed(s),
            diagnostics,
        }
    }
}

fn match_supported_named(bytes: &[u8], i: usize) -> Option<(char, usize)> {
    for (pat, ch) in NAMED_ENTITIES {
        if match_bytes(bytes, i, pat) {
            return Some((*ch, i + pat.len()));
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NumericReferenceScan {
    decoded: Option<char>,
    diagnostic: Option<CharacterReferenceDiagnostic>,
    end: usize,
}

fn scan_numeric_reference(bytes: &[u8], s: &str, start: usize) -> Option<NumericReferenceScan> {
    let (digits_start, max_digits, radix) =
        if match_bytes(bytes, start, b"&#x") || match_bytes(bytes, start, b"&#X") {
            (start + 3, MAX_HEX_DIGITS, 16)
        } else if match_bytes(bytes, start, b"&#") {
            (start + 2, MAX_DEC_DIGITS, 10)
        } else {
            return None;
        };

    let mut j = digits_start;
    let mut digits = 0usize;

    while j < bytes.len() {
        let b = bytes[j];
        if b == b';' {
            if digits == 0 {
                return Some(NumericReferenceScan {
                    decoded: None,
                    diagnostic: Some(diagnostic(
                        start,
                        CharacterReferenceDiagnosticKind::MissingNumericDigits,
                        None,
                    )),
                    end: j + 1,
                });
            }

            let raw_digits = &s[digits_start..j];
            let parsed = if radix == 16 {
                u32::from_str_radix(raw_digits, 16)
            } else {
                raw_digits.parse::<u32>()
            };
            if let Ok(value) = parsed
                && let Some(ch) = char::from_u32(value)
            {
                return Some(NumericReferenceScan {
                    decoded: Some(ch),
                    diagnostic: None,
                    end: j + 1,
                });
            }

            return Some(NumericReferenceScan {
                decoded: None,
                diagnostic: Some(diagnostic(
                    start,
                    CharacterReferenceDiagnosticKind::InvalidNumericScalar,
                    parsed.ok(),
                )),
                end: j + 1,
            });
        }

        if digits == max_digits {
            return Some(NumericReferenceScan {
                decoded: None,
                diagnostic: Some(diagnostic(
                    start,
                    CharacterReferenceDiagnosticKind::NumericTooLong,
                    None,
                )),
                end: malformed_entity_end(bytes, start),
            });
        }

        let valid_digit = if radix == 16 {
            b.is_ascii_hexdigit()
        } else {
            b.is_ascii_digit()
        };
        if !valid_digit {
            let kind = if digits == 0 {
                CharacterReferenceDiagnosticKind::MissingNumericDigits
            } else if b.is_ascii_whitespace() || b == b'&' {
                CharacterReferenceDiagnosticKind::MissingNumericSemicolon
            } else {
                CharacterReferenceDiagnosticKind::MalformedNumeric
            };
            return Some(NumericReferenceScan {
                decoded: None,
                diagnostic: Some(diagnostic(start, kind, Some(b as u32))),
                end: malformed_entity_end(bytes, start),
            });
        }

        digits += 1;
        j += 1;
    }

    let kind = if digits == 0 {
        CharacterReferenceDiagnosticKind::MissingNumericDigits
    } else {
        CharacterReferenceDiagnosticKind::MissingNumericSemicolon
    };
    Some(NumericReferenceScan {
        decoded: None,
        diagnostic: Some(diagnostic(start, kind, None)),
        end: bytes.len(),
    })
}

fn scan_named_diagnostic(
    bytes: &[u8],
    start: usize,
) -> Option<(CharacterReferenceDiagnostic, usize)> {
    let first = *bytes.get(start + 1)?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let mut j = start + 1;
    while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
        j += 1;
    }

    if bytes.get(j) == Some(&b';') {
        return Some((
            diagnostic(
                start,
                CharacterReferenceDiagnosticKind::UnknownNamed,
                Some(first as u32),
            ),
            j + 1,
        ));
    }

    for (pat, _) in NAMED_ENTITIES {
        let name = &pat[1..pat.len() - 1];
        if bytes.get(start + 1..j) == Some(name) {
            return Some((
                diagnostic(
                    start,
                    CharacterReferenceDiagnosticKind::MissingNamedSemicolon,
                    Some(first as u32),
                ),
                start + 1 + name.len(),
            ));
        }
    }

    None
}

fn malformed_entity_end(bytes: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b';' {
            return j + 1;
        }
        if b == b'&' || b.is_ascii_whitespace() {
            return j;
        }
        j += 1;
    }
    bytes.len()
}

fn diagnostic(
    offset: usize,
    kind: CharacterReferenceDiagnosticKind,
    aux: Option<u32>,
) -> CharacterReferenceDiagnostic {
    CharacterReferenceDiagnostic { offset, kind, aux }
}

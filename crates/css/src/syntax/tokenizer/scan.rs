pub(super) fn starts_with(input: &str, byte_offset: usize, pattern: &str) -> bool {
    input
        .get(byte_offset..)
        .map(|tail| tail.starts_with(pattern))
        .unwrap_or(false)
}

pub(super) fn peek_char_at(input: &str, byte_offset: usize) -> Option<char> {
    input.get(byte_offset..)?.chars().next()
}

pub(super) fn peek_char_at_after(input: &str, byte_offset: usize) -> Option<char> {
    let first = peek_char_at(input, byte_offset)?;
    peek_char_at(input, byte_offset + first.len_utf8())
}

pub(super) fn peek_char_at_after_after(input: &str, byte_offset: usize) -> Option<char> {
    let first = peek_char_at(input, byte_offset)?;
    let second = peek_char_at(input, byte_offset + first.len_utf8())?;
    peek_char_at(input, byte_offset + first.len_utf8() + second.len_utf8())
}

pub(super) fn is_css_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000C}')
}

pub(super) fn is_css_line_break_start(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{000C}')
}

pub(super) fn is_name_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic() || !ch.is_ascii()
}

pub(super) fn is_name(ch: char) -> bool {
    is_name_start(ch) || ch.is_ascii_digit() || ch == '-'
}

pub(super) fn starts_name(first: Option<char>, second: Option<char>) -> bool {
    match first {
        Some(ch) if is_name(ch) => true,
        Some('\\') => second
            .map(|ch| !is_css_line_break_start(ch))
            .unwrap_or(false),
        _ => false,
    }
}

pub(super) fn would_start_identifier(
    first: Option<char>,
    second: Option<char>,
    third: Option<char>,
) -> bool {
    match first {
        Some('-') => match second {
            Some(ch) if is_name_start(ch) || ch == '-' => true,
            Some('\\') => third
                .map(|ch| !is_css_line_break_start(ch))
                .unwrap_or(false),
            _ => false,
        },
        Some(ch) if is_name_start(ch) => true,
        Some('\\') => second
            .map(|ch| !is_css_line_break_start(ch))
            .unwrap_or(false),
        _ => false,
    }
}

pub(super) fn would_start_number(
    first: Option<char>,
    second: Option<char>,
    third: Option<char>,
) -> bool {
    match first {
        Some('+') | Some('-') => match second {
            Some(ch) if ch.is_ascii_digit() => true,
            Some('.') => third.map(|ch| ch.is_ascii_digit()).unwrap_or(false),
            _ => false,
        },
        Some('.') => second.map(|ch| ch.is_ascii_digit()).unwrap_or(false),
        Some(ch) => ch.is_ascii_digit(),
        None => false,
    }
}

pub(super) fn would_start_exponent(first: Option<char>, second: Option<char>) -> bool {
    match first {
        Some('+') | Some('-') => second.map(|ch| ch.is_ascii_digit()).unwrap_or(false),
        Some(ch) => ch.is_ascii_digit(),
        None => false,
    }
}

pub(super) fn is_non_printable(ch: char) -> bool {
    matches!(ch, '\u{0000}'..='\u{0008}' | '\u{000B}' | '\u{000E}'..='\u{001F}' | '\u{007F}')
}

pub(super) fn advance_css_line_break_in_str(input: &str, byte_offset: usize) -> Option<usize> {
    let ch = input.get(byte_offset..)?.chars().next()?;
    match ch {
        '\r' => {
            let next = byte_offset + ch.len_utf8();
            if input.get(next..)?.starts_with('\n') {
                Some(next + '\n'.len_utf8())
            } else {
                Some(next)
            }
        }
        '\n' | '\u{000C}' => Some(byte_offset + ch.len_utf8()),
        _ => None,
    }
}

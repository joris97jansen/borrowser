//! Shared CSS escape consumption. Token callers retain responsibility for
//! string continuations, malformed tokens and recovery; this is not a parser.

pub(crate) fn escaped_code_point(input: &str, slash: usize) -> Option<(char, usize)> {
    let mut end = slash + 1;
    let next = input.get(end..)?.chars().next();
    if matches!(next, Some('\n' | '\r' | '\u{c}')) {
        return None;
    }
    let Some(next) = next else {
        return Some(('\u{fffd}', end));
    };
    if next.is_ascii_hexdigit() {
        let mut value = 0;
        for ch in input[end..].chars().take(6) {
            let Some(digit) = ch.to_digit(16).filter(|_| ch.is_ascii_hexdigit()) else {
                break;
            };
            value = value * 16 + digit;
            end += ch.len_utf8();
        }
        if let Some(ch) = input[end..]
            .chars()
            .next()
            .filter(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}'))
        {
            end += ch.len_utf8();
            if ch == '\r' && input[end..].starts_with('\n') {
                end += 1;
            }
        }
        Some((
            char::from_u32(value)
                .filter(|_| value != 0)
                .unwrap_or('\u{fffd}'),
            end,
        ))
    } else {
        Some((
            if next == '\0' { '\u{fffd}' } else { next },
            end + next.len_utf8(),
        ))
    }
}

/// Decode a syntax-owned payload once. Continuations are valid only in strings.
pub(crate) fn decode(input: &str, string: bool) -> Option<String> {
    let mut output = String::new();
    let mut pos = 0;
    while let Some(ch) = input.get(pos..)?.chars().next() {
        if ch == '\\' {
            let tail = &input[pos + 1..];
            if string && tail.is_empty() {
                break;
            }
            if string && matches!(tail.chars().next(), Some('\n' | '\r' | '\u{c}')) {
                let c = tail.chars().next()?;
                pos += 1 + c.len_utf8();
                if c == '\r' && input[pos..].starts_with('\n') {
                    pos += 1;
                }
                continue;
            }
            let (value, end) = escaped_code_point(input, pos)?;
            output.push(value);
            pos = end;
        } else {
            output.push(if ch == '\0' { '\u{fffd}' } else { ch });
            pos += ch.len_utf8();
        }
    }
    Some(output)
}

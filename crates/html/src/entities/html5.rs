use std::borrow::Cow;

use super::numeric::decode_numeric_entity;
use super::policy::Html5EntityContext;

#[path = "../entities_html5.rs"]
mod entities_html5;

use self::entities_html5::{
    HTML5_ENTITIES as ENTITY_TABLE, HTML5_LEGACY_ENTITIES, MAX_HTML5_ENTITY_LEN,
    MAX_HTML5_LEGACY_ENTITY_LEN,
};

#[cfg(test)]
pub(super) use self::entities_html5::HTML5_ENTITIES;

pub(super) fn decode_entities_html5(s: &str, context: Html5EntityContext) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    if !bytes.contains(&b'&') {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copy_start = 0;

    while i < bytes.len() {
        if bytes[i] != b'&' {
            i += 1;
            continue;
        }

        if copy_start < i {
            out.push_str(&s[copy_start..i]);
        }

        if let Some(next) = decode_numeric_entity(bytes, s, i, &mut out) {
            i = next;
            copy_start = i;
            continue;
        }

        if let Some((value, next)) = html5_named_entity_longest(bytes, i, context) {
            out.push_str(value);
            i = next;
            copy_start = i;
            continue;
        }

        out.push('&');
        i += 1;
        copy_start = i;
    }

    if copy_start < bytes.len() {
        out.push_str(&s[copy_start..]);
    }

    Cow::Owned(out)
}

fn html5_named_entity_longest(
    bytes: &[u8],
    start: usize,
    context: Html5EntityContext,
) -> Option<(&'static str, usize)> {
    if start >= bytes.len() {
        return None;
    }
    let max_len = MAX_HTML5_ENTITY_LEN;
    if max_len == 0 {
        return None;
    }
    if start + 1 >= bytes.len() || bytes[start + 1] == b'#' {
        return None;
    }

    let max_end = (start + max_len - 1).min(bytes.len() - 1);
    let mut best: Option<(&'static str, usize)> = None;

    let mut j = start + 1;
    while j <= max_end {
        let b = bytes[j];
        if b == b';' {
            let candidate = &bytes[start..=j];
            let index = ENTITY_TABLE
                .binary_search_by(|entity| entity.name.cmp(candidate))
                .ok()?;
            return Some((ENTITY_TABLE[index].value, j + 1));
        }
        if !b.is_ascii()
            || b.is_ascii_whitespace()
            || b == b'<'
            || b == b'&'
            || b == b'"'
            || b == b'\''
            || b == b'='
        {
            break;
        }

        if j - start + 1 <= MAX_HTML5_LEGACY_ENTITY_LEN {
            let candidate = &bytes[start..=j];
            if let Ok(index) =
                HTML5_LEGACY_ENTITIES.binary_search_by(|entity| entity.name.cmp(candidate))
            {
                best = Some((HTML5_LEGACY_ENTITIES[index].value, j + 1));
            }
        }
        j += 1;
    }

    let Some((value, next)) = best else {
        return None;
    };
    if context == Html5EntityContext::AttributeValue {
        if let Some(next_b) = bytes.get(next)
            && (next_b.is_ascii_alphanumeric() || *next_b == b'=')
        {
            return None;
        }
    }
    Some((value, next))
}

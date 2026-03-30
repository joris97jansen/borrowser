use super::classify::is_html_space_byte;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScriptTagBoundaryMatch {
    Matched { cursor_after: usize },
    NeedMoreInput,
    NoMatch,
}

pub(crate) fn match_script_tag_boundary_at(
    bytes: &[u8],
    start: usize,
    closing: bool,
) -> ScriptTagBoundaryMatch {
    let mut cursor = start;
    let Some(&byte) = bytes.get(cursor) else {
        return ScriptTagBoundaryMatch::NeedMoreInput;
    };
    if byte != b'<' {
        return ScriptTagBoundaryMatch::NoMatch;
    }
    cursor += 1;

    if closing {
        let Some(&byte) = bytes.get(cursor) else {
            return ScriptTagBoundaryMatch::NeedMoreInput;
        };
        if byte != b'/' {
            return ScriptTagBoundaryMatch::NoMatch;
        }
        cursor += 1;
    }

    const SCRIPT: &[u8] = b"script";
    for expected in SCRIPT {
        let Some(&byte) = bytes.get(cursor) else {
            return ScriptTagBoundaryMatch::NeedMoreInput;
        };
        if !byte.eq_ignore_ascii_case(expected) {
            return ScriptTagBoundaryMatch::NoMatch;
        }
        cursor += 1;
    }

    let Some(&delimiter) = bytes.get(cursor) else {
        return ScriptTagBoundaryMatch::NeedMoreInput;
    };
    if delimiter == b'>' || delimiter == b'/' || is_html_space_byte(delimiter) {
        ScriptTagBoundaryMatch::Matched {
            cursor_after: cursor + 1,
        }
    } else {
        ScriptTagBoundaryMatch::NoMatch
    }
}

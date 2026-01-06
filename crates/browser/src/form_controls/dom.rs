use html::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputControlType {
    Text,
    Checkbox,
    Radio,
    Other,
}

pub fn input_control_type(node: &Node) -> InputControlType {
    let Node::Element {
        name, attributes, ..
    } = node
    else {
        return InputControlType::Other;
    };

    if !name.eq_ignore_ascii_case("input") {
        return InputControlType::Other;
    }

    let mut ty: Option<&str> = None;
    for (k, v) in attributes {
        if k.eq_ignore_ascii_case("type") {
            ty = v.as_deref().map(str::trim).filter(|s| !s.is_empty());
            break;
        }
    }

    match ty {
        None => InputControlType::Text, // missing type defaults to text
        Some(t) if t.eq_ignore_ascii_case("text") => InputControlType::Text,
        Some(t) if t.eq_ignore_ascii_case("checkbox") => InputControlType::Checkbox,
        Some(t) if t.eq_ignore_ascii_case("radio") => InputControlType::Radio,
        _ => InputControlType::Other,
    }
}

pub(super) fn attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match node {
        Node::Element { attributes, .. } => attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .and_then(|(_, v)| v.as_deref()),
        _ => None,
    }
}

pub(super) fn has_attr(node: &Node, name: &str) -> bool {
    match node {
        Node::Element { attributes, .. } => {
            attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
        }
        _ => false,
    }
}

pub(super) fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text { text, .. } => out.push_str(text),
            Node::Element { children, .. } | Node::Document { children, .. } => {
                collect_text(children, out);
            }
            Node::Comment { .. } => {}
        }
    }
}

pub(super) fn normalize_textarea_newlines(s: &str) -> String {
    // Normalize CRLF/CR to LF. (Browsers store textarea values with LF newlines.)
    if !s.contains('\r') {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    let _ = it.next();
                }
                out.push('\n');
            }
            _ => out.push(ch),
        }
    }
    out
}

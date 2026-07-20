use html::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputControlType {
    Text,
    Checkbox,
    Radio,
    Other,
}

pub fn input_control_type(node: &Node) -> InputControlType {
    let Node::Element { element } = node else {
        return InputControlType::Other;
    };

    if element.namespace() != html::ElementNamespace::Html || element.name() != "input" {
        return InputControlType::Other;
    }

    let mut ty: Option<&str> = None;
    for attribute in element.attributes() {
        if attribute.namespace() == html::AttributeNamespace::None
            && attribute.local_name().eq_ignore_ascii_case("type")
        {
            ty = Some(str::trim(attribute.value())).filter(|s| !s.is_empty());
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
        Node::Element { element } if element.namespace() == html::ElementNamespace::Html => element
            .attributes()
            .iter()
            .find(|attribute| {
                attribute.namespace() == html::AttributeNamespace::None
                    && attribute.local_name().eq_ignore_ascii_case(name)
            })
            .map(html::ParserCreatedAttribute::value),
        _ => None,
    }
}

pub(super) fn has_attr(node: &Node, name: &str) -> bool {
    match node {
        Node::Element { element } if element.namespace() == html::ElementNamespace::Html => {
            element.attributes().iter().any(|attribute| {
                attribute.namespace() == html::AttributeNamespace::None
                    && attribute.local_name().eq_ignore_ascii_case(name)
            })
        }
        _ => false,
    }
}

pub(super) fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text { text, .. } => out.push_str(text),
            Node::Element { element } => collect_text(element.children(), out),
            Node::Document { children, .. } => collect_text(children, out),
            Node::Comment { .. } | Node::DocumentType { .. } => {}
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

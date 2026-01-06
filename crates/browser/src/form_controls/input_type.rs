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

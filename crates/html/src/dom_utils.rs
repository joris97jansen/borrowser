use crate::Node;

/// Collect concatenated text from <style> elements.
pub fn collect_style_texts(node: &Node, out: &mut String) {
    match node {
        Node::Element { name, children, .. } if name.eq_ignore_ascii_case("style") => {
            for c in children {
                if let Node::Text { text } = c {
                    out.push_str(text);
                    out.push('\n');
                }
            }
        }
        Node::Element { children, .. } | Node::Document { children, .. } => {
            for c in children {
                collect_style_texts(c, out);
            }
        }
        _ => {}
    }
}

/// Collect <link rel="stylesheet" href="…"> href values.
pub fn collect_stylesheet_hrefs(node: &Node, out: &mut Vec<String>) {
    if let Node::Element { name, attributes, .. } = node {
        if name.eq_ignore_ascii_case("link") {
            let mut is_stylesheet = false;
            let mut href: Option<&str> = None;
            for (k, v) in attributes {
                let key = k.as_str();
                if key.eq_ignore_ascii_case("rel") {
                    if let Some(val) = v.as_deref() {
                        if val.split_whitespace().any(|t| t.eq_ignore_ascii_case("stylesheet")) {
                            is_stylesheet = true;
                        }
                    }
                } else if key.eq_ignore_ascii_case("href") {
                    href = v.as_deref();
                }
            }
            if is_stylesheet {
                if let Some(h) = href {
                    out.push(h.to_string());
                }
            }
        }
        if let Node::Element { children, .. } | Node::Document { children, .. } = node {
            for c in children {
                collect_stylesheet_hrefs(c, out);
            }
        }
    }
}
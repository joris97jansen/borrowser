use crate::{Id, Node};

/// Collect concatenated text from <style> elements.
pub fn collect_style_texts(node: &Node, out: &mut String) {
    match node {
        Node::Element { name, children, .. } if name.eq_ignore_ascii_case("style") => {
            for c in children {
                if let Node::Text { text, .. } = c {
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
    match node {
        Node::Element {
            name,
            attributes,
            children,
            ..
        } => {
            if name.eq_ignore_ascii_case("link") {
                let mut is_stylesheet = false;
                let mut href: Option<&str> = None;

                for (k, v) in attributes {
                    let key = k.as_str();
                    if key.eq_ignore_ascii_case("rel") {
                        if let Some(val) = v.as_deref() {
                            if val
                                .split_whitespace()
                                .any(|t| t.eq_ignore_ascii_case("stylesheet"))
                            {
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

            for c in children {
                collect_stylesheet_hrefs(c, out);
            }
        }
        Node::Document { children, .. } => {
            for c in children {
                collect_stylesheet_hrefs(c, out);
            }
        }
        _ => {}
    }
}

/// Collect <img src="…"> src values.
pub fn collect_img_srcs(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Element {
            name,
            attributes,
            children,
            ..
        } => {
            if name.eq_ignore_ascii_case("img") {
                for (k, v) in attributes {
                    if k.eq_ignore_ascii_case("src") {
                        if let Some(src) = v.as_deref() {
                            let src = src.trim();
                            if !src.is_empty() {
                                out.push(src.to_string());
                            }
                        }
                        break;
                    }
                }
            }

            for c in children {
                collect_img_srcs(c, out);
            }
        }
        Node::Document { children, .. } => {
            for c in children {
                collect_img_srcs(c, out);
            }
        }
        _ => {}
    }
}

pub fn collect_visible_text<'a>(node: &'a Node, ancestors: &mut Vec<&'a Node>, out: &mut String) {
    match node {
        Node::Text { text, .. } => {
            let t = text.trim();
            if !t.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
        Node::Element { name, children, .. } => {
            if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
                return; // skip
            }
            ancestors.push(node);
            for c in children {
                collect_visible_text(c, ancestors, out);
            }
            ancestors.pop();

            match &name.to_ascii_lowercase()[..] {
                "p" | "div" | "section" | "article" | "header" | "footer" | "h1" | "h2" | "h3"
                | "h4" | "h5" | "h6" | "li" => {
                    out.push_str("\n\n");
                }
                _ => {}
            }
        }
        Node::Document { children, .. } => {
            for c in children {
                collect_visible_text(c, ancestors, out);
            }
        }
        _ => {}
    }
}

pub fn first_styles(style: &[(String, String)]) -> String {
    style
        .iter()
        .take(3)
        .map(|(k, v)| format!(r#"{k}: {v};"#))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn outline_from_dom(root: &Node, cap: usize) -> Vec<String> {
    fn walk(node: &Node, depth: usize, out: &mut Vec<String>, left: &mut usize) {
        if *left == 0 {
            return;
        }
        *left -= 1;
        let indent = "  ".repeat(depth);
        match node {
            Node::Document {
                doctype, children, ..
            } => {
                if let Some(dt) = doctype {
                    out.push(format!("{indent}<!DOCTYPE {dt}>"));
                } else {
                    out.push(format!("{indent}#document"));
                }
                for c in children {
                    walk(c, depth + 1, out, left);
                }
            }
            Node::Element {
                name,
                attributes,
                children,
                style,
                ..
            } => {
                let id = attributes
                    .iter()
                    .find(|(k, _)| k == "id")
                    .and_then(|(_, v)| v.as_deref())
                    .unwrap_or("");
                let class = attributes
                    .iter()
                    .find(|(k, _)| k == "class")
                    .and_then(|(_, v)| v.as_deref())
                    .unwrap_or("");
                let styl = first_styles(style);
                let mut line = format!("{indent}<{name}");
                if !id.is_empty() {
                    line.push_str(&format!(r#" id="{id}""#));
                }
                if !class.is_empty() {
                    line.push_str(&format!(r#" class="{class}""#));
                }
                line.push('>');
                if !styl.is_empty() {
                    line.push_str(&format!("  /* {styl} */"));
                }
                out.push(line);
                for c in children {
                    walk(c, depth + 1, out, left);
                }
            }
            Node::Text { text, .. } => {
                let t = text.replace('\n', " ").trim().to_string();
                if !t.is_empty() {
                    let show = if t.len() > 40 {
                        format!("{}…", &t[..40])
                    } else {
                        t
                    };
                    out.push(format!("{indent}\"{show}\""));
                }
            }
            Node::Comment { text, .. } => {
                let t = text.replace('\n', " ");
                let show = if t.len() > 40 {
                    format!("{}…", &t[..40])
                } else {
                    t
                };
                out.push(format!("{indent}<!-- {show} -->"));
            }
        }
    }
    let mut out = Vec::new();
    let mut left = cap;
    walk(root, 0, &mut out, &mut left);
    out
}

pub fn is_non_rendering_element(node: &Node) -> bool {
    match node {
        Node::Element { name, .. } => {
            let n = name.to_ascii_lowercase();
            matches!(
                n.as_str(),
                "head" | "style" | "script" | "title" | "meta" | "link"
            )
        }
        _ => false,
    }
}

pub fn assign_node_ids(root: &mut Node) {
    fn walk(node: &mut Node, next: &mut u32) {
        // only assign if currently unset
        let needs_id = node.id() == Id(0);

        if needs_id {
            let id = Id(*next);
            *next = next.wrapping_add(1);
            node.set_id(id); // or match/set like you already do
        }

        match node {
            Node::Document { children, .. } | Node::Element { children, .. } => {
                for c in children {
                    walk(c, next);
                }
            }
            _ => {}
        }
    }

    let mut next = 1;
    walk(root, &mut next);
}

pub fn find_node_by_id<'a>(node: &'a Node, id: Id) -> Option<&'a Node> {
    if node.id() == id {
        return Some(node);
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for c in children {
                if let Some(found) = find_node_by_id(c, id) {
                    return Some(found);
                }
            }
        }
        _ => {}
    }
    None
}

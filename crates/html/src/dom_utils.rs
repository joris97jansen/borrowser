use crate::{Id, Node};

fn is_heading(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 2 && (b[0] | 0x20) == b'h' && (b'1'..=b'6').contains(&b[1])
}

fn is_blockish_break(name: &str) -> bool {
    name.eq_ignore_ascii_case("p")
        || name.eq_ignore_ascii_case("div")
        || name.eq_ignore_ascii_case("section")
        || name.eq_ignore_ascii_case("article")
        || name.eq_ignore_ascii_case("header")
        || name.eq_ignore_ascii_case("footer")
        || name.eq_ignore_ascii_case("li")
        || is_heading(name)
}

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
        Node::Element { name, children, .. } => {
            if name.eq_ignore_ascii_case("link")
                && node.attr_has_token("rel", "stylesheet")
                && let Some(href) = node.attr("href")
            {
                out.push(href.to_string());
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
        Node::Element { name, children, .. } => {
            if name.eq_ignore_ascii_case("img")
                && let Some(src) = node.attr("src")
            {
                let src = src.trim();
                if !src.is_empty() {
                    out.push(src.to_string());
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

            if is_blockish_break(name) && !out.ends_with("\n\n") {
                out.push_str("\n\n");
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

pub fn collect_visible_text_string(root: &Node) -> String {
    let mut out = String::new();
    let mut ancestors = Vec::new();
    collect_visible_text(root, &mut ancestors, &mut out);

    // Trim trailing whitespace without allocating a new String.
    while out.ends_with(|c: char| c.is_whitespace()) {
        out.pop();
    }

    // Trim leading whitespace in place.
    let trimmed = out.trim_start_matches(|c: char| c.is_whitespace());
    let prefix_len = out.len().saturating_sub(trimmed.len());
    if prefix_len > 0 {
        out.drain(..prefix_len);
    }

    out
}

pub fn first_styles(style: &[(String, String)]) -> String {
    let mut out = String::new();
    for (i, (k, v)) in style.iter().take(3).enumerate() {
        if i != 0 {
            out.push(' ');
        }
        use std::fmt::Write;
        let _ = write!(&mut out, r#"{k}: {v};"#);
    }
    out
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
                children,
                style,
                ..
            } => {
                let id = node.attr("id").unwrap_or("");
                let class = node.attr("class").unwrap_or("");
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
            name.eq_ignore_ascii_case("head")
                || name.eq_ignore_ascii_case("style")
                || name.eq_ignore_ascii_case("script")
                || name.eq_ignore_ascii_case("title")
                || name.eq_ignore_ascii_case("meta")
                || name.eq_ignore_ascii_case("link")
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

pub fn find_node_by_id(node: &Node, id: Id) -> Option<&Node> {
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

use crate::Node;

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

// `ancestors` is threaded for future context-aware extraction (e.g., spacing rules, aria).
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

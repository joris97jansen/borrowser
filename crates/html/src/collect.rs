use crate::Node;
use crate::types::debug_assert_lowercase_atom;

fn is_heading(name: &str) -> bool {
    debug_assert_lowercase_atom(name, "collect heading tag");
    let b = name.as_bytes();
    b.len() == 2 && b[0] == b'h' && (b'1'..=b'6').contains(&b[1])
}

fn is_blockish_break(name: &str) -> bool {
    name == "p"
        || name == "div"
        || name == "section"
        || name == "article"
        || name == "header"
        || name == "footer"
        || name == "main"
        || name == "nav"
        || name == "aside"
        || name == "table"
        || name == "thead"
        || name == "tbody"
        || name == "tfoot"
        || name == "li"
        || is_heading(name)
}

#[inline]
fn is_ascii_ws(byte: u8) -> bool {
    matches!(byte, b' ' | b'\n' | b'\t' | b'\r')
}

fn trim_ascii(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut start = 0usize;
    while start < bytes.len() && is_ascii_ws(bytes[start]) {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && is_ascii_ws(bytes[end - 1]) {
        end -= 1;
    }
    &s[start..end]
}

fn trim_ascii_end(s: &mut String) {
    let bytes = s.as_bytes();
    let mut end = bytes.len();
    while end > 0 && is_ascii_ws(bytes[end - 1]) {
        end -= 1;
    }
    s.truncate(end);
}

fn trim_ascii_end_preserve_newlines(s: &mut String) {
    let bytes = s.as_bytes();
    let mut end = bytes.len();
    while end > 0 {
        match bytes[end - 1] {
            b' ' | b'\t' | b'\r' => end -= 1,
            _ => break,
        }
    }
    s.truncate(end);
}

fn ensure_paragraph_boundary_before_block(out: &mut String) {
    if out.is_empty() {
        return;
    }
    trim_ascii_end_preserve_newlines(out);
    if out.ends_with('\n') && !out.ends_with("\n\n") {
        out.push('\n');
    }
}

/// Collect concatenated text from <style> elements.
pub fn collect_style_texts(node: &Node, out: &mut String) {
    match node {
        Node::Element { name, children, .. } if name.as_ref() == "style" => {
            debug_assert_lowercase_atom(name, "collect style tag");
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
            debug_assert_lowercase_atom(name, "collect stylesheet tag");
            if name.as_ref() == "link"
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
            debug_assert_lowercase_atom(name, "collect img tag");
            if name.as_ref() == "img"
                && let Some(src) = node.attr("src")
            {
                let src = trim_ascii(src);
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

pub fn collect_visible_text(node: &Node, out: &mut String) {
    fn push_break(out: &mut String, break_str: &str) {
        if out.is_empty() {
            return;
        }
        trim_ascii_end_preserve_newlines(out);
        if break_str == "\n\n" && out.ends_with('\n') && !out.ends_with("\n\n") {
            out.push('\n');
            return;
        }
        if !out.ends_with(break_str) {
            out.push_str(break_str);
        }
    }

    fn break_kind(name: &str) -> Option<&'static str> {
        if name == "br" {
            return Some("\n");
        }
        if name == "hr" || is_blockish_break(name) {
            return Some("\n\n");
        }
        if name == "tr" || name == "td" || name == "th" {
            return Some("\n");
        }
        None
    }

    match node {
        Node::Text { text, .. } => {
            let t = trim_ascii(text);
            if !t.is_empty() {
                if out.as_bytes().last().is_some_and(|b| !is_ascii_ws(*b)) {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
        Node::Element { name, children, .. } => {
            debug_assert_lowercase_atom(name, "collect visible text tag");
            if name.as_ref() == "script" || name.as_ref() == "style" {
                return; // skip
            }

            if let Some("\n\n") = break_kind(name.as_ref()) {
                ensure_paragraph_boundary_before_block(out);
            }
            for c in children {
                collect_visible_text(c, out);
            }

            if let Some(break_str) = break_kind(name.as_ref()) {
                // Breaks are inserted after children (post-order), which may
                // introduce extra spacing for nested block elements. Trimming
                // in `push_break` keeps output deterministic.
                push_break(out, break_str);
            }
        }
        Node::Document { children, .. } => {
            for c in children {
                collect_visible_text(c, out);
            }
        }
        _ => {}
    }
}

/// Collects visible text for utility extraction.
///
/// - Trims each text node and joins non-empty nodes with single spaces.
/// - Skips text in <script> and <style>.
/// - Inserts a paragraph break (`\n\n`) after blockish elements (and <hr>).
/// - Inserts a single newline (`\n`) after <br>.
/// - Trims leading/trailing ASCII whitespace from the final output.
pub fn collect_visible_text_string(root: &Node) -> String {
    let mut out = String::new();
    collect_visible_text(root, &mut out);

    // Trim trailing ASCII whitespace without allocating a new String.
    trim_ascii_end(&mut out);

    // Trim leading ASCII whitespace in place.
    let prefix_len = out
        .as_bytes()
        .iter()
        .position(|b| !is_ascii_ws(*b))
        .unwrap_or(out.len());
    if prefix_len > 0 {
        out.drain(..prefix_len);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::collect_visible_text_string;
    use crate::Node;
    use crate::types::Id;
    use std::sync::Arc;

    fn text_node(text: &str) -> Node {
        Node::Text {
            id: Id(0),
            text: text.to_string(),
        }
    }

    fn elem(name: &str, children: Vec<Node>) -> Node {
        Node::Element {
            id: Id(0),
            name: Arc::<str>::from(name),
            attributes: Vec::new(),
            style: Vec::new(),
            children,
        }
    }

    fn doc(children: Vec<Node>) -> Node {
        Node::Document {
            id: Id(0),
            doctype: None,
            children,
        }
    }

    #[test]
    fn inline_elements_join_with_single_spaces() {
        let root = doc(vec![
            text_node(" Hello "),
            elem("span", vec![text_node("world")]),
            elem("em", vec![text_node(" again ")]),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello world again");
    }

    #[test]
    fn block_breaks_are_inserted_for_common_elements() {
        let root = doc(vec![
            elem("p", vec![text_node("One")]),
            elem("div", vec![text_node("Two")]),
            elem("section", vec![text_node("Three")]),
            elem("article", vec![text_node("Four")]),
            elem("header", vec![text_node("Five")]),
            elem("footer", vec![text_node("Six")]),
            elem("li", vec![text_node("Seven")]),
            elem("h1", vec![text_node("Eight")]),
            elem("h6", vec![text_node("Nine")]),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(
            out,
            "One\n\nTwo\n\nThree\n\nFour\n\nFive\n\nSix\n\nSeven\n\nEight\n\nNine"
        );
    }

    #[test]
    fn script_and_style_are_skipped() {
        let root = doc(vec![
            text_node("Visible"),
            elem("script", vec![text_node("Hidden")]),
            elem("style", vec![text_node("Hidden too")]),
            elem("span", vec![text_node("Also")]),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Visible Also");
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        let root = doc(vec![text_node("   Hello world   ")]);
        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello world");
    }

    #[test]
    fn consecutive_block_breaks_do_not_run_away() {
        let root = doc(vec![
            elem("div", vec![text_node("One")]),
            elem("div", vec![]),
            elem("div", vec![text_node("Two")]),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "One\n\nTwo");
    }

    #[test]
    fn block_breaks_do_not_add_spaces_before_newlines() {
        let root = doc(vec![
            elem("div", vec![text_node("Hello")]),
            elem("div", vec![text_node("World")]),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello\n\nWorld");
    }

    #[test]
    fn whitespace_only_text_nodes_do_not_create_double_spaces() {
        let root = doc(vec![
            text_node("Hello"),
            text_node("   "),
            text_node("world"),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello world");
    }

    #[test]
    fn br_inserts_single_newline() {
        let root = doc(vec![
            text_node("Hello"),
            elem("br", vec![]),
            text_node("world"),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello\nworld");
    }

    #[test]
    fn hr_inserts_paragraph_break() {
        let root = doc(vec![
            text_node("Hello"),
            elem("hr", vec![]),
            text_node("world"),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello\n\nworld");
    }

    #[test]
    fn br_then_block_results_in_single_paragraph_break() {
        let root = doc(vec![
            text_node("Hello"),
            elem("br", vec![]),
            elem("div", vec![text_node("World")]),
        ]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "Hello\n\nWorld");
    }

    #[test]
    fn nested_block_breaks_do_not_double_insert() {
        let root = doc(vec![elem(
            "div",
            vec![
                elem("div", vec![text_node("One")]),
                elem("div", vec![text_node("Two")]),
            ],
        )]);

        let out = collect_visible_text_string(&root);
        assert_eq!(out, "One\n\nTwo");
    }
}

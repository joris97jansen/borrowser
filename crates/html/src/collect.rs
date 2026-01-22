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

#[inline]
fn is_ascii_ws(byte: u8) -> bool {
    matches!(byte, b' ' | b'\n' | b'\t' | b'\r')
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

pub fn collect_visible_text(node: &Node, out: &mut String) {
    fn push_block_break(out: &mut String) {
        if out.is_empty() {
            return;
        }
        while out.as_bytes().last().is_some_and(|b| is_ascii_ws(*b)) {
            out.pop();
        }
        out.push_str("\n\n");
    }

    match node {
        Node::Text { text, .. } => {
            let t = text.trim();
            if !t.is_empty() {
                if out.as_bytes().last().is_some_and(|b| !is_ascii_ws(*b)) {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
        Node::Element { name, children, .. } => {
            if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
                return; // skip
            }
            for c in children {
                collect_visible_text(c, out);
            }

            if is_blockish_break(name) {
                push_block_break(out);
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
/// - Inserts a paragraph break (`\n\n`) after blockish elements.
/// - Trims leading/trailing ASCII whitespace from the final output.
pub fn collect_visible_text_string(root: &Node) -> String {
    let mut out = String::new();
    collect_visible_text(root, &mut out);

    // Trim trailing ASCII whitespace without allocating a new String.
    while out.as_bytes().last().is_some_and(|b| is_ascii_ws(*b)) {
        out.pop();
    }

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
}

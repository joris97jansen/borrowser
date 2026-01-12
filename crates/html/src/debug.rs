use crate::Node;

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
    fn preview_replace_newlines(s: &str, max_chars: usize) -> String {
        let mut out = String::new();
        let mut truncated = false;
        for (i, ch) in s.chars().enumerate() {
            if i == max_chars {
                truncated = true;
                break;
            }
            if ch == '\n' {
                out.push(' ');
            } else {
                out.push(ch);
            }
        }
        if truncated {
            out.push('…');
        }
        out
    }

    const INDENT_STEP: &str = "  ";

    fn walk(node: &Node, indent: &mut String, out: &mut Vec<String>, left: &mut usize) {
        if *left == 0 {
            return;
        }
        *left -= 1;
        match node {
            Node::Document {
                doctype, children, ..
            } => {
                let mut line = String::new();
                line.push_str(indent);
                if let Some(dt) = doctype {
                    line.push_str("<!DOCTYPE ");
                    line.push_str(dt);
                    line.push('>');
                } else {
                    line.push_str("#document");
                }
                out.push(line);
                indent.push_str(INDENT_STEP);
                for c in children {
                    walk(c, indent, out, left);
                }
                indent.truncate(indent.len() - INDENT_STEP.len());
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
                let mut line = String::new();
                line.push_str(indent);
                line.push('<');
                line.push_str(name);
                if !id.is_empty() {
                    line.push_str(r#" id=""#);
                    line.push_str(id);
                    line.push('"');
                }
                if !class.is_empty() {
                    line.push_str(r#" class=""#);
                    line.push_str(class);
                    line.push('"');
                }
                line.push('>');
                if !styl.is_empty() {
                    line.push_str("  /* ");
                    line.push_str(&styl);
                    line.push_str(" */");
                }
                out.push(line);
                indent.push_str(INDENT_STEP);
                for c in children {
                    walk(c, indent, out, left);
                }
                indent.truncate(indent.len() - INDENT_STEP.len());
            }
            Node::Text { text, .. } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let show = preview_replace_newlines(trimmed, 40);
                    let mut line = String::new();
                    line.push_str(indent);
                    line.push('"');
                    line.push_str(&show);
                    line.push('"');
                    out.push(line);
                }
            }
            Node::Comment { text, .. } => {
                let show = preview_replace_newlines(text, 40);
                let mut line = String::new();
                line.push_str(indent);
                line.push_str("<!-- ");
                line.push_str(&show);
                line.push_str(" -->");
                out.push(line);
            }
        }
    }
    let mut out = Vec::new();
    let mut left = cap;
    let mut indent = String::new();
    walk(root, &mut indent, &mut out, &mut left);
    out
}

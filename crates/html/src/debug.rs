use crate::Node;
use std::fmt::Write;

pub fn first_styles(style: &[(String, String)]) -> String {
    let mut out = String::new();
    for (i, (k, v)) in style.iter().take(3).enumerate() {
        if i != 0 {
            out.push(' ');
        }
        let _ = write!(&mut out, r#"{k}: {v};"#);
    }
    out
}

pub fn outline_from_dom(root: &Node, cap: usize) -> Vec<String> {
    struct IndentGuard<'a> {
        indent: &'a mut String,
        step: usize,
    }

    impl Drop for IndentGuard<'_> {
        fn drop(&mut self) {
            let new_len = self.indent.len() - self.step;
            self.indent.truncate(new_len);
        }
    }

    fn trimmed_nonempty_slice(s: &str) -> Option<&str> {
        let start = match s.char_indices().find(|&(_, ch)| !ch.is_whitespace()) {
            Some((idx, _)) => idx,
            None => return None,
        };
        let end = s
            .char_indices()
            .rev()
            .find(|&(_, ch)| !ch.is_whitespace())
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(start);
        Some(&s[start..end])
    }

    fn push_preview_replace_newlines(out: &mut String, s: &str, max_chars: usize) {
        let mut truncated = false;
        for (i, ch) in s.chars().enumerate() {
            if i == max_chars {
                truncated = true;
                break;
            }
            out.push(if ch == '\n' { ' ' } else { ch });
        }
        if truncated {
            out.push('…');
        }
    }

    const INDENT_STEP: &str = "  ";
    const PREVIEW_CHARS: usize = 40;

    fn walk(node: &Node, indent: &mut String, out: &mut Vec<String>, left: &mut usize) {
        if *left == 0 {
            return;
        }
        *left -= 1;
        match node {
            Node::Document {
                doctype, children, ..
            } => {
                let mut line = String::with_capacity(indent.len() + 64);
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
                let _guard = IndentGuard {
                    indent,
                    step: INDENT_STEP.len(),
                };
                for c in children {
                    walk(c, indent, out, left);
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
                let mut line = String::with_capacity(indent.len() + 64);
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
                let _guard = IndentGuard {
                    indent,
                    step: INDENT_STEP.len(),
                };
                for c in children {
                    walk(c, indent, out, left);
                }
            }
            Node::Text { text, .. } => {
                if let Some(trimmed) = trimmed_nonempty_slice(text) {
                    let mut line = String::with_capacity(indent.len() + 64);
                    line.push_str(indent);
                    line.push('"');
                    push_preview_replace_newlines(&mut line, trimmed, PREVIEW_CHARS);
                    line.push('"');
                    out.push(line);
                }
            }
            Node::Comment { text, .. } => {
                let mut line = String::with_capacity(indent.len() + 64);
                line.push_str(indent);
                line.push_str("<!-- ");
                push_preview_replace_newlines(&mut line, text, PREVIEW_CHARS);
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

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
    fn preview(s: &str, max_chars: usize) -> String {
        let mut out = String::new();
        let mut truncated = false;
        for (i, ch) in s.chars().enumerate() {
            if i == max_chars {
                truncated = true;
                break;
            }
            out.push(ch);
        }
        if truncated {
            out.push('…');
        }
        out
    }

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
                    let show = preview(&t, 40);
                    out.push(format!("{indent}\"{show}\""));
                }
            }
            Node::Comment { text, .. } => {
                let t = text.replace('\n', " ");
                let show = preview(&t, 40);
                out.push(format!("{indent}<!-- {show} -->"));
            }
        }
    }
    let mut out = Vec::new();
    let mut left = cap;
    walk(root, 0, &mut out, &mut left);
    out
}

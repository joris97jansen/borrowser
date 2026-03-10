use html_test_support::escape_text;

pub(crate) fn format_patch_batches(batches: &[Vec<html::DomPatch>]) -> Vec<String> {
    let mut lines = Vec::new();
    for (batch_index, batch) in batches.iter().enumerate() {
        lines.push(format!("Batch index={batch_index} size={}", batch.len()));
        for patch in batch {
            lines.push(format_patch(patch));
        }
    }
    lines
}

fn format_patch(patch: &html::DomPatch) -> String {
    match patch {
        html::DomPatch::Clear => "Clear".to_string(),
        html::DomPatch::CreateDocument { key, doctype } => match doctype {
            Some(value) => {
                format!(
                    "CreateDocument key={} doctype=\"{}\"",
                    key.0,
                    escape_text(value)
                )
            }
            None => format!("CreateDocument key={} doctype=<none>", key.0),
        },
        html::DomPatch::CreateElement {
            key,
            name,
            attributes,
        } => {
            let attrs = format_attributes(attributes);
            format!(
                "CreateElement key={} name={} attrs=[{}]",
                key.0, name, attrs
            )
        }
        html::DomPatch::CreateText { key, text } => {
            format!("CreateText key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::CreateComment { key, text } => {
            format!("CreateComment key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::AppendChild { parent, child } => {
            format!("AppendChild parent={} child={}", parent.0, child.0)
        }
        html::DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => {
            format!(
                "InsertBefore parent={} child={} before={}",
                parent.0, child.0, before.0
            )
        }
        html::DomPatch::RemoveNode { key } => format!("RemoveNode key={}", key.0),
        html::DomPatch::SetAttributes { key, attributes } => {
            let attrs = format_attributes(attributes);
            format!("SetAttributes key={} attrs=[{}]", key.0, attrs)
        }
        html::DomPatch::SetText { key, text } => {
            format!("SetText key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::AppendText { key, text } => {
            format!("AppendText key={} text=\"{}\"", key.0, escape_text(text))
        }
        other => panic!("unhandled DomPatch variant in golden formatter: {other:?}"),
    }
}

fn format_attributes(attributes: &[(std::sync::Arc<str>, Option<String>)]) -> String {
    if attributes.is_empty() {
        return String::new();
    }

    let mut sorted = attributes
        .iter()
        .map(|(name, value)| (name.as_ref(), value.as_deref()))
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));

    let mut out = String::new();
    for (index, (name, value)) in sorted.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(name);
        out.push('=');
        match value {
            Some(value) => {
                out.push('"');
                out.push_str(&escape_text(value));
                out.push('"');
            }
            None => out.push_str("<none>"),
        }
    }
    out
}

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
        html::DomPatch::CreateDocumentType {
            key,
            name,
            public_id,
            system_id,
        } => {
            format!(
                "CreateDocumentType key={} name={} public-id={} system-id={}",
                key.0,
                format_optional_text(name.as_deref()),
                format_optional_text(public_id.as_deref()),
                format_optional_text(system_id.as_deref())
            )
        }
        html::DomPatch::CreateElement {
            key,
            name,
            attributes,
        } => {
            let attrs = format_attributes(attributes);
            format!(
                "CreateElement key={} ns={} local=\"{}\" attrs=[{}]",
                key.0,
                name.namespace().snapshot_name(),
                escape_text(name.local_name_str()),
                attrs
            )
        }
        html::DomPatch::CreateTemplateContents { host, contents } => {
            format!(
                "CreateTemplateContents host={} contents={}",
                host.0, contents.0
            )
        }
        html::DomPatch::CreateText { key, text } => {
            format!("CreateText key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::CreateComment { key, text } => {
            format!("CreateComment key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::CreateProcessingInstruction { key, target, data } => {
            format!(
                "CreateProcessingInstruction key={} target=\"{}\" data=\"{}\"",
                key.0,
                escape_text(target),
                escape_text(data)
            )
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

fn format_optional_text(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", escape_text(value)),
        None => "<none>".to_string(),
    }
}

fn format_attributes(attributes: &[html::ParserCreatedAttribute]) -> String {
    if attributes.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for (index, attribute) in attributes.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str("{ns=");
        out.push_str(attribute.namespace().snapshot_name());
        out.push_str(" prefix=");
        if let Some(prefix) = attribute.prefix() {
            out.push('"');
            out.push_str(prefix);
            out.push('"');
        } else {
            out.push('-');
        }
        out.push_str(" local=\"");
        out.push_str(&escape_text(attribute.local_name()));
        out.push_str("\" value=\"");
        out.push_str(&escape_text(attribute.value()));
        out.push_str("\"}");
    }
    out
}

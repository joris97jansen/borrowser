mod foster;

pub(super) use super::location::InsertionLocation;
use crate::dom_patch::PatchKey;
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::{TextResolveError, TextResolver};
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::stack::OpenElement;

struct EmptyResolver;

impl TextResolver for EmptyResolver {
    fn resolve_span(&self, span: crate::html5::shared::TextSpan) -> Result<&str, TextResolveError> {
        Err(TextResolveError::InvalidSpan { span })
    }
}

fn bootstrap_html_body(
    builder: &mut Html5TreeBuilder,
    ctx: &DocumentParseContext,
) -> (PatchKey, PatchKey) {
    builder
        .with_structural_mutation(|this| {
            let document = this.ensure_document_created()?;
            let html = this
                .create_detached_element(this.known_tags.html, &[], &ctx.atoms)?
                .expect("html bootstrap should not hit resource limits");
            this.append_existing_child(document, html);
            this.open_elements
                .push(OpenElement::new(html, this.known_tags.html));

            let body = this
                .create_detached_element(this.known_tags.body, &[], &ctx.atoms)?
                .expect("body bootstrap should not hit resource limits");
            this.append_existing_child(html, body);
            this.open_elements
                .push(OpenElement::new(body, this.known_tags.body));
            Ok((html, body))
        })
        .expect("bootstrap should remain recoverable")
}

fn attach_live_table(
    builder: &mut Html5TreeBuilder,
    ctx: &DocumentParseContext,
    body: PatchKey,
) -> PatchKey {
    builder
        .with_structural_mutation(|this| {
            let table = this
                .create_detached_element(this.known_tags.table, &[], &ctx.atoms)?
                .expect("table setup should not hit resource limits");
            this.append_existing_child(body, table);
            this.open_elements
                .push(OpenElement::new(table, this.known_tags.table));
            Ok(table)
        })
        .expect("live table attach should remain recoverable")
}

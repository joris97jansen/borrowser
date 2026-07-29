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
    ctx: &mut DocumentParseContext,
) -> (PatchKey, PatchKey) {
    let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
    let atoms = context.atoms();
    builder
        .with_structural_mutation(|this| {
            let document = this.ensure_document_created(&mut context)?;
            let html = this
                .create_detached_element(this.known_tags.html, &[], &mut context, atoms)?
                .expect("html bootstrap should not hit resource limits");
            this.append_existing_child(document, html, &mut context);
            this.open_elements
                .push(OpenElement::new_html(html, this.known_tags.html));

            let body = this
                .create_detached_element(this.known_tags.body, &[], &mut context, atoms)?
                .expect("body bootstrap should not hit resource limits");
            this.append_existing_child(html, body, &mut context);
            this.open_elements
                .push(OpenElement::new_html(body, this.known_tags.body));
            Ok((html, body))
        })
        .expect("bootstrap should remain recoverable")
}

fn attach_live_table(
    builder: &mut Html5TreeBuilder,
    ctx: &mut DocumentParseContext,
    body: PatchKey,
) -> PatchKey {
    let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
    let atoms = context.atoms();
    builder
        .with_structural_mutation(|this| {
            let table = this
                .create_detached_element(this.known_tags.table, &[], &mut context, atoms)?
                .expect("table setup should not hit resource limits");
            this.append_existing_child(body, table, &mut context);
            this.open_elements
                .push(OpenElement::new_html(table, this.known_tags.table));
            Ok(table)
        })
        .expect("live table attach should remain recoverable")
}

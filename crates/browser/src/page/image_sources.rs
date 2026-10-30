use super::PageState;
use addressing::BrowserUrl;
use html::{ElementNamespace, Node, internal::Id};
use std::collections::HashMap;

// Raw authored data is retained for mutation comparison, never formatted by
// production diagnostics. Resource cache identity remains the selected URL.
pub(crate) struct ImageSourceBinding {
    authored_source: String,
    resolved_url: Option<BrowserUrl>,
}

impl PageState {
    pub(crate) fn reconcile_image_sources(&mut self) -> Vec<String> {
        let mut old = std::mem::take(&mut self.image_sources);
        let mut next = HashMap::new();
        let mut requests = Vec::new();
        let base = self
            .addresses
            .as_ref()
            .map(super::DocumentAddressState::active_base_url);
        let mut stack: Vec<&Node> = self.dom.as_deref().into_iter().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Document { children, .. } => stack.extend(children.iter().rev()),
                Node::Element { element } => {
                    stack.extend(element.children().iter().rev());
                    if element.expanded_name().is(ElementNamespace::Html, "img")
                        && let Some(source) = node.attr("src")
                    {
                        let retained = old
                            .remove(&node.id())
                            .filter(|binding| binding.authored_source == source);
                        let binding = retained.unwrap_or_else(|| ImageSourceBinding {
                            authored_source: source.to_owned(),
                            resolved_url: crate::resources::resolve_image_source(base, source),
                        });
                        if let Some(url) = &binding.resolved_url {
                            requests.push(url.as_str().to_owned());
                        }
                        next.insert(node.id(), binding);
                    }
                }
                _ => {}
            }
        }
        // Old owners, including removed subtrees and source-less images, retire
        // here. Shared ResourceManager cache entries have their own lifetime.
        self.image_sources = next;
        requests
    }
}

pub(crate) fn selected_image_source(
    bindings: &HashMap<Id, ImageSourceBinding>,
    node_id: Id,
    source: &str,
) -> Option<String> {
    let binding = bindings.get(&node_id)?;
    if binding.authored_source != source {
        return None;
    }
    binding
        .resolved_url
        .as_ref()
        .map(|url| url.as_str().to_owned())
}

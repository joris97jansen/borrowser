use crate::Node;

#[derive(Debug, Clone, Default)]
pub struct HeadMetadata {
    pub title: Option<String>,
    pub meta: Vec<MetaTag>,
    pub links: Vec<LinkTag>,
    pub base_href: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MetaTag {
    pub name: Option<String>,     // e.g. name="description"
    pub property: Option<String>, // e.g. property="og:title"
    pub content: Option<String>,  // e.g. content="Some text"
}

#[derive(Debug, Clone)]
pub struct LinkTag {
    pub rel: Vec<String>,         // e.g. ["icon"], ["stylesheet"]
    pub href: Option<String>,
}

pub fn extract_head_metadata(dom: &Node) -> HeadMetadata {
    let mut meta = HeadMetadata::default();

    // Find <head> inside the document
    let head = find_head(dom);
    if let Some(head_node) = head {
        fill_head_metadata_from(head_node, &mut meta);
    }

    meta
}

fn find_head(dom: &Node) -> Option<&Node> {
    match dom {
        Node::Document { children, .. } => {
            for child in children {
                if let Node::Element { name, children: html_children, .. } = child {
                    if name.eq_ignore_ascii_case("html") {
                        // search inside <html> for <head>
                        for hc in html_children {
                            if let Node::Element { name, .. } = hc {
                                if name.eq_ignore_ascii_case("head") {
                                    return Some(hc);
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn fill_head_metadata_from(head: &Node, out: &mut HeadMetadata) {
    if let Node::Element { children, .. } = head {
        for child in children {
            if let Node::Element { name, attributes, children, .. } = child {
                // <title>
                if name.eq_ignore_ascii_case("title") {
                    if out.title.is_none() {
                        if let Some(text) = first_text_child(children) {
                            out.title = Some(text);
                        }
                    }
                }

                // <meta>
                if name.eq_ignore_ascii_case("meta") {
                    let tag = MetaTag {
                        name: get_attr(attributes, "name").map(|s| s.to_string()),
                        property: get_attr(attributes, "property").map(|s| s.to_string()),
                        content: get_attr(attributes, "content").map(|s| s.to_string()),
                    };
                    if tag.name.is_some() || tag.property.is_some() || tag.content.is_some() {
                        out.meta.push(tag);
                    }
                }

                // <link>
                if name.eq_ignore_ascii_case("link") {
                    let rel_raw = get_attr(attributes, "rel").unwrap_or("");
                    let rels = rel_raw
                        .split_whitespace()
                        .map(|s| s.to_ascii_lowercase())
                        .collect::<Vec<_>>();

                    let href = get_attr(attributes, "href").map(|s| s.to_string());

                    if !rels.is_empty() || href.is_some() {
                        out.links.push(LinkTag { rel: rels, href });
                    }
                }

                // <base>
                if name.eq_ignore_ascii_case("base") {
                    if out.base_href.is_none() {
                        if let Some(h) = get_attr(attributes, "href") {
                            out.base_href = Some(h.to_string());
                        }
                    }
                }
            }
        }
    }
}

fn get_attr<'a>(attrs: &'a [(String, Option<String>)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_deref())
}

fn first_text_child(children: &[Node]) -> Option<String> {
    for c in children {
        if let Node::Text { text } = c {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
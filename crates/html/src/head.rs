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
    pub rel: Vec<String>, // e.g. ["icon"], ["stylesheet"]
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
    let Node::Document { children, .. } = dom else {
        return None;
    };

    for child in children {
        let Node::Element {
            name,
            children: html_children,
            ..
        } = child
        else {
            continue;
        };

        if !name.eq_ignore_ascii_case("html") {
            continue;
        }

        // search inside <html> for <head>
        for hc in html_children {
            let Node::Element { name, .. } = hc else {
                continue;
            };

            if name.eq_ignore_ascii_case("head") {
                return Some(hc);
            }
        }
    }

    None
}

fn fill_head_metadata_from(head: &Node, out: &mut HeadMetadata) {
    if let Node::Element { children, .. } = head {
        for child in children {
            if let Node::Element { name, children, .. } = child {
                // <title>
                if name.eq_ignore_ascii_case("title") && out.title.is_none() {
                    out.title = first_text_child(children);
                }

                // <meta>
                if name.eq_ignore_ascii_case("meta") {
                    let tag = MetaTag {
                        name: child.attr("name").map(|s| s.to_string()),
                        property: child.attr("property").map(|s| s.to_string()),
                        content: child.attr("content").map(|s| s.to_string()),
                    };
                    if tag.name.is_some() || tag.property.is_some() || tag.content.is_some() {
                        out.meta.push(tag);
                    }
                }

                // <link>
                if name.eq_ignore_ascii_case("link") {
                    let rel_raw = child.attr("rel").unwrap_or("");
                    let rels = rel_raw
                        .split_whitespace()
                        .map(|s| s.to_ascii_lowercase())
                        .collect::<Vec<_>>();

                    let href = child.attr("href").map(|s| s.to_string());

                    if !rels.is_empty() || href.is_some() {
                        out.links.push(LinkTag { rel: rels, href });
                    }
                }

                // <base>
                if name.eq_ignore_ascii_case("base") && out.base_href.is_none() {
                    out.base_href = child.attr("href").map(|h| h.to_string());
                }
            }
        }
    }
}

fn first_text_child(children: &[Node]) -> Option<String> {
    for c in children {
        if let Node::Text { text, .. } = c {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

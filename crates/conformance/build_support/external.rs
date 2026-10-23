//! Build-time source authority checks, using the synchronous production parser.
//! This never starts runtime workers and never executes a test page.
#[path = "../src/metadata_links.rs"]
pub mod links;
use html::{AttributeNamespace, ElementNamespace, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct License {
    pub path: String,
    pub upstream_path: String,
    pub sha256: String,
    pub spdx: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub repository: String,
    pub revision: String,
    pub path: String,
    pub sha256: String,
    pub license: License,
    pub attribution: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamAssertion {
    pub form: String,
    pub unsupported_reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub kind: String,
    pub input: String,
    pub provenance: Option<Provenance>,
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn validate_provenance(p: &Provenance, bytes: &[u8]) -> Result<(), String> {
    if p.repository != "https://github.com/web-platform-tests/wpt"
        || p.revision.len() != 40
        || !p
            .revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || p.sha256 != sha256(bytes)
        || p.attribution.trim().is_empty()
        || p.license.spdx != "BSD-3-Clause"
        || p.license.upstream_path != "LICENSE.md"
    {
        return Err("invalid upstream repository/revision/hash/licence/attribution".into());
    }
    super::relative(&p.path)?;
    Ok(())
}

#[derive(Default)]
pub struct Facts {
    pub links: Vec<links::LinkFact>,
    pub static_exclusions: Vec<&'static str>,
}

pub fn inspect(bytes: &[u8]) -> Result<Facts, String> {
    // Source input is bounded before parser allocation; traversal is also bounded.
    if bytes.len() > 64 * 1024 {
        return Err("external source exceeds 64 KiB".into());
    }
    let parsed = html::parse_document(bytes, html::HtmlParseOptions::default())
        .map_err(|e| format!("source HTML parse: {e:?}"))?;
    let mut facts = Facts::default();
    if parsed.document_mode != html::DocumentMode::NoQuirks {
        facts.static_exclusions.push("standards doctype required");
    }
    fn walk(node: &Node, depth: usize, count: &mut usize, facts: &mut Facts) -> Result<(), String> {
        *count += 1;
        if *count > 128 || depth > 32 {
            return Err("source metadata tree limit".into());
        }
        match node {
            Node::Document { children, .. } => {
                for c in children {
                    walk(c, depth + 1, count, facts)?;
                }
            }
            Node::Element { element: e } => {
                if e.attributes().len() > 32 {
                    return Err("source attribute limit".into());
                }
                if e.name().eq_ignore_ascii_case("link") {
                    let mut attributes: Vec<_> = e
                        .attributes()
                        .iter()
                        .map(|a| links::AttributeFact {
                            namespace: match a.namespace() {
                                AttributeNamespace::None => "",
                                AttributeNamespace::Xml => "http://www.w3.org/XML/1998/namespace",
                                AttributeNamespace::Xmlns => "http://www.w3.org/2000/xmlns/",
                                AttributeNamespace::XLink => "http://www.w3.org/1999/xlink",
                            }
                            .into(),
                            prefix: a.prefix().unwrap_or("").into(),
                            local_name: a.local_name().into(),
                            value: a.value().into(),
                        })
                        .collect();
                    attributes.sort();
                    facts.links.push(links::LinkFact {
                        namespace: match e.namespace() {
                            ElementNamespace::Html => links::HTML,
                            ElementNamespace::Svg => "http://www.w3.org/2000/svg",
                            ElementNamespace::MathMl => "http://www.w3.org/1998/Math/MathML",
                        }
                        .into(),
                        local_name: e.name().into(),
                        attributes,
                    });
                }
                if matches!(e.name(), "script" | "base") {
                    facts
                        .static_exclusions
                        .push("active script/base outside static relationship profile");
                }
                for a in e.attributes() {
                    if a.namespace() != AttributeNamespace::None {
                        continue;
                    }
                    if a.local_name() == "class"
                        && a.value()
                            .split_ascii_whitespace()
                            .any(|v| matches!(v, "reftest-wait" | "reftest-print"))
                    {
                        facts
                            .static_exclusions
                            .push("dynamic readiness/print reftest unsupported");
                    }
                    if e.name() == "meta"
                        && a.local_name() == "name"
                        && matches!(a.value(), "fuzzy" | "flags")
                    {
                        facts
                            .static_exclusions
                            .push("reftest fuzzy/flags metadata requires unsupported semantics");
                    }
                }
                // Deliberately do not enter template contents.
                for c in e.children() {
                    walk(c, depth + 1, count, facts)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&parsed.document, 0, &mut 0, &mut facts)?;
    Ok(facts)
}

pub fn relationships(facts: &Facts) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for link in &facts.links {
        if link.namespace != links::HTML {
            continue;
        }
        let relation = link.relation()?;
        if matches!(relation.as_str(), "match" | "mismatch") {
            out.push((
                relation,
                link.attribute("href")
                    .ok_or("relationship missing href")?
                    .into(),
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ag7_parser_ignores_comments_script_text_and_template_contents() {
        let facts = inspect(
            br#"<!doctype html><!-- <link rel=match href=wrong.html> -->
            <script>const s='<link rel=mismatch href=wrong.html>';</script>
            <template><link rel=match href=wrong.html></template>
            <link TITLE="A &amp; B" HREF="ref.html" REL=" MaTcH ">"#,
        )
        .unwrap();
        assert_eq!(
            relationships(&facts).unwrap(),
            vec![("match".into(), "ref.html".into())]
        );
        assert_eq!(facts.links[0].attribute("title"), Some("A & B"));
        assert_eq!(facts.links[0].attribute("href"), Some("ref.html"));
        links::validate(&facts.links).unwrap();
        assert!(!facts.static_exclusions.is_empty());
    }
    #[test]
    fn ag7_metadata_links_require_complete_reviewed_facts() {
        for attrs in [
            "rel='help stylesheet' href=x",
            "rel='match preload' href=ref.html",
            "rel='help help' href=x",
            "rel=icon href=x",
            "rel=unknown href=x",
            "rel=help href=x onload=x",
            "rel=help href=x media=print",
            "rel=help href=x xlink:href=y",
        ] {
            let html = format!("<!doctype html><link {attrs}>");
            let facts = inspect(html.as_bytes()).unwrap();
            assert!(links::validate(&facts.links).is_err(), "{attrs}");
        }
        for rel in ["help", "author", "match", "mismatch"] {
            let facts =
                inspect(format!("<!doctype html><link rel={rel} href=ref.html>").as_bytes())
                    .unwrap();
            links::validate(&facts.links).unwrap();
        }
        let foreign =
            inspect(b"<!doctype html><svg><link rel=match href=ref.html></link></svg>").unwrap();
        assert!(links::validate(&foreign.links).is_err());
    }
    #[test]
    fn ag7_reference_filename_is_not_a_url_resolver() {
        for name in [
            "../ref.html",
            "./ref.html",
            "folder/ref.html",
            "/ref.html",
            "https://example/ref.html",
            "ref.html?x",
            "ref.html#x",
            "ref%2ehtml",
            " ref.html",
            "ref.html ",
            "Ref.html",
            "a\\ref.html",
            ".html",
        ] {
            assert!(links::reference_filename(name).is_err(), "{name}");
        }
        for name in ["red.html", "background-color-rgb-001-ref.html", "a_1.html"] {
            links::reference_filename(name).unwrap();
        }
    }
}
